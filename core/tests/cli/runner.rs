//! Transcript runner — parse, drive, render, normalize, diff.
//! Zero dependencies. All exec paths call the production entry points:
//! Cli::exec, Cli::script, Cli::help, Cli::complete, Cli::key.

use std::fs;
use std::path::{Path, PathBuf};

use netlab_core::cli::types::{CompleteResult, HelpResult, Op, OpCls};
use netlab_core::cli::{sep, Cli, IfMap};
use netlab_core::{DeviceId, DeviceKind, SimEvent, Topology};

use crate::fixtures::{self, TopoId};

pub const CORPUS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cli/corpus");
pub const STEPS_JSON: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../web/_test/corpus-steps.json");

pub const FR_IDS: &[&str] = &[
    "FR-SVI",
    "FR-ACL-NAMED",
    "FR-OSPF-DEEP",
    "FR-DHCP-POOL",
    "FR-NAT-POOL",
    "FR-PING-EXT",
    "FR-AUTH",
    "FR-EMACS",
    "FR-PAGER",
    "FR-UNBACKED",
];

/// Every deferred-FR keyword head, each as a token sequence that must
/// appear contiguously inside ≥1 gap-bound exec line. `interface vlan` /
/// `interface vl` stand only under the dormant-family pin: no v1 kind
/// emits `vl<N>` canonical names, so no `vl<N>` key ever exists in a v1
/// IfMap; when a kind gains `vl<N>` ports these markers flip to real
/// goldens in the same commit.
pub const UNBACKED_HEADS: &[(&str, &[&str])] = &[
    ("FR-SVI", &["interface", "vlan"]),
    ("FR-SVI", &["interface", "vl"]),
    ("FR-SVI", &["ip", "default-gateway"]),
    ("FR-SVI", &["no", "switchport"]),
    ("FR-ACL-NAMED", &["ip", "access-list"]),
    ("FR-OSPF-DEEP", &["passive-interface"]),
    ("FR-OSPF-DEEP", &["default-information"]),
    ("FR-DHCP-POOL", &["ip", "dhcp", "pool"]),
    ("FR-NAT-POOL", &["ip", "nat", "pool"]),
    ("FR-NAT-POOL", &["overload"]),
    ("FR-PING-EXT", &["repeat"]),
    ("FR-PING-EXT", &["size"]),
    ("FR-AUTH", &["enable", "secret"]),
    ("FR-AUTH", &["line"]),
    ("FR-AUTH", &["login"]),
    ("FR-AUTH", &["username"]),
    ("FR-AUTH", &["exec-timeout"]),
    ("FR-AUTH", &["privilege"]),
    ("FR-AUTH", &["banner"]),
    ("FR-EMACS", &["terminal", "editing"]),
    ("FR-PAGER", &["terminal", "length"]),
    ("FR-UNBACKED", &["mac", "address-table", "static"]),
    ("FR-UNBACKED", &["arp"]),
    ("FR-UNBACKED", &["logging"]),
    ("FR-UNBACKED", &["mtu"]),
    ("FR-UNBACKED", &["speed"]),
    ("FR-UNBACKED", &["duplex"]),
    ("FR-UNBACKED", &["interface", "range"]),
    ("FR-UNBACKED", &["ssh"]),
    ("FR-UNBACKED", &["telnet"]),
    ("FR-UNBACKED", &["cdp"]),
    ("FR-UNBACKED", &["lldp"]),
    ("FR-UNBACKED", &["vtp"]),
    ("FR-UNBACKED", &["spanning-tree"]),
    ("FR-UNBACKED", &["debug"]),
    ("FR-UNBACKED", &["standby"]),
    ("FR-UNBACKED", &["vrrp"]),
    ("FR-UNBACKED", &["hsrp"]),
    ("FR-UNBACKED", &["channel-group"]),
    ("FR-UNBACKED", &["port-channel"]),
    ("FR-UNBACKED", &["aaa"]),
    ("FR-UNBACKED", &["ip", "http"]),
    ("FR-UNBACKED", &["snmp-server"]),
    ("FR-UNBACKED", &["ntp"]),
    ("FR-UNBACKED", &["ipv6"]),
    ("FR-UNBACKED", &["route-map"]),
    ("FR-UNBACKED", &["crypto"]),
    ("FR-UNBACKED", &["mpls"]),
    ("FR-UNBACKED", &["ip", "sla"]),
    ("FR-UNBACKED", &["udld"]),
    ("FR-UNBACKED", &["monitor"]),
    ("FR-UNBACKED", &["ip", "helper-address"]),
    ("FR-UNBACKED", &["redistribute"]),
    ("FR-UNBACKED", &["service", "password-encryption"]),
];

#[derive(Debug)]
pub struct CaseErr {
    pub file: String,
    pub line: u32,
    pub msg: String,
}

