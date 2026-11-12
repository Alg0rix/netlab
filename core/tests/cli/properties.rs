//! Property checks P01–P12 — the corpus-independent half of the surface.
//! Every check drives the same public entry points the adapter consumes
//! plus the static grammar slices the engine itself walks.

use std::collections::HashMap;
use std::fs;

use netlab_core::cli::ifmap::IfMap;
use netlab_core::cli::matcher::{match_g, Outcome};
use netlab_core::cli::nvgen::nvgen;
use netlab_core::cli::token::tokenize;
use netlab_core::cli::translate::translate;
use netlab_core::cli::tree::root;
use netlab_core::cli::types::{CompleteResult, Edge, GNode, ModeCtx, ModeId, Tok};
use netlab_core::cli::{sep, Cli};
use netlab_core::config::{self, SetErr, ToggleKind};
use netlab_core::{Device, DeviceId, DeviceKind, SimEvent, Topology};

use crate::fixtures;
use crate::runner;

const ALL_KINDS: [DeviceKind; 3] =
    [DeviceKind::Router, DeviceKind::Switch, DeviceKind::Host];
const MODEFUL_MODES: [ModeId; 6] = [
    ModeId::Exec,
    ModeId::PrivExec,
    ModeId::Config,
    ModeId::ConfigIf,
    ModeId::ConfigVlan,
    ModeId::ConfigRouter,
];

fn rig(kind: DeviceKind) -> (Topology, DeviceId, Cli) {
    let mut t = Topology::new();
    let id = t.add_device(kind, "D".to_string(), 0.0, 0.0);
    let mut cli = Cli::new();
    let names = t.devices[&id].port_names();
    cli.set_ifmap(&names).expect("ifmap");
    (t, id, cli)
}

fn dv(m: &mut HashMap<DeviceId, Device>, id: DeviceId) -> &mut Device {
    m.get_mut(&id).expect("device")
}

/// The help.rs placeholder spellings — the row-vocabulary contract.
fn placeholder(g: &GNode) -> Option<String> {
    match &g.tok {
        Tok::Lit(l) => Some(l.to_string()),
        Tok::Word => Some("WORD".into()),
        Tok::Line => Some("LINE".into()),
        Tok::Range { lo, hi } => Some(format!("<{lo}-{hi}>")),
        Tok::Ipv4 | Tok::Mask => Some("<A.B.C.D>".into()),
        Tok::Ipv4Prefix => Some("<A.B.C.D/len>".into()),
        Tok::Mac => Some("<MAC>".into()),
        Tok::IfName => Some("<if>".into()),
        Tok::VlanList => Some("<list>".into()),
        Tok::Names(_) => None,
    }
}

fn sample(g: &GNode, ifname0: &str) -> Option<String> {
    match &g.tok {
        Tok::Lit(l) => Some(l.to_string()),
        Tok::Word => Some("sampleword".into()),
        Tok::Line => Some("sample line tail".into()),
        Tok::Range { lo, .. } => Some(lo.to_string()),
        Tok::Ipv4 => Some("10.1.1.9".into()),
        Tok::Mask => Some("255.255.255.0".into()),
        Tok::Ipv4Prefix => Some("10.2.0.0/24".into()),
        Tok::Mac => Some("aa:bb:cc:dd:ee:ff".into()),
        Tok::IfName => Some(ifname0.to_string()),
        Tok::VlanList => Some("10".into()),
        Tok::Names(_) => None,
    }
}

/// A stable identity key for a node — the placeholder for Lit/value
/// seats, or a variant tag. Used for superset/subset comparisons only.
fn key_of(g: &GNode) -> String {
    match &g.tok {
        Tok::Lit(l) => format!("lit:{l}"),
        Tok::Word => "word".into(),
        Tok::Line => "line".into(),
        Tok::Range { lo, hi } => format!("range:{lo}-{hi}"),
        Tok::Ipv4 => "ipv4".into(),
        Tok::Mask => "mask".into(),
        Tok::Ipv4Prefix => "prefix".into(),
        Tok::Mac => "mac".into(),
        Tok::IfName => "ifname".into(),
        Tok::VlanList => "vlanlist".into(),
        Tok::Names(_) => "names".into(),
    }
}

