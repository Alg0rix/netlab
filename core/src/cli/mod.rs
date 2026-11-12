//! netlab-core IOS CLI engine — a per-device `Cli` session parses lines
//! against a per-kind static grammar DAG, dispatches acts into the
//! kind-gated config setters, renders ops, and answers ?/Tab/key/script
//! requests. The adapter never parses command text; core owns all wording.

pub mod types;
pub mod token;
pub mod ifmap;
pub mod tree;
pub mod matcher;
pub mod bind;
pub mod pending;
pub mod modes;
pub mod dispatch;
pub mod render;
pub mod nvgen;
pub mod translate;
pub mod script;
pub mod help;
pub mod key;

pub use types::{
    Act, Arg, CliResult, CompleteResult, HelpResult, HelpRow, Intent, ModeCtx, ModeId,
    Op, OpCls, Pending, PendingView, PStep, ScriptError, ScriptResult, Token, Verdict,
};
pub use ifmap::IfMap;

use std::collections::VecDeque;

use crate::device::{Device, DeviceKind};
use crate::events::SimEvent;
use matcher::{match_g, Outcome};

/// One CLI session on one device.
pub struct Cli {
    pub node: ModeId,
    pub ctx: ModeCtx,
    pub pending: Option<Pending>,
    /// interactive exec lines only — script lines never land here
    pub history: VecDeque<String>,
    pub history_max: usize,
    /// injected port-name aliases — the interface-name authority
    pub ifmap: IfMap,
    /// the name set `ifmap` was built from — a same-count rename must
    /// invalidate just like a count drift
    ifmap_src: Vec<String>,
    /// mirror of meta.startup — the ONLY thing `show startup-config` reads
    pub startup_text: Option<String>,
}

impl Cli {
    pub fn new() -> Cli {
        Cli {
            node: ModeId::Exec,
            ctx: ModeCtx::default(),
            pending: None,
            history: VecDeque::new(),
            history_max: 10,
            ifmap: IfMap::default(),
            ifmap_src: Vec::new(),
            startup_text: None,
        }
    }

    /// Adapter injects canonical names[] in declared port order.
    pub fn set_ifmap(&mut self, names: &[String]) -> Result<(), String> {
        self.ifmap = IfMap::new(names)?;
        self.ifmap_src = names.to_vec();
        Ok(())
    }

    /// Fall back to the device's own canonical names when the adapter
    /// hasn't injected a map (or the device's name set drifted).
    pub fn ifmap_for(&mut self, d: &Device) -> &IfMap {
        let want = d.port_names();
        if self.ifmap_src != want {
            if let Ok(m) = IfMap::new(&want) {
                self.ifmap = m;
                self.ifmap_src = want;
            }
        }
        &self.ifmap
    }

    /// pending-armed → the armed prompt verbatim; else the mode prompt.
    pub fn current_prompt(&self, d: &Device) -> String {
        if let Some(p) = &self.pending {
            return pending_prompt(p);
        }
        modes::mode_prompt(self, d)
    }

    pub fn pending_view(&self) -> Option<PendingView> {
        self.pending
            .as_ref()
            .map(|p| PendingView { prompt: pending_prompt(p) })
    }

    /// One submitted line. Order: pending gate → tokenize → match_g
    /// (walk-up on NoMatch) → bind → dispatch → assemble CliResult.
    pub fn exec(
        &mut self,
        d: &mut Device,
        line: &str,
        tick: u64,
        events: &mut Vec<SimEvent>,
    ) -> CliResult {
        let line = line.trim_end_matches('\r');
        // the pending gate precedes the blank early-exit — blank is the
        // dialog's default answer
        if self.pending.is_some() {
            return self.feed_pending(d, line, tick, events);
        }
        if line.trim().is_empty() {
            return CliResult {
                prompt: self.current_prompt(d),
                out: Vec::new(),
                mutated: false,
                pending: self.pending_view(),
            };
        }

        let result = self.exec_line(d, line, tick, events);
        if self.history_max > 0 {
            self.history.push_back(line.to_string());
            while self.history.len() > self.history_max {
                self.history.pop_front();
            }
        }
        result
    }