impl CaseErr {
    fn at(file: &str, line: u32, msg: impl Into<String>) -> CaseErr {
        CaseErr {
            file: file.to_string(),
            line,
            msg: msg.into(),
        }
    }
    fn whole(file: &str, msg: impl Into<String>) -> CaseErr {
        CaseErr::at(file, 0, msg)
    }
}

impl std::fmt::Display for CaseErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.file, self.msg)
        } else {
            write!(f, "{}:{}: {}", self.file, self.line, self.msg)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mask {
    Uptime,
    Tick,
}

#[derive(Debug, Clone)]
pub struct Gap {
    pub fr: String,
    pub tok: Option<usize>,
}

#[derive(Debug)]
pub enum Step {
    Exec { line: String, gap: Option<Gap> },
    Script(String),
    Key { op: KeyOp, text: String },
    Tick(u64),
    Drain,
    AllowEvents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOp {
    Help,
    Complete,
    CtrlC,
    CtrlZ,
}

impl KeyOp {
    pub fn name(self) -> &'static str {
        match self {
            KeyOp::Help => "?",
            KeyOp::Complete => "tab",
            KeyOp::CtrlC => "ctrlc",
            KeyOp::CtrlZ => "ctrlz",
        }
    }
}

#[derive(Debug)]
pub struct Case {
    pub id: String,
    pub kind: DeviceKind,
    pub topo: TopoId,
    pub tick: u64,
    pub ifmap: Option<Vec<String>>,
    pub startup: Option<String>,
    pub masks: Vec<Mask>,
    pub provenance: String,
    pub steps: Vec<Step>,
}

impl Case {
    pub fn allow_events(&self) -> bool {
        self.steps.iter().any(|s| matches!(s, Step::AllowEvents))
    }
}

pub fn parse_case(path: &Path) -> Result<Case, CaseErr> {
    let file = path.display().to_string();
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let bytes = fs::read(path)
        .map_err(|e| CaseErr::whole(&file, format!("cannot read: {e}")))?;
    let src = String::from_utf8(bytes)
        .map_err(|e| CaseErr::whole(&file, format!("not utf-8: {e}")))?;
    let raw_lines: Vec<&str> = src.split('\n').collect();
    let lines: Vec<String> = raw_lines
        .iter()
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect();


    if lines.is_empty() || lines[0] != "## netlab-corpus v1" {
        return Err(perr(&file, 0, "missing magic line `## netlab-corpus v1`"));
    }

    let mut id: Option<String> = None;
    let mut kind: Option<DeviceKind> = None;
    let mut topo = TopoId::SingleRouter;
    let mut tick: u64 = 0;
    let mut ifmap: Option<Vec<String>> = None;
    let mut startup: Option<String> = None;
    let mut masks: Vec<Mask> = Vec::new();
    let mut provenance: Option<String> = None;
    let mut seen: Vec<String> = Vec::new();

    let mut i = 1usize;
    'header: while i < lines.len() {
        let line = &lines[i];
        if line.trim().is_empty() {
            i += 1;
            break 'header;
        }
        if line.starts_with(' ') {
            return Err(perr(&file, 
                i,
                "stray continuation line outside `## startup:`",
            ));
        }
        if !line.starts_with("##") {
            return Err(perr(&file, i, "header unterminated — non-`##` line before the blank terminator"));
        }
        if line == "## startup:" {
            if seen.iter().any(|s| s == "startup") {
                return Err(perr(&file, i, "duplicate `## startup:`"));
            }
            seen.push("startup".to_string());
            i += 1;
            let mut stored: Vec<String> = Vec::new();
            while i < lines.len() && lines[i].starts_with(' ') {
                stored.push(lines[i][1..].to_string());
                i += 1;
            }
            if stored.is_empty() {
                return Err(CaseErr::at(
                    &file,
                    i as u32,
                    "`## startup:` block carries zero stored lines",
                ));
            }
            startup = Some(stored.join("\n"));
            continue 'header;
        }
        let (field, value) = match line[2..].split_once(':') {
            Some((f, v)) => (f.trim(), v.trim().to_string()),
            None => return Err(perr(&file, i, format!("bad header field `{line}`"))),
        };
        if field != "note" && seen.iter().any(|s| s == field) {
            return Err(perr(&file, i, format!("duplicate `## {field}:`")));
        }
        seen.push(field.to_string());
        match field {
            "id" => id = Some(value),
            "kind" => {
                kind = Some(match value.as_str() {
                    "router" => DeviceKind::Router,
                    "switch" => DeviceKind::Switch,
                    "host" => DeviceKind::Host,
                    _ => {
                        return Err(perr(&file, 
                            i,
                            format!("bad kind `{value}` — router|switch|host"),
                        ))
                    }
                })
            }
            "topo" => {
                topo = fixtures::topo_id(&value).ok_or_else(|| {
                    perr(&file, i, format!("unknown topo `{value}`"))
                })?
            }
            "tick" => {
                tick = value
                    .parse::<u64>()
                    .map_err(|_| perr(&file, i, format!("bad tick `{value}` — u64 expected")))?
            }
            "ifmap" => {
                let names: Vec<String> =
                    value.split(',').map(|s| s.trim().to_string()).collect();
                IfMap::new(&names)
                    .map_err(|e| perr(&file, i, format!("non-canonical `## ifmap:` entry — {e}")))?;
                ifmap = Some(names);
            }
            "masks" => {
                for name in value.split(',').map(|s| s.trim()) {
                    if name.is_empty() {
                        continue;
                    }
                    masks.push(match name {
                        "uptime" => Mask::Uptime,
                        "tick" => Mask::Tick,
                        _ => {
                            return Err(perr(&file, 
                                i,
                                format!("unknown mask `{name}` — uptime|tick"),
                            ))
                        }
                    });
                }
            }
            "provenance" => {
                let ok = value == "authored"
                    || value.starts_with("regression:")
                    || value.starts_with("oracle:");
                if !ok {
                    return Err(perr(&file, 
                        i,
                        format!("bad provenance `{value}` — authored|regression:<ref>|oracle:<ref>"),
                    ));
                }
                provenance = Some(value);
            }
            "note" => {}
            other => return Err(perr(&file, i, format!("bad header field `{other}`"))),
        }
        i += 1;
    }

    let id = id.ok_or_else(|| CaseErr::whole(&file, "missing required `## id:`"))?;
    if id != stem {
        return Err(CaseErr::whole(
            &file,
            format!("`## id: {id}` does not match file stem `{stem}`"),
        ));
    }
    let kind = kind.ok_or_else(|| CaseErr::whole(&file, "missing required `## kind:`"))?;
    let provenance =
        provenance.ok_or_else(|| CaseErr::whole(&file, "missing required `## provenance:`"))?;

    let mut steps: Vec<Step> = Vec::new();
    let mut pending_gap: Option<(Gap, usize)> = None;

    while i < lines.len() {
        let line = &lines[i];
        if line == "## script" {
            if let Some((_, ln)) = &pending_gap {
                return Err(perr(&file, 
                    *ln,
                    "dangling `## gap:` — the bound step must be a bare exec line",
                ));
            }
            let script_ln = i;
            i += 1;
            let mut text: Vec<String> = Vec::new();
            loop {
                if i >= lines.len() {
                    return Err(perr(&file, 
                        script_ln,
                        "`## script` unterminated at EOF",
                    ));
                }
                if lines[i] == "## endscript" {
                    break;
                }
                if lines[i] == "## script" {
                    return Err(perr(&file, i, "nested `## script`"));
                }
                text.push(lines[i].clone());
                i += 1;
            }
            i += 1;
            steps.push(Step::Script(text.join("\n")));
            continue;
        }
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        if line.starts_with('#') && !line.starts_with("##") {
            i += 1;
            continue;
        }
        if line.starts_with(' ') {
            return Err(perr(&file, 
                i,
                "stray continuation line outside `## startup:`",
            ));
        }
        if let Some((_, ln)) = &pending_gap {
            if line.starts_with("##") {
                return Err(perr(&file, 
                    *ln,
                    "dangling `## gap:` — the bound step must be a bare exec line",
                ));
            }
        }
        if line.starts_with("##") {
            let step = parse_directive(line)
                .map_err(|m| perr(&file, i, m))?;
            if let Some(step) = step {
                steps.push(step);
            } else {
                // `## gap:` arms the next exec line
                let gap = parse_gap(line).map_err(|m| perr(&file, i, m))?;
                pending_gap = Some((gap, i));
            }
            i += 1;
            continue;
        }
        let gap = pending_gap.take().map(|(g, _)| g);
        steps.push(Step::Exec {
            line: line.clone(),
            gap,
        });
        i += 1;
    }

    if let Some((_, ln)) = pending_gap {
        return Err(perr(&file, 
            ln,
            "dangling `## gap:` — the bound step must be a bare exec line",
        ));
    }
    if steps.is_empty() {
        return Err(CaseErr::whole(&file, "empty case — no steps"));
    }

    Ok(Case {
        id,
        kind,
        topo,
        tick,
        ifmap,
        startup,
        masks,
        provenance,
        steps,
    })
}

