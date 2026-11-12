//! Mode lattice — prompts, parent edges, transitions. The parent table
//! is shared by walk-up rematch and `exit`.

use super::types::{ModeId, ModeOp, Op};
use super::Cli;
use crate::device::{Device, DeviceKind};

/// Parent edge for walk-up rematch and `exit`.
pub fn parent(node: ModeId) -> Option<ModeId> {
    match node {
        ModeId::ConfigIf | ModeId::ConfigVlan | ModeId::ConfigRouter => {
            Some(ModeId::Config)
        }
        ModeId::Config => Some(ModeId::PrivExec),
        ModeId::PrivExec => Some(ModeId::Exec),
        ModeId::Exec => None,
    }
}

/// Mode prompts never end with a space (sep = 1 under the
/// echo-separator rule). While a dialog is armed every prompt-bearing
/// payload returns the armed PendingView.prompt verbatim — callers use
/// Cli::current_prompt, which applies that rule.
pub fn mode_prompt(cli: &Cli, d: &Device) -> String {
    let n = &d.name;
    match cli.node {
        ModeId::Exec => format!("{n}>"),
        ModeId::PrivExec => format!("{n}#"),
        ModeId::Config => format!("{n}(config)#"),
        ModeId::ConfigIf => format!("{n}(config-if)#"),
        ModeId::ConfigVlan => format!("{n}(config-vlan)#"),
        ModeId::ConfigRouter => format!("{n}(config-router)#"),
    }
}

/// Lattice verbs are bound to the mode that owns them: `enable` only
/// at Exec, `disable`/`configure` only at PrivExec. A match walked up
/// from a sub-mode must not apply them — exec_line turns that hit into
/// the invalid-input triad.
pub fn admits(node: ModeId, op: ModeOp) -> bool {
    match op {
        ModeOp::Enable => node == ModeId::Exec,
        ModeOp::Disable | ModeOp::Configure => node == ModeId::PrivExec,
        _ => true,
    }
}

/// The session-close render for `exit`/`logout` at Exec/PrivExec.
fn session_close() -> Vec<Op> {
    vec![
        Op::text("con0 is now available"),
        Op::text("Press RETURN to get started."),
    ]
}

/// Apply a ModeOp. EnterVlan/EnterRouterOspf run their setter so the
/// SetErr (e.g. a conflicting OSPF pid) reaches the single mapping site.
pub fn transition(
    cli: &mut Cli,
    d: &mut Device,
    op: ModeOp,
    _tick: u64,
) -> Result<Vec<Op>, crate::config::SetErr> {
    if d.kind == DeviceKind::Host {
        // host sessions hold node = Exec for the session's whole life
        return Ok(Vec::new());
    }
    match op {
        ModeOp::Enable => {
            if cli.node == ModeId::Exec {
                cli.node = ModeId::PrivExec;
            }
            Ok(Vec::new())
        }
        ModeOp::Disable => {
            if cli.node == ModeId::PrivExec {
                cli.node = ModeId::Exec;
            }
            Ok(Vec::new())
        }
        ModeOp::Configure => {
            if cli.node == ModeId::PrivExec {
                cli.node = ModeId::Config;
            }
            Ok(Vec::new())
        }
        ModeOp::End => match cli.node {
            ModeId::Config | ModeId::ConfigIf | ModeId::ConfigVlan
            | ModeId::ConfigRouter => {
                cli.node = ModeId::PrivExec;
                cli.ctx = Default::default();
                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        },
        ModeOp::Exit => match cli.node {
            ModeId::Exec | ModeId::PrivExec => {
                cli.node = ModeId::Exec;
                cli.ctx = Default::default();
                Ok(session_close())
            }
            _ => {
                let p = parent(cli.node).unwrap_or(ModeId::Exec);
                cli.node = p;
                cli.ctx = Default::default();
                Ok(Vec::new())
            }
        },
        ModeOp::EnterIf(port) => {
            cli.node = ModeId::ConfigIf;
            cli.ctx.if_port = Some(port);
            Ok(Vec::new())
        }
        ModeOp::EnterVlan(id) => {
            crate::config::enter_vlan(d, id)?;
            cli.node = ModeId::ConfigVlan;
            cli.ctx.vlan_id = Some(id);
            Ok(Vec::new())
        }
        ModeOp::EnterRouterOspf(pid) => {
            crate::config::ospf_enter(d, pid)?;
            cli.node = ModeId::ConfigRouter;
            cli.ctx.ospf_pid = Some(pid);
            Ok(Vec::new())
        }
    }
}