    /// exec() minus history — script lines route through the same
    /// pending gate and matcher/dispatch but never enter cli.history.
    pub(crate) fn exec_line(
        &mut self,
        d: &mut Device,
        line: &str,
        tick: u64,
        events: &mut Vec<SimEvent>,
    ) -> CliResult {
        if self.pending.is_some() {
            return self.feed_pending(d, line, tick, events);
        }

        let toks = token::tokenize(line);
        if toks.is_empty() {
            return CliResult {
                prompt: self.current_prompt(d),
                out: Vec::new(),
                mutated: false,
                pending: self.pending_view(),
            };
        }

        let mut node_order = vec![self.node];
        let mut cur = self.node;
        while let Some(p) = modes::parent(cur) {
            node_order.push(p);
            cur = p;
        }

        let mut best: Option<(Outcome, usize, ModeId)> = None;
        for node in &node_order {
            let (out, furthest) = {
                let ifmap = self.ifmap_for(d);
                match_g(tree::root(d.kind, *node), &toks, ifmap)
            };
            match out {
                Outcome::NoMatch { tok_index } => {
                    let idx = tok_index.max(furthest.min(toks.len().saturating_sub(1)));
                    if best
                        .as_ref()
                        .map(|(_, f, _)| idx > *f)
                        .unwrap_or(true)
                    {
                        best = Some((Outcome::NoMatch { tok_index: idx }, idx, *node));
                    }
                    continue;
                }
                other => {
                    best = Some((other, 0, *node));
                    break;
                }
            }
        }

        let (outcome, ..) = match best {
            Some(b) => b,
            None => (Outcome::NoMatch { tok_index: 0 }, 0, self.node),
        };

        match outcome {
            Outcome::Hit(c) => {
                match bind::bind(c.tag, &c.argv, &self.ctx, c.neg, c.dw, d) {
                    Ok(act) => {
                        // a walked-up Mode act applies only when the
                        // current node admits it
                        if let Act::Mode(op) = &act {
                            if !modes::admits(self.node, *op) {
                                return CliResult {
                                    prompt: self.current_prompt(d),
                                    out: self.triad(d, &toks, 0, 0),
                                    mutated: false,
                                    pending: self.pending_view(),
                                };
                            }
                        }
                        let (out, mutated) =
                            dispatch::dispatch(self, d, act, tick, events);
                        CliResult {
                            prompt: self.current_prompt(d),
                            out,
                            mutated,
                            pending: self.pending_view(),
                        }
                    }
                    Err(tok_index) => CliResult {
                        prompt: self.current_prompt(d),
                        out: self.triad(d, &toks, tok_index, 2),
                        mutated: false,
                        pending: self.pending_view(),
                    },
                }
            }
            Outcome::Incomplete => CliResult {
                prompt: self.current_prompt(d),
                out: self.triad(d, &toks, 0, 1),
                mutated: false,
                pending: self.pending_view(),
            },
            Outcome::Ambiguous => CliResult {
                prompt: self.current_prompt(d),
                out: self.triad(d, &toks, 0, 3),
                mutated: false,
                pending: self.pending_view(),
            },
            Outcome::NoMatch { tok_index } => {
                // translate fires iff every level stalled at tok 0 and
                // the first token is not a lattice verb
                let lattice_verb = matches!(
                    toks[0].text.to_lowercase().as_str(),
                    "no" | "enable" | "disable" | "configure"
                );
                if matches!(d.kind, DeviceKind::Router | DeviceKind::Switch)
                    && tok_index == 0
                    && !lattice_verb
                {
                    let (out, mutated) = dispatch::dispatch(
                        self,
                        d,
                        Act::Translate { word: toks[0].text.clone() },
                        tick,
                        events,
                    );
                    CliResult {
                        prompt: self.current_prompt(d),
                        out,
                        mutated,
                        pending: self.pending_view(),
                    }
                } else {
                    CliResult {
                        prompt: self.current_prompt(d),
                        out: self.triad(d, &toks, tok_index, 0),
                        mutated: false,
                        pending: self.pending_view(),
                    }
                }
            }
        }
    }