/// Non-gap step directives. `None` means the line is a `## gap:` marker —
/// handled by the caller so it can bind to the next exec line.
fn parse_directive(line: &str) -> Result<Option<Step>, String> {
    if line == "## drain" {
        return Ok(Some(Step::Drain));
    }
    if line == "## allow-events" {
        return Ok(Some(Step::AllowEvents));
    }
    if let Some(v) = line.strip_prefix("## tick:") {
        let v = v.trim();
        if let Some(n) = v.strip_prefix('+') {
            return n
                .parse::<u64>()
                .map(|n| Some(Step::Tick(n)))
                .map_err(|_| format!("bad tick step `{line}`"));
        }
        return Err(format!("unknown `##` directive `{line}`"));
    }
    if let Some(rest) = line.strip_prefix("## key:") {
        if let Some(op) = rest.strip_prefix(" ?") {
            let text = op.strip_prefix(' ').unwrap_or(op);
            return Ok(Some(Step::Key {
                op: KeyOp::Help,
                text: text.to_string(),
            }));
        }
        if let Some(op) = rest.strip_prefix(" tab") {
            let text = op.strip_prefix(' ').unwrap_or(op);
            return Ok(Some(Step::Key {
                op: KeyOp::Complete,
                text: text.to_string(),
            }));
        }
        return match rest.trim() {
            "ctrlc" => Ok(Some(Step::Key {
                op: KeyOp::CtrlC,
                text: String::new(),
            })),
            "ctrlz" => Ok(Some(Step::Key {
                op: KeyOp::CtrlZ,
                text: String::new(),
            })),
            _ => Err(format!("unknown `##` directive `{line}`")),
        };
    }
    if line == "## endscript" {
        return Err("stray `## endscript` outside `## script`".to_string());
    }
    if line.starts_with("## gap:") {
        return Ok(None);
    }
    Err(format!("unknown `##` directive `{line}`"))
}

