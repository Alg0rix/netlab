//! cli_script — run a pasted/startup script through the real session.
//! Per line: Op::Echo(prompt + " "·sep(prompt) + line) + that line's
//! ops. Script lines route through the same pending gate and never enter
//! cli.history. The synthesized enable/configure prelude is written for
//! the Exec seat, so the batch runs from Exec regardless of the caller's
//! mode; afterwards the saved (node, ctx) is restored and cli.pending
//! cleared.

use super::translate::translate;
use super::types::{ModeId, Op, OpCls, ScriptError, ScriptResult};
use super::{Cli, sep};
use crate::device::Device;
use crate::events::SimEvent;

pub fn run_script(
    cli: &mut Cli,
    d: &mut Device,
    text: &str,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> ScriptResult {
    let saved_node = cli.node;
    let saved_ctx = std::mem::take(&mut cli.ctx);
    cli.pending = None;
    cli.node = ModeId::Exec;
    let mut out: Vec<Op> = Vec::new();
    let mut errors = Vec::new();
    let mut mutated = false;

    for (line, src_lineno) in translate(d.kind, text, cli.ifmap_for(d)) {
        let prompt = cli.current_prompt(d);
        out.push(Op::Echo {
            text: format!("{}{}{}", prompt, " ".repeat(sep(&prompt)), line),
        });
        let r = cli.exec_line(d, &line, tick, events);
        mutated |= r.mutated;
        let err = r.out.iter().find_map(|op| {
            if let Op::Text { cls: Some(OpCls::Err), text } = op {
                Some(text.clone())
            } else {
                None
            }
        });
        if let Some(err) = err {
            errors.push(ScriptError { lineno: src_lineno, msg: err });
        }
        out.extend(r.out);
    }

    cli.node = saved_node;
    cli.ctx = saved_ctx;
    cli.pending = None;
    ScriptResult { out, mutated, errors }
}