    /// Route an armed dialog's line through feed(). The blank early
    /// exit in exec() must never precede this — blank is the default
    /// answer.
    fn feed_pending(
        &mut self,
        d: &mut Device,
        line: &str,
        tick: u64,
        events: &mut Vec<SimEvent>,
    ) -> CliResult {
        let p = self.pending.take().unwrap();
        let mut runner = |cli: &mut Cli, step: PStep| -> (Vec<Op>, bool) {
            run_pstep(cli, d, step, tick, events)
        };
        let (out, mutated) = pending::feed(p, line, &mut runner, self);
        CliResult {
            prompt: self.current_prompt(d),
            out,
            mutated,
            pending: self.pending_view(),
        }
    }

    /// `?` on textBeforeCursor.
    pub fn help(&mut self, d: &Device, before_cursor: &str) -> HelpResult {
        self.ifmap_for(d);
        help::help(self, d, before_cursor, &self.ifmap)
    }

    /// Tab on textBeforeCursor.
    pub fn complete(&mut self, d: &Device, before_cursor: &str) -> CompleteResult {
        self.ifmap_for(d);
        help::complete(self, d, before_cursor, &self.ifmap)
    }

    /// ctrlz/ctrlc.
    pub fn key(
        &mut self,
        d: &mut Device,
        which: &str,
        tick: u64,
        events: &mut [SimEvent],
    ) -> CliResult {
        key::key(self, d, which, tick, events)
    }

    /// run a script batch (startup config / paste / reload replay).
    pub fn script(
        &mut self,
        d: &mut Device,
        text: &str,
        tick: u64,
        events: &mut Vec<SimEvent>,
    ) -> ScriptResult {
        script::run_script(self, d, text, tick, events)
    }
}

impl Default for Cli {
    fn default() -> Self {
        Cli::new()
    }
}

fn pending_prompt(p: &Pending) -> String {
    match p {
        Pending::YesNo { prompt, .. } | Pending::Field { prompt, .. } => prompt.clone(),
    }
}

/// Run one resolved dialog step — an Act dispatch or a chained Ask —
/// inside the same cli_exec. Seq runs each step in order (the reload
/// save leg emits the write before arming proceed).
fn run_pstep(
    cli: &mut Cli,
    d: &mut Device,
    step: PStep,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> (Vec<Op>, bool) {
    match step {
        PStep::Act(act) => dispatch::dispatch(cli, d, act, tick, events),
        PStep::Ask(p) => {
            cli.pending = Some(p);
            (Vec::new(), false)
        }
        PStep::Out(ops) => (ops, false),
        PStep::Seq(steps) => {
            let mut out = Vec::new();
            let mut mutated = false;
            for s in steps {
                let (o, m) = run_pstep(cli, d, s, tick, events);
                out.extend(o);
                mutated |= m;
            }
            (out, mutated)
        }
    }
}

/// The echo-separator predicate — ONE site: sep(p) = 0 when the prompt
/// already ends with a space, else 1.
pub fn sep(p: &str) -> usize {
    if p.ends_with(' ') {
        0
    } else {
        1
    }
}

impl Cli {
    /// The error triad, through ONE mapping site. kind:
    ///   0/2 → NoMatch/bind-defect: padded caret + "% Invalid input
    ///         detected at '^' marker." — caret pad is
    ///         display_width(prompt) + sep(prompt) + Token.start
    ///   1   → Incomplete: "% Incomplete command."
    ///   3   → Ambiguous: "% Ambiguous command"
    fn triad(&self, d: &Device, toks: &[Token], tok_index: usize, kind: u8) -> Vec<Op> {
        match kind {
            1 => vec![Op::err("% Incomplete command.")],
            3 => vec![Op::err("% Ambiguous command")],
            _ => {
                let p = self.current_prompt(d);
                let index = toks.get(tok_index).map(|t| t.start).unwrap_or(0);
                let pad = p.chars().count() + sep(&p) + index;
                vec![
                    Op::text(format!("{}^", " ".repeat(pad))),
                    Op::err("% Invalid input detected at '^' marker."),
                ]
            }
        }
    }
}