fn perr(file: &str, idx0: usize, msg: impl Into<String>) -> CaseErr {
    CaseErr::at(file, (idx0 + 1) as u32, msg)
}

fn parse_gap(line: &str) -> Result<Gap, String> {
    let rest = line["## gap:".len()..].trim();
    let mut it = rest.split_whitespace();
    let fr = it.next().unwrap_or("");
    if !FR_IDS.contains(&fr) {
        return Err(format!("unknown gap FR-id `{fr}` in `{line}`"));
    }
    let tok = match it.next() {
        None => None,
        Some(t) => {
            let digits = t
                .strip_prefix("tok")
                .ok_or_else(|| format!("bad gap qualifier `{t}` in `{line}`"))?;
            Some(
                digits
                    .parse::<usize>()
                    .map_err(|_| format!("bad gap qualifier `{t}` in `{line}`"))?,
            )
        }
    };
    if it.next().is_some() {
        return Err(format!("bad `## gap:` spelling `{line}`"));
    }
    Ok(Gap {
        fr: fr.to_string(),
        tok,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Never masked — echoes, directives, caret, prompts, counts.
    Bare,
    /// uptime-maskable engine text.
    Engine,
    /// uptime + tick maskable — `!event` lines only.
    Event,
}

#[derive(Debug)]
pub struct Frame {
    pub text: String,
    pub scope: Scope,
}

fn f(text: impl Into<String>, scope: Scope) -> Frame {
    Frame {
        text: text.into(),
        scope,
    }
}

fn text_frames(frames: &mut Vec<Frame>, tag: &str, text: &str) {
    for seg in text.split('\n') {
        frames.push(if tag.is_empty() {
            f(seg.to_string(), Scope::Engine)
        } else {
            f(format!("{tag} {seg}"), Scope::Engine)
        });
    }
}

fn op_frames(out: Vec<Op>) -> Vec<Frame> {
    let mut frames = Vec::new();
    for op in out {
        match op {
            Op::Text { cls, text } => {
                let tag = match cls {
                    None => "",
                    Some(OpCls::Ok) => "!ok",
                    Some(OpCls::Dim) => "!dim",
                    Some(OpCls::Warn) => "!warn",
                    Some(OpCls::Err) => "!err",
                    Some(OpCls::Echo) => "!echo",
                };
                text_frames(&mut frames, tag, &text);
            }
            Op::Echo { text } => text_frames(&mut frames, "", &text),
            Op::Clear => frames.push(f("!clear", Scope::Bare)),
            Op::Bell => frames.push(f("!bell", Scope::Bare)),
            Op::Intent(i) => match i {
                netlab_core::Intent::Ping { target } => {
                    frames.push(f(format!("!intent ping {target}"), Scope::Engine))
                }
                netlab_core::Intent::Trace { target } => {
                    frames.push(f(format!("!intent trace {target}"), Scope::Engine))
                }
                netlab_core::Intent::Rename { name } => {
                    frames.push(f(format!("!intent rename {name}"), Scope::Engine))
                }
                netlab_core::Intent::NvramWrite { text } => {
                    let segs: Vec<&str> = text.split('\n').collect();
                    frames.push(f(format!("!intent nvram-write {}", segs.len()), Scope::Bare));
                    for seg in segs {
                        frames.push(f(format!("  {seg}"), Scope::Engine));
                    }
                }
                netlab_core::Intent::NvramErase => {
                    frames.push(f("!intent nvram-erase", Scope::Engine))
                }
                netlab_core::Intent::Reload => {
                    frames.push(f("!intent reload", Scope::Engine))
                }
            },
        }
    }
    frames
}

fn prompt_frame(prompt: &str, pending: bool) -> Frame {
    if pending {
        f(format!("!pending \"{prompt}\""), Scope::Bare)
    } else {
        f(format!("!prompt {prompt}"), Scope::Bare)
    }
}

fn event_frames(events: Vec<SimEvent>) -> Vec<Frame> {
    events
        .into_iter()
        .map(|e| {
            // EvKind lives in a private module — Debug names match the
            // lowercase serde vocabulary exactly.
            let kind = format!("{:?}", e.kind).to_lowercase();
            f(format!("!event {kind} {}", e.msg), Scope::Event)
        })
        .collect()
}

pub fn drain(topo: &mut Topology, _dev: DeviceId) -> Vec<SimEvent> {
    topo.drain_events()
}

pub fn pump(topo: &mut Topology, n: u64) {
    for _ in 0..n {
        topo.step();
    }
}

const TRIAD: &[&str] = &[
    "% Invalid input detected at '^' marker.",
    "% Incomplete command.",
    "% Ambiguous command",
];
const TRANSLATE_ERR: &str =
    "% Unknown command or computer name, or unable to find computer address";

fn token_starts(line: &str) -> Vec<usize> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b' ' || b[i] == b'\t' {
            i += 1;
            continue;
        }
        out.push(i);
        while i < b.len() && b[i] != b' ' && b[i] != b'\t' {
            i += 1;
        }
    }
    out
}

