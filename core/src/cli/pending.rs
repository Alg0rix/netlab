//! Pending dialogs — built at MATCH time by bind(); dispatch only
//! assigns cli.pending. Every prompt ends with exactly one space
//! (builder invariant — the owned separator under the echo-separator
//! rule). Cli::exec routes to feed BEFORE the matcher while a dialog
//! is armed; the consumed line never enters cli.history.

use super::types::{Act, Intent, Op, PStep, Pending};
use super::Cli;

type StepRun<'a> = &'a mut dyn FnMut(&mut Cli, PStep) -> (Vec<Op>, bool);

fn copy_dest(dest: &str) -> PStep {
    if dest == "startup-config" {
        PStep::Act(Act::Intent(Intent::NvramWrite {
            text: String::new(),
        }))
    } else {
        PStep::Out(vec![Op::err(format!(
            "%Error opening nvram:{dest} (No such file or directory)"
        ))])
    }
}

pub fn copy_run_start() -> Pending {
    Pending::Field {
        prompt: "Destination filename [startup-config]? ".into(),
        default: Some("startup-config".into()),
        on_submit: copy_dest,
    }
}

pub fn write_erase() -> Pending {
    Pending::YesNo {
        prompt: "Erasing the nvram filesystem will remove all configuration files! Continue? [confirm] ".into(),
        default: Some(true),
        on_yes: Box::new(PStep::Act(Act::Intent(Intent::NvramErase))),
        on_no: None,
    }
}

pub fn reload_chain() -> Pending {
    Pending::YesNo {
        prompt: "System configuration has been modified. Save? [yes/no]: ".into(),
        default: Some(true),
        on_yes: Box::new(PStep::Seq(vec![
            PStep::Act(Act::Intent(Intent::NvramWrite {
                text: String::new(),
            })),
            PStep::Ask(proceed_reload()),
        ])),
        on_no: Some(Box::new(PStep::Ask(proceed_reload()))),
    }
}

fn proceed_reload() -> Pending {
    Pending::YesNo {
        prompt: "Proceed with reload? [confirm] ".into(),
        default: Some(true),
        on_yes: Box::new(PStep::Act(Act::Intent(Intent::Reload))),
        on_no: None,
    }
}

/// Answer lexicon for YesNo dialogs, case-folded.
fn yesno(line: &str) -> Option<bool> {
    match line.trim().to_lowercase().as_str() {
        "y" | "yes" => Some(true),
        "n" | "no" => Some(false),
        _ => None,
    }
}

/// Consume one line against an armed dialog. `run` executes a resolved
/// PStep — an Act dispatch or a chained Ask — so a single cli_exec can
/// end the dialog, arm the next, or emit continuation ops. Returns the
/// dialog's (ops, mutated) pair.
pub fn feed(p: Pending, line: &str, run: StepRun, cli: &mut Cli) -> (Vec<Op>, bool) {
    match p {
        Pending::YesNo {
            prompt,
            default,
            on_yes,
            on_no,
        } => {
            let answer = if line.trim().is_empty() {
                default
            } else {
                yesno(line)
            };
            match answer {
                Some(true) => {
                    cli.pending = None;
                    run(cli, *on_yes)
                }
                Some(false) => match on_no {
                    Some(step) => {
                        cli.pending = None;
                        run(cli, *step)
                    }
                    None => {
                        cli.pending = None;
                        (Vec::new(), false)
                    }
                },
                // blank-with-no-default or junk: re-prompt, no error line
                None => {
                    cli.pending = Some(Pending::YesNo {
                        prompt,
                        default,
                        on_yes,
                        on_no,
                    });
                    (Vec::new(), false)
                }
            }
        }
        Pending::Field {
            prompt,
            default,
            on_submit,
        } => {
            let typed = line.trim();
            let submitted = if typed.is_empty() {
                match default {
                    Some(v) => v,
                    // blank with no default: re-prompt, no error line
                    None => {
                        cli.pending = Some(Pending::Field {
                            prompt,
                            default: None,
                            on_submit,
                        });
                        return (Vec::new(), false);
                    }
                }
            } else {
                typed.to_string()
            };
            cli.pending = None;
            run(cli, on_submit(&submitted))
        }
    }
}