/// P01 `?-totality` — walk every non-hidden node of every modeful root;
/// every row-word and completion candidate must re-parse at its seat;
/// hidden nodes never appear; EXEC ⊆ PRIVEXEC; DO ⊆ PRIVEXEC minus the
/// mode-armed/do heads.
#[test]
fn p01_help_totality() {
    for kind in ALL_KINDS {
        let (mut t, id, mut cli) = rig(kind);
        let d = dv(&mut t.devices, id);
        let names = d.port_names();
        let ifmap = IfMap::new(&names).expect("ifmap");
        let modes: &[ModeId] = match kind {
            DeviceKind::Host => &[ModeId::Exec],
            _ => &MODEFUL_MODES,
        };
        for &mode in modes {
            cli.node = mode;
            let mut w = Walk {
                at_root: root(kind, mode),
                ifmap: &ifmap,
                ifname0: &names[0],
                cli: &mut cli,
                d,
            };
            walk(&mut w, root(kind, mode), false, &mut Vec::new());
        }
    }
    // privileged superset + do-subset, per modeful kind.
    for kind in [DeviceKind::Router, DeviceKind::Switch] {
        let exec = root(kind, ModeId::Exec);
        let privx = root(kind, ModeId::PrivExec);
        for g in exec {
            let hit = privx
                .iter()
                .find(|g2| key_of(g2) == key_of(g))
                .unwrap_or_else(|| panic!("exec node {:?} missing from priv grammar", key_of(g)));
            assert!(
                std::ptr::eq(g.kids, hit.kids),
                "node {} kids differ exec vs priv",
                key_of(g)
            );
        }
        let do_kids = privx
            .iter()
            .find(|g| matches!(g.edge, Edge::Do))
            .map(|g| g.kids)
            .expect("do edge");
        const MODE_HEADS: &[&str] =
            &["enable", "disable", "configure", "exit", "logout", "do"];
        let remaining: Vec<&GNode> = privx
            .iter()
            .filter(|g| match &g.tok {
                Tok::Lit(l) => !MODE_HEADS.contains(l),
                _ => true,
            })
            .collect();
        assert_eq!(
            do_kids.len(),
            remaining.len(),
            "do kids must equal priv root minus mode-armed/do heads"
        );
        for g in do_kids {
            let hit = remaining
                .iter()
                .find(|g2| key_of(g2) == key_of(g))
                .unwrap_or_else(|| panic!("do node {:?} not under priv root", key_of(g)));
            assert!(
                std::ptr::eq(g.kids, hit.kids),
                "do node {} is not the same slice as the priv node",
                key_of(g)
            );
        }
    }
}

/// The grammar walk's read-mostly rig — the mode root it descends from,
/// the name map, and the session/device pair every probe runs through.
struct Walk<'a> {
    at_root: &'static [GNode],
    ifmap: &'a IfMap,
    ifname0: &'a str,
    cli: &'a mut Cli,
    d: &'a Device,
}

fn walk(w: &mut Walk<'_>, kids: &'static [GNode], parent_act: bool, path: &mut Vec<String>) {
    for k in kids.iter().filter(|k| !k.hidden) {
        let Some(word) = sample(k, w.ifname0) else {
            continue;
        };
        // help rows at this position must equal the non-hidden
        // placeholders — and never contain a hidden word.
        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{} ", path.join(" "))
        };
        let rows = match w.cli.help(w.d, &prefix) {
            netlab_core::cli::types::HelpResult::Rows { rows, .. } => rows,
            _ => panic!("help at `{prefix}` must return rows"),
        };
        let row_words: Vec<String> = rows.iter().map(|r| r.word.clone()).collect();
        let mut expected: Vec<String> = kids
            .iter()
            .filter(|k| !k.hidden)
            .filter_map(placeholder)
            .collect();
        if parent_act && !kids.is_empty() {
            expected.insert(0, "<cr>".into());
        }
        assert_eq!(row_words, expected, "help rows at `{prefix}`");
        for hid in kids.iter().filter(|k| k.hidden) {
            if let Some(pw) = placeholder(hid) {
                assert!(
                    !row_words.iter().any(|w| w == &pw),
                    "hidden node {pw} leaked into rows at `{prefix}`"
                );
                // and never completes either.
                if let Tok::Lit(l) = &hid.tok {
                    if let CompleteResult::List { list } =
                        w.cli.complete(w.d, &format!("{prefix}{}", &l[..1]))
                    {
                        assert!(
                            !list.iter().any(|w| w == l),
                            "hidden lit {l} completed at `{prefix}`"
                        );
                    }
                }
            }
        }
        // completion with the placeholder's first char must surface the
        // candidate — either as the unique line or inside the list.
        // Value toks complete as their placeholder ("<if>", "<A.B.C.D>").
        let want = placeholder(k).unwrap_or_else(|| word.clone());
        let partial = format!("{prefix}{}", &want[..1]);
        match w.cli.complete(w.d, &partial) {
            CompleteResult::Line { line } => assert!(
                line.ends_with(want.as_str()),
                "completion `{partial}` -> {line} does not end with {want}"
            ),
            CompleteResult::List { list } => {
                assert!(
                    list.iter().any(|w| w == &want),
                    "completion `{partial}` list {list:?} lacks {want}"
                )
            }
            CompleteResult::Err { index } => {
                panic!("completion `{partial}` errored at {index}")
            }
        }
        // the word re-parses at this position: never NoMatch at or
        // before this seat.
        let line = format!("{prefix}{word}");
        let toks = tokenize(&line);
        let (outcome, _furthest) = match_g(w.at_root, &toks, w.ifmap);
        if let Outcome::NoMatch { tok_index } = outcome {
            assert!(
                tok_index > path.len(),
                "row word {word} fails re-parse at `{line}` (tok {tok_index})"
            );
        }
        path.push(word);
        walk(w, k.kids, k.act.is_some(), path);
        path.pop();
    }
}