/// The three honest-error classes a `## gap:` marker discharges on.
struct GapHit {
    caret_pad: Option<usize>,
    triad: bool,
    pct_reject: bool,
    translate: bool,
}

fn gap_classify(out: &[Op]) -> GapHit {
    let mut hit = GapHit {
        caret_pad: None,
        triad: false,
        pct_reject: false,
        translate: false,
    };
    for op in out {
        match op {
            Op::Text { cls, text } => {
                if text.starts_with("Translating \"") {
                    hit.translate = true;
                }
                if text.trim() == "^" {
                    hit.caret_pad = Some(text.chars().count() - 1);
                }
                if *cls == Some(OpCls::Err) {
                    if TRIAD.contains(&text.as_str()) {
                        hit.triad = true;
                    } else if text == TRANSLATE_ERR {
                        hit.translate = true;
                    } else if text.starts_with('%') {
                        hit.pct_reject = true;
                    }
                }
            }
            Op::Echo { .. } | Op::Clear | Op::Bell | Op::Intent(_) => {}
        }
    }
    hit
}

fn check_gap(
    gap: &Gap,
    line: &str,
    prompt: &str,
    out: &[Op],
    mutated: bool,
) -> Result<(), String> {
    if mutated {
        return Err(format!(
            "gap closed — remove marker, write real golden (line `{line}` mutated)"
        ));
    }
    let hit = gap_classify(out);
    let matched = hit.triad || hit.pct_reject || hit.translate;
    if !matched {
        return Err(format!(
            "`## gap:` unmet classes on `{line}` — not a caret+triad, a `%` rejection, or a translate render; if the line succeeded: gap closed — remove marker, write real golden"
        ));
    }
    match gap.tok {
        Some(0) => {
            if !hit.translate {
                return Err(format!(
                    "`## gap:` tok0 unmet on `{line}` — expected the translate render"
                ));
            }
        }
        Some(i) => {
            let starts = token_starts(line);
            let Some(&start) = starts.get(i) else {
                return Err(format!(
                    "`## gap:` tok{i} unmet on `{line}` — token {i} does not exist"
                ));
            };
            let expected = prompt.chars().count() + sep(prompt) + start;
            match hit.caret_pad {
                Some(pad) if pad == expected => {}
                _ => {
                    return Err(format!(
                        "`## gap:` tok{i} unmet on `{line}` — NoMatch caret did not land on Token[{i}]"
                    ))
                }
            }
        }
        None => {}
    }
    Ok(())
}

