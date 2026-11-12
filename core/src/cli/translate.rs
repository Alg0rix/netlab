//! Startup-script translate — one entry point for applyStartup, paste,
//! and reload replay. Lines pass VERBATIM except the implicit prelude:
//! mode-lattice kinds get a synthesized `enable` + `configure terminal`
//! pair at the head; leading prelude-equivalent lines collapse into it
//! (TRANSLATE-PRELUDE-DEDUP). Returns (line, src_lineno) pairs — the
//! synthesized prelude carries src_lineno = 0.

use super::matcher::{match_g, Outcome};
use super::token::tokenize;
use super::tree;
use super::types::ModeId;
use crate::device::DeviceKind;

/// Is this line prelude-equivalent — does it resolve to Act::Mode(Enable)
/// or Act::Mode(Configure) with no residual argv? The matcher owns case
/// folding and unique-prefix, so `en`/`conf t`/`CONFIGURE TERMINAL` all
/// collapse.
fn is_prelude(kind: DeviceKind, line: &str, ifmap: &super::ifmap::IfMap) -> bool {
    let toks = tokenize(line);
    for (mode, want) in [
        (ModeId::Exec, "e"),
        (ModeId::PrivExec, "c"),
    ] {
        let (out, _) = match_g(tree::root(kind, mode), &toks, ifmap);
        if let Outcome::Hit(c) = out {
            let hit = matches!(
                c.tag,
                super::types::ActTag::Enable if want == "e"
            ) || matches!(
                c.tag,
                super::types::ActTag::Configure if want == "c"
            );
            if hit {
                return true;
            }
        }
    }
    false
}

pub fn translate(kind: DeviceKind, text: &str, ifmap: &super::ifmap::IfMap) -> Vec<(String, u32)> {
    let modeful = matches!(kind, DeviceKind::Router | DeviceKind::Switch | DeviceKind::Ext);
    let mut lines: Vec<(String, u32)> = text
        .replace('\r', "")
        .split('\n')
        .enumerate()
        .filter_map(|(i, l)| {
            let t = l.to_string();
            let s = t.trim_start();
            if s.is_empty() || s.starts_with('!') || s.starts_with('#') {
                None
            } else {
                Some((t, (i + 1) as u32))
            }
        })
        .collect();

    if !modeful {
        return lines;
    }

    let mut head = 0usize;
    while head < lines.len() && is_prelude(kind, &lines[head].0, ifmap) {
        head += 1;
    }
    lines.drain(0..head);
    let mut out = vec![
        ("enable".to_string(), 0),
        ("configure terminal".to_string(), 0),
    ];
    out.extend(lines);
    out
}