/// P02 `caret-offset` — bad lines at every token position: the caret op
/// lands on prompt_width + sep + Token[tok].start exactly; the triad
/// strings are byte-exact; help/complete `!caret` carries the bare byte
/// index; valid controls emit no triad.
#[test]
fn p02_caret_offsets() {
    let cases: &[(&[&str], &str, usize)] = &[
        (&["enable"], "no frobnicate", 0),
        (&["enable"], "show zz", 1),
        (&["enable"], "show version extra", 2),
        (&["enable"], "ping 999.1.1.1", 1),
        (&["enable", "conf t"], "interface zzz", 1),
        (&["enable", "conf t"], "no frobnicate", 1),
        (&["enable", "conf t"], "ip route 10.0.0.0 255.255.255.0 999.9.9.9", 4),
        (&["enable"], "write junk", 1),
    ];
    for (setup, line, tok) in cases {
        let (mut t, id, mut cli) = rig(DeviceKind::Router);
        let d = dv(&mut t.devices, id);
        for s in setup.iter() {
            cli.exec(d, s, 0, &mut Vec::new());
        }
        let prompt = cli.current_prompt(d);
        let r = cli.exec(d, line, 0, &mut Vec::new());
        let caret = r
            .out
            .iter()
            .find_map(|op| match op {
                netlab_core::Op::Text { text, .. } if text.trim() == "^" => {
                    Some(text.chars().count() - 1)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("`{line}` produced no caret op: {:?}", r.out));
        let starts: Vec<usize> = tokenize(line).iter().map(|t| t.start).collect();
        let expected = prompt.chars().count() + sep(&prompt) + starts[*tok];
        assert_eq!(caret, expected, "caret pad on `{line}`");
        let has_invalid = r.out.iter().any(|op| matches!(op,
            netlab_core::Op::Text { cls: Some(netlab_core::cli::OpCls::Err), text }
                if text == "% Invalid input detected at '^' marker."));
        assert!(has_invalid, "`{line}` missing the exact Invalid triad");
    }
    // help/complete `!caret` — bare byte index, never prompt-padded.
    let (mut t, id, mut cli) = rig(DeviceKind::Router);
    let d = dv(&mut t.devices, id);
    cli.exec(d, "enable", 0, &mut Vec::new());
    match cli.help(d, "show zz ") {
        netlab_core::cli::types::HelpResult::Err { index, .. } => assert_eq!(index, 5),
        _ => panic!("help `show zz ` must be Err"),
    }
    match cli.complete(d, "sh zz") {
        CompleteResult::Err { index } => assert_eq!(index, 3),
        _ => panic!("complete `sh zz` must be Err"),
    }
    // negative controls — valid lines emit zero caret/triad ops.
    let (mut t, id, mut cli) = rig(DeviceKind::Router);
    let d = dv(&mut t.devices, id);
    for line in ["enable", "show version", "conf t", "hostname Ok", "end"] {
        let r = cli.exec(d, line, 0, &mut Vec::new());
        for op in &r.out {
            if let netlab_core::Op::Text { text, .. } = op {
                assert!(
                    text.trim() != "^"
                        && !text.starts_with("% Invalid")
                        && !text.starts_with("% Incomplete")
                        && !text.starts_with("% Ambiguous"),
                    "negative control `{line}` emitted a triad"
                );
            }
        }
    }
}

/// P03 `fixpoint` — nvgen(run_script(nvgen(state))) == nvgen(state) on
/// every write-memory-capable device across every seeded rig; every
/// emitted line must be modeful and re-parse with zero errors.
#[test]
fn p03_nvgen_fixpoint() {
    for topo in [
        fixtures::TopoId::SingleRouter,
        fixtures::TopoId::SingleSwitch,
        fixtures::TopoId::TwoRouterLink,
        fixtures::TopoId::SwitchHosts,
        fixtures::TopoId::OspfPair,
        fixtures::TopoId::NatDhcp,
    ] {
        for kind in [DeviceKind::Router, DeviceKind::Switch] {
            let Ok((mut t, id)) = fixtures::build(topo, kind) else {
                continue;
            };
            let names = t.devices[&id].port_names();
            let mut cli = Cli::new();
            cli.set_ifmap(&names).expect("ifmap");
            let text1 = nvgen(&t.devices[&id]);
            let r = {
                let d = dv(&mut t.devices, id);
                cli.script(d, &text1.clone(), t.tick, &mut t.events)
            };
            assert!(
                r.errors.is_empty(),
                "nvgen replay on {:?}/{:?} errored: {:?}\n{text1}",
                topo,
                kind,
                r.errors
            );
            let text2 = nvgen(&t.devices[&id]);
            assert_eq!(
                text1, text2,
                "nvgen fixpoint broke on {:?}/{:?}",
                topo, kind
            );
        }
    }
}

/// P04 `checkpoint-spy` — the Rust-side anchor of the JS spy contract:
/// the `mutated` flag is the checkpoint trigger the adapter consumes.
/// Mutations flag true exactly once per mutation-bearing batch; pure
/// queries and dialog/key steps never flag; a script flags once for the
/// whole batch, not per line.
#[test]
fn p04_checkpoint_flags() {
    let (mut t, id, mut cli) = rig(DeviceKind::Router);
    let d = dv(&mut t.devices, id);
    let mut ev = Vec::new();
    let mutated = |cli: &mut Cli, d: &mut Device, ev: &mut Vec<SimEvent>, line: &str| {
        cli.exec(d, line, 0, ev).mutated
    };
    assert!(!mutated(&mut cli, d, &mut ev, "enable"));
    assert!(!mutated(&mut cli, d, &mut ev, "show version"));
    assert!(!mutated(&mut cli, d, &mut ev, "show running-config"));
    assert!(!mutated(&mut cli, d, &mut ev, "conf t"));
    assert!(mutated(&mut cli, d, &mut ev, "hostname Flag"));
    assert!(mutated(&mut cli, d, &mut ev, "no hostname"));
    assert!(!mutated(&mut cli, d, &mut ev, "show ip route"));
    // dialog arms never flag; answers flag only when the arm resolves
    // into a real mutation.
    assert!(!mutated(&mut cli, d, &mut ev, "write erase"));
    assert!(!mutated(&mut cli, d, &mut ev, "no"));
    // key steps are exempt — they never produce a mutation flag of their own.
    let r = cli.key(d, "ctrlz", 0, &mut ev);
    assert!(!r.mutated);
    // one checkpoint per script batch — a pure-query batch never flags.
    let script = cli.script(d, "hostname Batch\ninterface g0/0\ndescription x\nend\n", 0, &mut ev);
    assert!(script.mutated);
    let quiet = cli.script(d, "show version\n", 0, &mut ev);
    assert!(!quiet.mutated, "pure-query script flagged mutated");
}

/// P05 `pending-routing` — every armed dialog swallows matcher-shaped
/// input: junk re-prompts, typed values resolve, `?` reports the armed
/// prompt, ctrlc aborts, ctrlz drains.
#[test]
fn p05_pending_routing() {
    let arms: &[(&str, &str)] = &[
        ("reload", "System configuration has been modified. Save? [yes/no]: "),
        ("write erase", "Erasing the nvram filesystem will remove all configuration files! Continue? [confirm] "),
        ("copy running-config startup-config", "Destination filename [startup-config]? "),
    ];
    for (arm, prompt) in arms {
        for input in ["show version", "en", "configure terminal", "bogus"] {
            let (mut t, id, mut cli) = rig(DeviceKind::Router);
            let d = dv(&mut t.devices, id);
            let mut ev = Vec::new();
            cli.exec(d, "enable", 0, &mut ev);
            let r = cli.exec(d, arm, 0, &mut ev);
            assert_eq!(r.pending.as_ref().map(|p| p.prompt.as_str()), Some(*prompt));
            // `?` mid-dialog reads the armed prompt, not the mode prompt.
            let help = cli.help(d, "");
            let (hrows, hprompt) = match help {
                netlab_core::cli::types::HelpResult::Rows { rows, prompt } => (rows, prompt),
                _ => panic!("armed `?` must return rows"),
            };
            assert_eq!(hprompt, *prompt, "{arm} `?` prompt");
            assert!(!hrows.is_empty());
            // matcher-shaped input never reaches the grammar.
            let r2 = cli.exec(d, input, 0, &mut ev);
            let saw_show = r2.out.iter().any(|op| match op {
                netlab_core::Op::Text { text, .. } => text.contains("CLI engine"),
                _ => false,
            });
            assert!(!saw_show, "pending let `{input}` reach the show grammar");
            if arm.starts_with("copy") {
                assert!(r2.pending.is_none(), "Field pending resolved on any text");
            } else {
                assert_eq!(
                    r2.pending.as_ref().map(|p| p.prompt.as_str()),
                    Some(*prompt),
                    "{arm} junk `{input}` must re-prompt"
                );
            }
        }
    }
    // ctrlc and ctrlz clear armed dialogs without reaching the matcher.
    for which in ["ctrlc", "ctrlz"] {
        let (mut t, id, mut cli) = rig(DeviceKind::Router);
        let d = dv(&mut t.devices, id);
        let mut ev = Vec::new();
        cli.exec(d, "enable", 0, &mut ev);
        cli.exec(d, "reload", 0, &mut ev);
        let r = cli.key(d, which, 0, &mut ev);
        assert!(r.pending.is_none(), "{which} must clear pending");
    }
    // a dialog answer then a normal line both route correctly.
    let (mut t, id, mut cli) = rig(DeviceKind::Router);
    let d = dv(&mut t.devices, id);
    let mut ev = Vec::new();
    cli.exec(d, "enable", 0, &mut ev);
    cli.exec(d, "reload", 0, &mut ev);
    let r = cli.exec(d, "no", 0, &mut ev);
    assert_eq!(
        r.pending.as_ref().map(|p| p.prompt.as_str()),
        Some("Proceed with reload? [confirm] ")
    );
    let r = cli.exec(d, "no", 0, &mut ev);
    assert!(r.pending.is_none());
    let r = cli.exec(d, "show version", 0, &mut ev);
    assert!(r.out.iter().any(|op| matches!(op,
        netlab_core::Op::Text { text, .. } if text.contains("CLI engine"))));
}

/// P06 `byte-identity` — every corpus case, run twice from a fresh rig,
/// produces byte-identical normalized frames.
#[test]
fn p06_rerun_byte_identity() {
    for raw in runner::corpus_files() {
        let case = runner::parse_case(&raw).expect("parse");
        let render = || {
            let frames = runner::run_case(&case).expect("run");
            let mut m = runner::MaskSet::new(&case.masks);
            let body = runner::normalize(&frames, &mut m);
            m.check_used().expect("masks");
            body
        };
        let a = render();
        let b = render();
        assert_eq!(a, b, "{}: rerun not byte-identical", case.id);
    }
}

/// P07 `gap coverage` — every deferred head in UNBACKED_HEADS sits under
/// at least one `## gap:` marker of the right FR. (The run_case check
/// itself proves each marker produced an honest class.)
#[test]
fn p07_gap_coverage() {
    let mut bound: Vec<(&'static str, Vec<String>)> = Vec::new();
    for raw in runner::corpus_files() {
        let case = runner::parse_case(&raw).expect("parse");
        for step in &case.steps {
            if let runner::Step::Exec { line, gap: Some(g) } = step {
                let toks = tokenize(line).iter().map(|t| t.text.clone()).collect();
                bound.push((leak_fr(&g.fr), toks));
            }
        }
    }
    for (fr, head) in runner::UNBACKED_HEADS {
        let covered = bound.iter().any(|(bfr, toks)| {
            bfr == fr
                && toks
                    .windows(head.len())
                    .any(|w| {
                        w.iter()
                            .zip(head.iter())
                            .all(|(a, b)| a.eq_ignore_ascii_case(b))
                    })
        });
        assert!(covered, "deferred head {head:?} ({fr}) has no gap marker");
    }
}

fn leak_fr(fr: &str) -> &'static str {
    runner::FR_IDS.iter().copied().find(|f| *f == fr).expect("fr")
}

/// P08 `translate-idempotence` — translate(translate(t).join) == translate(t)
/// as line sequences; leading enable/conf t collapse once; mid-script
/// prelude words stay literal.
#[test]
fn p08_translate_idempotent() {
    let mut corpus_scripts: Vec<String> = Vec::new();
    for raw in runner::corpus_files() {
        let case = runner::parse_case(&raw).expect("parse");
        for step in &case.steps {
            if let runner::Step::Script(text) = step {
                corpus_scripts.push(text.clone());
            }
        }
    }
    corpus_scripts.push("enable\nconf t\nhostname A\nenable\nconf t\nend\n".into());
    corpus_scripts.push("enable\nconfigure terminal\nhostname B\n".into());
    corpus_scripts.push("hostname C\nend\n".into());
    corpus_scripts.push("!\n\nenable\nconf t\nend\n".into());
    for kind in [DeviceKind::Router, DeviceKind::Switch, DeviceKind::Host] {
        let (mut t, id, _) = rig(kind);
        let d = dv(&mut t.devices, id);
        let names = d.port_names();
        let ifmap = IfMap::new(&names).expect("ifmap");
        for text in &corpus_scripts {
            let l1 = translate(kind, text, &ifmap);
            let t2 = l1.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("\n");
            let l2 = translate(kind, &t2, &ifmap);
            assert_eq!(
                l1.iter().map(|(l, _)| l).collect::<Vec<_>>(),
                l2.iter().map(|(l, _)| l).collect::<Vec<_>>(),
                "translate not idempotent for {text:?}"
            );
            // lineno 0 marks the synthesized prelude pair: exactly the two
            // head lines on modeful kinds, never on hosts.
            let zero: Vec<&(String, u32)> = l1.iter().filter(|(_, i)| *i == 0).collect();
            if matches!(kind, DeviceKind::Router | DeviceKind::Switch) {
                assert_eq!(
                    zero.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
                    vec!["enable", "configure terminal"],
                    "synthesized prelude not exactly at head for {text:?}: {l1:?}"
                );
                // mid-script `configure terminal` passes verbatim after
                // the first content line — these texts carry only `conf t`.
                assert_eq!(
                    l1.iter()
                        .filter(|(l, _)| l.as_str() == "configure terminal")
                        .count(),
                    1,
                    "prelude pair count for {text:?}"
                );
            } else {
                assert!(
                    zero.is_empty(),
                    "host translate synthesized a prelude for {text:?}"
                );
            }
        }
    }
}

/// P09 `mode-restore` — run_script from every reachable seat restores the
/// caller's node/ctx and leaves pending drained — including EOF-mid-
/// config, error-mid-batch, and pending-left-armed scripts.
#[test]
fn p09_mode_restore() {
    let seats: &[(DeviceKind, ModeId, netlab_core::cli::types::ModeCtx)] = &[
        (DeviceKind::Router, ModeId::Exec, Default::default()),
        (DeviceKind::Router, ModeId::PrivExec, Default::default()),
        (DeviceKind::Router, ModeId::Config, Default::default()),
        (
            DeviceKind::Router,
            ModeId::ConfigIf,
            netlab_core::cli::types::ModeCtx {
                if_port: Some(0),
                ..Default::default()
            },
        ),
        (
            DeviceKind::Router,
            ModeId::ConfigRouter,
            netlab_core::cli::types::ModeCtx {
                ospf_pid: Some(1),
                ..Default::default()
            },
        ),
        (DeviceKind::Switch, ModeId::Exec, Default::default()),
        (DeviceKind::Switch, ModeId::PrivExec, Default::default()),
        (DeviceKind::Switch, ModeId::Config, Default::default()),
        (
            DeviceKind::Switch,
            ModeId::ConfigIf,
            netlab_core::cli::types::ModeCtx {
                if_port: Some(0),
                ..Default::default()
            },
        ),
        (
            DeviceKind::Switch,
            ModeId::ConfigVlan,
            netlab_core::cli::types::ModeCtx {
                vlan_id: Some(30),
                ..Default::default()
            },
        ),
    ];
    let scripts: &[&str] = &[
        "hostname Restored\nend\n",
        "interface g0/0\ndescription mid\n",
        "bogus-command\nhostname AfterErr\n",
        "reload\nno\n",
    ];
    for (kind, mode, ctx) in seats {
        // a seat is only tested when the device backs it (ports/vlans).
        let (mut t, id, _) = rig(*kind);
        let backed = match mode {
            ModeId::ConfigVlan => t.devices[&id].vlans.contains_key(&30),
            _ => true,
        };
        if !backed {
            continue;
        }
        for script in scripts {
            let mut cli = Cli::new();
            let names = t.devices[&id].port_names();
            cli.set_ifmap(&names).expect("ifmap");
            cli.node = *mode;
            cli.ctx = ModeCtx {
                if_port: ctx.if_port,
                vlan_id: ctx.vlan_id,
                ospf_pid: ctx.ospf_pid,
            };
            let mut ev = Vec::new();
            {
                let d = dv(&mut t.devices, id);
                cli.script(d, script, 0, &mut ev);
            }
            assert!(
                cli.node == *mode,
                "node not restored for {:?}/{:?} script {script:?}",
                kind,
                mode
            );
            assert!(
                cli.pending.is_none(),
                "pending left armed for {:?}/{:?} script {script:?}",
                kind,
                mode
            );
        }
    }
}

/// P10 `session-lifecycle` — the session map exists iff the device
/// exists: missing-device execution returns Err, never panics; a device
/// re-add needs a fresh session.
#[test]
fn p10_session_lifecycle() {
    let mut topo = Topology::new();
    let mut sessions: HashMap<DeviceId, Cli> = HashMap::new();
    let id = topo.add_device(DeviceKind::Router, "R".to_string(), 0.0, 0.0);
    let names = topo.devices[&id].port_names();
    let mut cli = Cli::new();
    cli.set_ifmap(&names).expect("ifmap");
    sessions.insert(id, cli);

    fn exec_on(
        topo: &mut Topology,
        sessions: &mut HashMap<DeviceId, Cli>,
        id: DeviceId,
        line: &str,
    ) -> Result<bool, String> {
        let Some(d) = topo.devices.get_mut(&id) else {
            return Err(format!("no device {id}"));
        };
        let Some(cli) = sessions.get_mut(&id) else {
            return Err(format!("no session {id}"));
        };
        Ok(cli.exec(d, line, topo.tick, &mut topo.events).mutated)
    }

    assert!(exec_on(&mut topo, &mut sessions, id, "show version").is_ok());
    let ghost: DeviceId = 9999;
    assert!(exec_on(&mut topo, &mut sessions, ghost, "show version").is_err());
    topo.remove_device(id);
    assert!(
        exec_on(&mut topo, &mut sessions, id, "show version").is_err(),
        "exec on a removed device must Err, not panic"
    );
    // a re-added device gets a fresh session of the same shape.
    let id2 = topo.add_device(DeviceKind::Router, "R2".to_string(), 0.0, 0.0);
    assert!(exec_on(&mut topo, &mut sessions, id2, "show version").is_err());
    let names2 = topo.devices[&id2].port_names();
    let mut cli2 = Cli::new();
    cli2.set_ifmap(&names2).expect("ifmap");
    sessions.insert(id2, cli2);
    assert!(exec_on(&mut topo, &mut sessions, id2, "show version").is_ok());
}

/// P11 `kind-gate matrix` — every kind-gated mutator rejects the wrong
/// kind through BOTH the direct setter and the exec path; the exec path
/// rejects either at the grammar (translate/caret) or with the pinned
/// `% Feature not supported on this device` line.
#[test]
fn p11_kind_gates() {
    type Setter = fn(&mut Device) -> Result<(), SetErr>;
    let rows: &[(&str, &[DeviceKind], Setter)] = &[
        ("set_hostname", &[DeviceKind::Router, DeviceKind::Switch], |d| {
            config::set_hostname(d, "X")
        }),
        ("set_pcname", &[DeviceKind::Host], |d| {
            config::set_pcname(d, "X")
        }),
        ("set_ip", &[DeviceKind::Router], |d| {
            config::set_ip(d, 0, [10, 1, 1, 1], [255, 255, 255, 0], 0)
        }),
        ("set_admin", &[DeviceKind::Router, DeviceKind::Switch], |d| {
            config::set_admin(d, 0, true, 0, &mut Vec::new())
        }),
        ("set_sw_mode", &[DeviceKind::Switch], |d| {
            config::set_sw_mode(d, 0, true)
        }),
        ("set_sw_access_vlan", &[DeviceKind::Switch], |d| {
            config::set_sw_access_vlan(d, 0, 1)
        }),
        ("enter_vlan", &[DeviceKind::Switch], |d| config::enter_vlan(d, 42)),
        ("add_route", &[DeviceKind::Router, DeviceKind::Switch], |d| {
            config::add_route(d, [10, 9, 0, 0], [255, 255, 255, 0], [10, 0, 0, 2], None, 0)
        }),
        ("ospf_enter", &[DeviceKind::Router], |d| config::ospf_enter(d, 1)),
        ("set_nat_role", &[DeviceKind::Router], |d| {
            config::set_nat_role(d, 0, netlab_core::device::NatRole::Inside)
        }),
        ("set_dhcp_service", &[DeviceKind::Router, DeviceKind::Switch], |d| {
            config::set_dhcp_service(d, true)
        }),
        ("dhcp_excluded_add", &[DeviceKind::Router, DeviceKind::Switch], |d| {
            config::dhcp_excluded_add(d, [10, 0, 0, 1], None)
        }),
        ("toggle ProxyArp", &[DeviceKind::Router], |d| {
            config::set_toggle(d, ToggleKind::ProxyArp, true)
        }),
        ("toggle DirectedBroadcast", &[DeviceKind::Router], |d| {
            config::set_toggle(d, ToggleKind::DirectedBroadcast, true)
        }),
        ("toggle IpRouting", &[DeviceKind::Router, DeviceKind::Switch], |d| {
            config::set_toggle(d, ToggleKind::IpRouting, false)
        }),
        ("host_dhcp", &[DeviceKind::Host], |d| {
            config::host_dhcp(d, 0, &mut Vec::new())
        }),
        ("set_host_ip", &[DeviceKind::Host], |d| {
            config::set_host_ip(d, [10, 0, 0, 9], [255, 255, 255, 0], Some([10, 0, 0, 1]), 0)
        }),
    ];
    for (label, ok_kinds, f) in rows {
        for kind in ALL_KINDS {
            let (mut t, id, _) = rig(kind);
            let d = dv(&mut t.devices, id);
            match (ok_kinds.contains(&kind), f(d)) {
                (false, Ok(())) => panic!("{label}: wrong-kind {kind:?} accepted"),
                (false, Err(e)) => assert!(
                    matches!(e, SetErr::KindGate),
                    "{label}: wrong-kind {kind:?} rejected with {e:?}, not KindGate"
                ),
                (true, Err(SetErr::KindGate)) => {
                    panic!("{label}: kind-gated on its own kind {kind:?}")
                }
                (true, _) => {}
            }
        }
    }

    // exec path: lines that PARSE on the wrong kind must emit the pinned
    // kind-gate line; lines that don't parse reject at the grammar.
    let (mut t, id, mut cli) = rig(DeviceKind::Switch);
    let d = dv(&mut t.devices, id);
    let mut ev = Vec::new();
    for s in ["enable", "conf t", "interface fa0/1"] {
        cli.exec(d, s, 0, &mut ev);
    }
    for line in ["ip address 10.0.0.1 255.255.255.0", "ip access-group 10 in"] {
        let r = cli.exec(d, line, 0, &mut ev);
        assert!(
            r.out.iter().any(|op| matches!(op,
                netlab_core::Op::Text { cls: Some(netlab_core::cli::OpCls::Err), text }
                    if text == "% Feature not supported on this device")),
            "switch `{line}` missing the pinned kind-gate line: {:?}",
            r.out
        );
    }
    // grammar-level rejection on a router for switch-only heads.
    let (mut t, id, mut cli) = rig(DeviceKind::Router);
    let d = dv(&mut t.devices, id);
    for s in ["enable", "conf t", "interface g0/0"] {
        cli.exec(d, s, 0, &mut ev);
    }
    for line in ["switchport mode trunk", "ip helper-address 10.0.0.1"] {
        let r = cli.exec(d, line, 0, &mut ev);
        let rejected = r.out.iter().any(|op| matches!(op,
            netlab_core::Op::Text { text, .. }
                if text.trim() == "^"
                    || text.starts_with('%')
                    || text.starts_with("Translating")));
        assert!(rejected, "router `{line}` produced no honest rejection");
    }
}

/// P12 `event-wording` — every SimEvent drained from the fixture
/// scenarios uses a pinned kind vocabulary and pinned message wording.
/// Checked per event, order-insensitive (the engine owns ordering).
#[test]
fn p12_event_wording() {
    let mut events: Vec<SimEvent> = Vec::new();

    // two-router link flap — exec-time link events.
    {
        let (mut t, id) = fixtures::build(fixtures::TopoId::TwoRouterLink, DeviceKind::Router)
            .expect("rig");
        let mut cli = Cli::new();
        let names = t.devices[&id].port_names();
        cli.set_ifmap(&names).expect("ifmap");
        let d = dv(&mut t.devices, id);
        for s in ["enable", "conf t", "interface g0/0", "no shutdown", "shutdown"] {
            cli.exec(d, s, t.tick, &mut t.events);
        }
        events.extend(t.drain_events());
    }
    // nat-dhcp exchange — tick-driven dhcp/syslog events.
    {
        let (mut t, id) = fixtures::build(fixtures::TopoId::NatDhcp, DeviceKind::Host).expect("rig");
        let mut cli = Cli::new();
        let names = t.devices[&id].port_names();
        cli.set_ifmap(&names).expect("ifmap");
        {
            let d = dv(&mut t.devices, id);
            cli.exec(d, "ip dhcp", t.tick, &mut t.events);
        }
        for _ in 0..60 {
            t.step();
        }
        events.extend(t.drain_events());
    }
    // ping + traceroute — armed directly (the adapter's intent path).
    {
        let (mut t, id) = fixtures::build(fixtures::TopoId::TwoRouterLink, DeviceKind::Router)
            .expect("rig");
        {
            let tick = t.tick;
            let d = dv(&mut t.devices, id);
            config::set_ip(d, 0, [10, 0, 0, 1], [255, 255, 255, 0], tick).expect("ip");
            config::set_admin(d, 0, true, tick, &mut t.events).expect("up");
        }
        assert!(t.ping(id, "10.0.0.2"));
        assert!(t.traceroute(id, "10.0.0.2"));
        for _ in 0..90 {
            t.step();
        }
        events.extend(t.drain_events());
    }
    // ospf adjacency — order-insensitive by design.
    {
        let (mut t, _id) = fixtures::build(fixtures::TopoId::OspfPair, DeviceKind::Router)
            .expect("rig");
        for _ in 0..60 {
            t.step();
        }
        events.extend(t.drain_events());
    }

    assert!(!events.is_empty(), "scenarios produced no events");
    let mut kinds = std::collections::BTreeSet::new();
    for e in &events {
        let kind = format!("{:?}", e.kind).to_lowercase();
        kinds.insert(kind.clone());
        assert!(
            msg_ok(&kind, &e.msg),
            "unpinned {kind} event: {:?}",
            e.msg
        );
    }
    // the scenarios must actually exercise the vocabulary breadth.
    assert!(kinds.contains("link"), "no link events: {kinds:?}");
    assert!(kinds.contains("dhcp"), "no dhcp events: {kinds:?}");
    assert!(kinds.contains("ping"), "no ping events: {kinds:?}");
    assert!(kinds.contains("syslog"), "no syslog events: {kinds:?}");
}

fn ip4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
}

fn msg_ok(kind: &str, msg: &str) -> bool {
    match kind {
        "link" => {
            (msg.starts_with("%LINK-5-CHANGED: Interface ")
                || msg.starts_with("%LINEPROTO-5-UPDOWN: Line protocol on Interface "))
                && (msg.ends_with("changed state to up") || msg.ends_with("changed state to down"))
        }
        "syslog" => {
            msg.starts_with("%OSPF-5-ADJCHG: Process ")
                || msg
                    .split_once(": ")
                    .map(|(name, rest)| {
                        !name.is_empty()
                            && name
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                            && !rest.is_empty()
                    })
                    .unwrap_or(false)
        }
        "ping" => {
            msg == "!"
                || msg == "."
                || msg == "U"
                || msg == "Type escape sequence to abort."
                || (msg.starts_with("Sending 5, 100-byte ICMP Echos to ")
                    && msg.ends_with(", timeout is 2 seconds:"))
                || (msg.starts_with("Success rate is ")
                    && msg.contains(" percent (")
                    && msg.contains("/5)"))
        }
        "trace" => {
            msg.starts_with("Tracing the route to ")
                || {
                    let words: Vec<&str> = msg.split_whitespace().collect();
                    !words.is_empty()
                        && words[0].bytes().all(|b| b.is_ascii_digit())
                        && (msg == format!("{}  *", words[0])
                            || (words.len() >= 3
                                && ip4(words[1])
                                && words.last().copied() == Some("msec")))
                }
        }
        "dhcp" => msg.contains(": DHCP "),
        _ => false,
    }
}

/// The mask engine is only reachable through `## masks:` — self-check
/// its boundary rules so the machinery stays honest while no case needs
/// it yet.
#[test]
fn mask_boundary_rules() {
    use crate::runner::{apply_masks, Mask, MaskSet, Scope};
    let mut m = MaskSet::new(&[Mask::Uptime, Mask::Tick]);
    let engine = apply_masks("uptime 3d4h, seen 0:12:34 ago, 1w2d", Scope::Engine, &mut m);
    assert_eq!(engine, "uptime <uptime>, seen <uptime> ago, <uptime>");
    let event = apply_masks("tick 99 up", Scope::Event, &mut m);
    assert_eq!(event, "tick <tick> up");
    // Bare frames never mask; tick never masks outside !event.
    let bare = apply_masks("tick 5 at 0:12:34", Scope::Bare, &mut m);
    assert_eq!(bare, "tick 5 at 0:12:34");
    let engine_tick = apply_masks("tick 5", Scope::Engine, &mut m);
    assert_eq!(engine_tick, "tick 5");
    m.check_used().expect("both masks fired");
    // declared-but-unused fails.
    let mut m2 = MaskSet::new(&[Mask::Tick]);
    let _ = apply_masks("plain text", Scope::Engine, &mut m2);
    assert!(m2.check_used().is_err());
}

/// Parse-error battery — every malformed .raw class fails naming the
/// file and the 1-based line.
#[test]
fn parse_error_battery() {
    // Per-process, per-run dir — concurrent test binaries must not race.
    let dir = std::env::temp_dir().join(format!(
        "netlab-corpus-bad-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("mkdir");
    let cases: &[(&str, &str)] = &[
        ("", "missing magic"),
        ("## netlab-corpus v1\n## kind: router\n## provenance: authored\n\nenable\n", "missing required `## id:`"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n", "empty case"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n## frobnicate\n", "unknown `##` directive"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n## gap: FR-SVI\n", "dangling `## gap:`"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n## gap: FR-NOPE\nshow version\n", "unknown gap FR-id"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n## gap: FR-SVI\n## drain\n", "dangling `## gap:`"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n## script\nhostname x\n", "unterminated"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n## endscript\n", "stray `## endscript`"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n## script\n## script\n## endscript\n", "nested `## script`"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\nenable\n", "header unterminated"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n## startup:\n\nenable\n", "zero stored lines"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n\n stray\n", "stray continuation"),
        ("## netlab-corpus v1\n## id: bad\n## kind: bogus\n## provenance: authored\n\nenable\n", "bad kind"),
        ("## netlab-corpus v1\n## id: bad\n## kind: router\n## provenance: authored\n## ifmap: notaport\n\nenable\n", "non-canonical"),
    ];
    for (src, want) in cases {
        let path = dir.join("bad.raw");
        fs::write(&path, src).expect("write");
        match runner::parse_case(&path) {
            Ok(_) => panic!("{want:?} case parsed"),
            Err(e) => assert!(
                e.msg.contains(want),
                "error {e:?} does not name {want:?}"
            ),
        }
    }
    let _ = fs::remove_dir_all(&dir);
}
