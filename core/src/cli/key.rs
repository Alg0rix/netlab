//! Key handling — ctrlz/ctrlc.
//!   ctrlz: drain an armed pending first (silent abort — no continuation
//!     runs), then apply the modes::end guard (config-family only); at
//!     Exec/PrivExec it is a no-op. Returns out: [] in EVERY seat — no
//!     scrollback line; only `prompt` may change.
//!   ctrlc: cli.pending = None, Op text "^C", mode prompt.

use super::types::{ModeId, Op};
use super::Cli;
use crate::device::Device;
use crate::events::SimEvent;

pub fn key(
    cli: &mut Cli,
    d: &mut Device,
    which: &str,
    _tick: u64,
    _events: &mut [SimEvent],
) -> super::types::CliResult {
    match which {
        "ctrlz" => {
            if cli.pending.take().is_some() {
                // silent abort — no continuation, no output
            }
            match cli.node {
                ModeId::Config | ModeId::ConfigIf | ModeId::ConfigVlan
                | ModeId::ConfigRouter => {
                    cli.node = ModeId::PrivExec;
                    cli.ctx = Default::default();
                }
                _ => {}
            }
            super::types::CliResult {
                prompt: cli.current_prompt(d),
                out: Vec::new(),
                mutated: false,
                pending: cli.pending_view(),
            }
        }
        "ctrlc" => {
            cli.pending = None;
            super::types::CliResult {
                prompt: cli.current_prompt(d),
                out: vec![Op::text("^C")],
                mutated: false,
                pending: cli.pending_view(),
            }
        }
        _ => super::types::CliResult {
            prompt: cli.current_prompt(d),
            out: Vec::new(),
            mutated: false,
            pending: cli.pending_view(),
        },
    }
}
