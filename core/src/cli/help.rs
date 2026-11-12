//! `?` / Tab — read the CURRENT mode root only, never walk up.
//! Rows: candidates at the position (hidden filtered, prefix-filtered by
//! a partial last token). Inline: `<cr>` when the cursor sits after a
//! completed leaf. Mixed leaf+kids positions put `<cr>` first.

use super::ifmap::IfMap;
use super::matcher;
use super::token::tokenize;
use super::tree;
use super::types::{CompleteResult, GNode, HelpResult, HelpRow, Tok, Token};
use crate::device::Device;

fn placeholder(k: &GNode) -> Option<String> {
    match k.tok {
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

/// One step of literal-or-value matching for `?`/Tab descents. A typed
/// token reaches a Lit kid on exact OR unique-prefix; other kinds use
/// the matcher's own value check.
fn step<'a>(cur: &'a [GNode], toks: &[Token], i: usize, ifmap: &IfMap) -> Option<(&'a GNode, usize)> {
    let t = toks[i].text.to_lowercase();
    let mut lit: Vec<&GNode> = cur
        .iter()
        .filter(|k| match k.tok {
            Tok::Lit(l) => l == t || (!t.is_empty() && l.starts_with(t.as_str())),
            _ => false,
        })
        .collect();
    if let Some(e) = lit.iter().find(|k| matches!(k.tok, Tok::Lit(l) if l == t)) {
        return Some((e, 1));
    }
    if lit.len() > 1 {
        return None; // ambiguous — caller reports Err at this token
    }
    if let Some(k) = lit.pop() {
        return Some((k, 1));
    }
    for k in cur {
        if matches!(k.tok, Tok::Lit(_)) {
            continue;
        }
        if let Some((used, _)) = matcher::match_value(k, toks, i, ifmap) {
            return Some((k, used));
        }
    }
    None
}

/// Descend `root` through `toks`. Returns the terminal kids slice plus
/// the last node consumed (or Err(token index) on a stall).
fn descend_to<'a>(
    root: &'a [GNode],
    toks: &[Token],
    ifmap: &IfMap,
) -> Result<(&'a [GNode], Option<&'a GNode>), usize> {
    let mut cur = root;
    let mut last: Option<&GNode> = None;
    let mut i = 0;
    while i < toks.len() {
        match step(cur, toks, i, ifmap) {
            Some((k, used)) => {
                i += used;
                last = Some(k);
                cur = k.kids;
            }
            None => return Err(i),
        }
    }
    Ok((cur, last))
}

fn rows_for(kids: &[GNode], prefix: Option<&str>) -> Vec<HelpRow> {
    kids.iter()
        .filter(|k| !k.hidden)
        .filter_map(|k| {
            let word = placeholder(k)?;
            if let Some(p) = prefix {
                match word.get(..p.len()) {
                    Some(w) if w.eq_ignore_ascii_case(p) => {}
                    _ => return None,
                }
            }
            Some(HelpRow { word, help: k.help.map(|s| s.to_string()) })
        })
        .collect()
}

/// `?` on textBeforeCursor.
pub fn help(cli: &super::Cli, d: &Device, before_cursor: &str, ifmap: &IfMap) -> HelpResult {
    let prompt = cli.current_prompt(d);
    let root = tree::root(d.kind, cli.node);
    let trailing_ws = before_cursor
        .chars()
        .last()
        .map(|c| c == ' ' || c == '\t')
        .unwrap_or(false);
    let toks = tokenize(before_cursor);

    if toks.is_empty() {
        return HelpResult::Rows { rows: rows_for(root, None), prompt };
    }

    let (complete, partial): (&[Token], Option<String>) = if trailing_ws {
        (toks.as_slice(), None)
    } else {
        (&toks[..toks.len() - 1], Some(toks[toks.len() - 1].text.clone()))
    };

    match descend_to(root, complete, ifmap) {
        Err(i) => HelpResult::Err { index: toks[i].start, prompt },
        Ok((cur, last)) => {
            if partial.is_none() {
                match last {
                    Some(k) if k.kids.is_empty() && k.act.is_some() => {
                        HelpResult::Inline { inline: vec!["<cr>".into()], prompt }
                    }
                    Some(k) if k.act.is_some() && !k.kids.is_empty() => {
                        let mut rows = vec![HelpRow { word: "<cr>".into(), help: None }];
                        rows.extend(rows_for(cur, None));
                        HelpResult::Rows { rows, prompt }
                    }
                    _ => HelpResult::Rows { rows: rows_for(cur, None), prompt },
                }
            } else {
                HelpResult::Rows {
                    rows: rows_for(cur, partial.as_deref()),
                    prompt,
                }
            }
        }
    }
}

/// Tab on textBeforeCursor — the LCP-extended replacement, a list, or
/// Err(byte offset of the failing token).
pub fn complete(cli: &super::Cli, d: &Device, before_cursor: &str, ifmap: &IfMap) -> CompleteResult {
    let root = tree::root(d.kind, cli.node);
    let trailing_ws = before_cursor
        .chars()
        .last()
        .map(|c| c == ' ' || c == '\t')
        .unwrap_or(false);
    let toks = tokenize(before_cursor);
    if toks.is_empty() || trailing_ws {
        return CompleteResult::Err { index: before_cursor.len() };
    }
    let (complete, last) = (&toks[..toks.len() - 1], toks[toks.len() - 1].text.clone());
    match descend_to(root, complete, ifmap) {
        Err(i) => CompleteResult::Err { index: complete[i].start },
        Ok((cur, _)) => {
            let p = last.to_lowercase();
            let cands: Vec<String> = cur
                .iter()
                .filter(|k| !k.hidden)
                .filter_map(placeholder)
                .filter(|w| w.to_lowercase().starts_with(p.as_str()))
                .collect();
            match cands.len() {
                0 => CompleteResult::Err { index: toks[toks.len() - 1].start },
                1 => {
                    let word = cands.into_iter().next().unwrap();
                    let head: String = complete
                        .iter()
                        .map(|t| t.text.clone())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let line = if head.is_empty() {
                        word
                    } else {
                        format!("{head} {word}")
                    };
                    CompleteResult::Line { line }
                }
                _ => CompleteResult::List { list: cands },
            }
        }
    }
}