pub fn run_case(case: &Case) -> Result<Vec<Frame>, String> {
    let (mut topo, dev) = fixtures::build(case.topo, case.kind)?;
    topo.tick = case.tick;
    let names = match &case.ifmap {
        Some(n) => n.clone(),
        None => topo.devices[&dev].port_names(),
    };
    let mut cli = Cli::new();
    cli.set_ifmap(&names)
        .map_err(|e| format!("{}: set_ifmap: {e}", case.id))?;
    cli.startup_text = case.startup.clone();

    let mut frames: Vec<Frame> = Vec::new();
    for step in &case.steps {
        match step {
            Step::Exec { line, gap } => {
                if let Some(g) = gap {
                    let replay = match g.tok {
                        Some(i) => format!("## gap: {} tok{i}", g.fr),
                        None => format!("## gap: {}", g.fr),
                    };
                    frames.push(f(replay, Scope::Bare));
                }
                let prompt = {
                    let d = &topo.devices[&dev];
                    cli.current_prompt(d)
                };
                frames.push(f(
                    format!("{prompt}{}{line}", " ".repeat(sep(&prompt))),
                    Scope::Bare,
                ));
                let r = {
                    let d = topo.devices.get_mut(&dev).expect("session device");
                    cli.exec(d, line, topo.tick, &mut topo.events)
                };
                if let Some(g) = gap {
                    check_gap(g, line, &prompt, &r.out, r.mutated)
                        .map_err(|m| format!("{}: {m}", case.id))?;
                }
                frames.extend(op_frames(r.out));
            }
            Step::Script(text) => {
                frames.push(f("## script", Scope::Bare));
                let r = {
                    let d = topo.devices.get_mut(&dev).expect("session device");
                    cli.script(d, text, topo.tick, &mut topo.events)
                };
                frames.extend(op_frames(r.out));
                for e in &r.errors {
                    frames.push(f(format!("!error {} {}", e.lineno, e.msg), Scope::Engine));
                }
                frames.push(f("## endscript", Scope::Bare));
            }
            Step::Key { op, text } => {
                let replay = match op {
                    KeyOp::CtrlC | KeyOp::CtrlZ => format!("## key: {}", op.name()),
                    _ if text.is_empty() => format!("## key: {}", op.name()),
                    _ => format!("## key: {} {}", op.name(), text),
                };
                frames.push(f(replay, Scope::Bare));
                match op {
                    KeyOp::Help => {
                        let d = &topo.devices[&dev];
                        match cli.help(d, text) {
                            HelpResult::Rows { rows, prompt } => {
                                frames.push(f(format!("!rows {}", rows.len()), Scope::Bare));
                                for row in rows {
                                    let line = match row.help {
                                        Some(h) => format!("  {} {}", row.word, h),
                                        None => format!("  {}", row.word),
                                    };
                                    frames.push(f(line, Scope::Engine));
                                }
                                frames.push(prompt_frame(&prompt, cli.pending.is_some()));
                            }
                            HelpResult::Inline { inline, prompt } => {
                                frames.push(f(format!("!inline {}", inline.join(" ")), Scope::Engine));
                                frames.push(prompt_frame(&prompt, cli.pending.is_some()));
                            }
                            HelpResult::Err { index, prompt } => {
                                frames.push(f(format!("!caret {index}"), Scope::Bare));
                                frames.push(prompt_frame(&prompt, cli.pending.is_some()));
                            }
                        }
                    }
                    KeyOp::Complete => {
                        let (r, prompt) = {
                            let d = &topo.devices[&dev];
                            let r = cli.complete(d, text);
                            let p = cli.current_prompt(d);
                            (r, p)
                        };
                        match r {
                            CompleteResult::Line { line } => {
                                frames.push(f(format!("!line {line}"), Scope::Engine))
                            }
                            CompleteResult::List { list } => {
                                frames.push(f(format!("!list {}", list.join(" ")), Scope::Engine))
                            }
                            CompleteResult::Err { index } => {
                                frames.push(f(format!("!caret {index}"), Scope::Bare))
                            }
                        }
                        frames.push(prompt_frame(&prompt, cli.pending.is_some()));
                    }
                    KeyOp::CtrlC | KeyOp::CtrlZ => {
                        let which = if *op == KeyOp::CtrlC { "ctrlc" } else { "ctrlz" };
                        let r = {
                            let d = topo.devices.get_mut(&dev).expect("session device");
                            cli.key(d, which, topo.tick, &mut topo.events)
                        };
                        frames.extend(op_frames(r.out));
                        frames.push(prompt_frame(&r.prompt, r.pending.is_some()));
                    }
                }
            }
            Step::Tick(n) => {
                frames.push(f(format!("## tick: +{n}"), Scope::Bare));
                pump(&mut topo, *n);
                frames.extend(event_frames(drain(&mut topo, dev)));
            }
            Step::Drain => {
                frames.push(f("## drain", Scope::Bare));
                frames.extend(event_frames(drain(&mut topo, dev)));
            }
            Step::AllowEvents => {
                frames.push(f("## allow-events", Scope::Bare));
            }
        }
    }
    {
        let d = &topo.devices[&dev];
        let prompt = cli.current_prompt(d);
        frames.push(prompt_frame(&prompt, cli.pending.is_some()));
    }
    if !case.allow_events() && !topo.events.is_empty() {
        return Err(format!(
            "{}: unasserted events left at case end — {} events undrained (use ## drain or ## allow-events)",
            case.id,
            topo.events.len()
        ));
    }
    Ok(frames)
}

#[derive(Debug, Default)]
pub struct MaskSet {
    uptime: bool,
    tick: bool,
    used_uptime: bool,
    used_tick: bool,
}

impl MaskSet {
    pub fn new(masks: &[Mask]) -> MaskSet {
        MaskSet {
            uptime: masks.contains(&Mask::Uptime),
            tick: masks.contains(&Mask::Tick),
            used_uptime: false,
            used_tick: false,
        }
    }
    pub fn check_used(&self) -> Result<(), String> {
        if self.uptime && !self.used_uptime {
            return Err("declared mask `uptime` never fired".to_string());
        }
        if self.tick && !self.used_tick {
            return Err("declared mask `tick` never fired".to_string());
        }
        Ok(())
    }
}

fn digits(b: &[u8], mut i: usize) -> usize {
    let s = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    i - s
}

/// `\d+:\d{2}:\d{2}` — H:MM:SS.
fn m_hms(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + digits(b, i);
    if j == i || b.get(j) != Some(&b':') {
        return None;
    }
    j += 1;
    if digits(b, j) < 2 {
        return None;
    }
    j += 2;
    if b.get(j) != Some(&b':') {
        return None;
    }
    j += 1;
    if digits(b, j) < 2 {
        return None;
    }
    Some(j + 2 - i)
}

/// `\d+w\d+d` — weeks+days.
fn m_wd(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + digits(b, i);
    if j == i || b.get(j) != Some(&b'w') {
        return None;
    }
    j += 1;
    let d = digits(b, j);
    if d == 0 {
        return None;
    }
    j += d;
    if b.get(j) != Some(&b'd') {
        return None;
    }
    Some(j + 1 - i)
}

/// `\d+d\d+h` — days+hours.
fn m_dh(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + digits(b, i);
    if j == i || b.get(j) != Some(&b'd') {
        return None;
    }
    j += 1;
    let d = digits(b, j);
    if d == 0 {
        return None;
    }
    j += d;
    if b.get(j) != Some(&b'h') {
        return None;
    }
    Some(j + 1 - i)
}

/// `<lit><d+>` — literal prefix then digits.
fn m_lit_digits(b: &[u8], i: usize, lit: &[u8]) -> Option<usize> {
    if b.len().saturating_sub(i) < lit.len() || &b[i..i + lit.len()] != lit {
        return None;
    }
    let d = digits(b, i + lit.len());
    if d == 0 {
        None
    } else {
        Some(lit.len() + d)
    }
}

fn sub_all(
    s: &str,
    m: impl Fn(&[u8], usize) -> Option<usize>,
    rep: &str,
) -> (String, bool) {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    let mut hit = false;
    while i < b.len() {
        match m(b, i) {
            Some(n) if n > 0 => {
                out.extend_from_slice(rep.as_bytes());
                i += n;
                hit = true;
            }
            _ => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    (String::from_utf8(out).unwrap_or_else(|_| s.to_string()), hit)
}

pub fn apply_masks(line: &str, scope: Scope, set: &mut MaskSet) -> String {
    let mut s = line.to_string();
    if set.uptime && matches!(scope, Scope::Engine | Scope::Event) {
        for m in [m_hms as fn(&[u8], usize) -> Option<usize>, m_wd, m_dh] {
            let (n, hit) = sub_all(&s, m, "<uptime>");
            s = n;
            set.used_uptime |= hit;
        }
    }
    if set.tick && scope == Scope::Event {
        for lit in [b"tick " as &[u8], b"tick=", b"t="] {
            let rep = match lit {
                b"tick " => "tick <tick>",
                b"tick=" => "tick=<tick>",
                _ => "t=<tick>",
            };
            let (n, hit) = sub_all(&s, |b, i| m_lit_digits(b, i, lit), rep);
            s = n;
            set.used_tick |= hit;
        }
    }
    s
}

pub fn normalize(frames: &[Frame], masks: &mut MaskSet) -> String {
    frames
        .iter()
        .map(|fr| {
            let s = apply_masks(&fr.text, fr.scope, masks);
            s.replace('\r', "")
                .lines()
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// N3 over the WHOLE file — HASH-PATH ONLY.
pub struct NormalizedRaw(pub Vec<u8>);

pub fn normalize_raw(src: &str) -> NormalizedRaw {
    let mut out: Vec<u8> = Vec::with_capacity(src.len());
    for l in src.replace('\r', "").split('\n') {
        out.extend_from_slice(l.trim_end().as_bytes());
        out.push(b'\n');
    }
    NormalizedRaw(out)
}

/// FNV-1a 64 — offset basis 0xcbf29ce484222325, prime 0x100000001b3.
pub fn src_sha(raw: &NormalizedRaw) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in &raw.0 {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub struct GoldenHead {
    pub id: String,
    pub sha: u64,
    pub provenance: String,
    pub body: String,
}

pub fn parse_golden(text: &str) -> Result<GoldenHead, String> {
    let mut lines = text.split('\n');
    let magic = lines.next().unwrap_or("");
    if magic != "## netlab-golden v1" {
        return Err("bad golden magic".to_string());
    }
    let mut id = String::new();
    let mut sha = 0u64;
    let mut provenance = String::new();
    for line in lines.by_ref() {
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("## id: ") {
            id = v.to_string();
        } else if let Some(v) = line.strip_prefix("## src-sha: ") {
            sha = u64::from_str_radix(v, 16)
                .map_err(|_| format!("bad src-sha `{v}`"))?;
        } else if let Some(v) = line.strip_prefix("## provenance: ") {
            provenance = v.to_string();
        }
    }
    let body: Vec<&str> = lines.collect();
    let mut body = body.join("\n");
    while body.ends_with('\n') {
        body.pop();
    }
    Ok(GoldenHead {
        id,
        sha,
        provenance,
        body,
    })
}

pub fn render_golden(case: &Case, sha: u64, body: &str) -> String {
    format!(
        "## netlab-golden v1\n## id: {}\n## src-sha: {sha:016x}\n## provenance: {}\n## generated-by: NETLAB_GOLDEN=write cargo test\n## do-not-edit\n\n{}\n",
        case.id, case.provenance, body
    )
}

pub struct Diff {
    pub report: String,
}

pub fn compare(golden: &str, actual: &str) -> Result<(), Diff> {
    if golden == actual {
        return Ok(());
    }
    let g: Vec<&str> = golden.split('\n').collect();
    let a: Vec<&str> = actual.split('\n').collect();
    let mut pre = 0;
    while pre < g.len().min(a.len()) && g[pre] == a[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < (g.len() - pre).min(a.len() - pre) && g[g.len() - 1 - suf] == a[a.len() - 1 - suf] {
        suf += 1;
    }
    let mut r = String::from("--- golden\n+++ actual\n");
    r.push_str(&format!(
        "@@ -{} +{} @@\n",
        pre + 1,
        pre + 1
    ));
    for l in &g[pre..g.len() - suf] {
        r.push_str(&format!("-{l}\n"));
    }
    for l in &a[pre..a.len() - suf] {
        r.push_str(&format!("+{l}\n"));
    }
    Err(Diff { report: r })
}

fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// `{"t":"gap"}` folds the marker into its bound line; a `## script`
/// block collapses to ONE script step carrying the verbatim text.
fn compiled_steps(case: &Case) -> String {
    let mut out = String::new();
    for step in &case.steps {
        if !out.is_empty() {
            out.push(',');
        }
        match step {
            Step::Exec { line, gap } => match gap {
                Some(g) => out.push_str(&format!(
                    "{{\"t\":\"gap\",\"fr\":\"{}\",\"line\":\"{}\",\"tok\":{}}}",
                    g.fr,
                    json_escape(line),
                    g.tok.map(|i| i.to_string()).unwrap_or_else(|| "null".into())
                )),
                None => out.push_str(&format!(
                    "{{\"t\":\"exec\",\"line\":\"{}\"}}",
                    json_escape(line)
                )),
            },
            Step::Script(text) => out.push_str(&format!(
                "{{\"t\":\"script\",\"text\":\"{}\"}}",
                json_escape(text)
            )),
            Step::Key { op, text } => out.push_str(&format!(
                "{{\"t\":\"key\",\"op\":\"{}\",\"text\":\"{}\"}}",
                op.name(),
                json_escape(text)
            )),
            Step::Tick(n) => out.push_str(&format!("{{\"t\":\"tick\",\"n\":{n}}}")),
            Step::Drain => out.push_str("{\"t\":\"drain\"}"),
            Step::AllowEvents => out.push_str("{\"t\":\"allow-events\"}"),
        }
    }
    out
}

/// One record per corpus case — written only under NETLAB_GOLDEN=write.
pub fn emit_steps(cases: &[(&Case, u64, &str)]) -> Result<String, String> {
    let mut out = String::from("[\n");
    for (i, (case, sha, body)) in cases.iter().enumerate() {
        let masks: Vec<&str> = case
            .masks
            .iter()
            .map(|m| match m {
                Mask::Uptime => "uptime",
                Mask::Tick => "tick",
            })
            .collect();
        let masks = masks
            .iter()
            .map(|m| format!("\"{m}\""))
            .collect::<Vec<_>>()
            .join(",");
        let frames: Vec<String> = body
            .split('\n')
            .map(|l| format!("\"{}\"", json_escape(l)))
            .collect();
        out.push_str(&format!(
            "  {{\"id\":\"{}\",\"src-sha\":\"{sha:016x}\",\"masks\":[{}],\"steps\":[{}],\"frames\":[{}]}}",
            case.id,
            masks,
            compiled_steps(case),
            frames.join(",")
        ));
        if i + 1 < cases.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("]\n");
    let path = PathBuf::from(STEPS_JSON);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{STEPS_JSON}: mkdir: {e}"))?;
    }
    fs::write(&path, &out).map_err(|e| format!("{STEPS_JSON}: write: {e}"))?;
    Ok(out)
}

pub fn corpus_files() -> Vec<PathBuf> {
    let mut raws: Vec<PathBuf> = fs::read_dir(CORPUS_DIR)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == "raw").unwrap_or(false))
                .collect()
        })
        .unwrap_or_default();
    raws.sort();
    raws
}

/// `.golden` files with no sibling `.raw` — a deleted case leaves a stale
/// oracle that silently stops being checked.
pub fn orphan_goldens() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(CORPUS_DIR)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map(|x| x == "golden").unwrap_or(false))
                .filter(|p| !p.with_extension("raw").exists())
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}
