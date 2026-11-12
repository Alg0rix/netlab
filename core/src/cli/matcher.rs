//! Token matcher — the only place command text is parsed. Lit matching
//! is case-insensitive unique-prefix; captures preserve case. no/default/
//! do edges record neg/do_wrap rather than emitting argv.
//! (Spec name: match.rs — `match` is a Rust keyword, so matcher.rs.)

use std::net::Ipv4Addr;

use super::ifmap::IfMap;
use super::types::{Arg, Edge, GNode, NegKind, Tok, Token};

/// One successful path to a leaf.
#[derive(Debug)]
pub struct Cand {
    pub tag: super::types::ActTag,
    pub argv: Vec<Arg>,
    pub neg: Option<NegKind>,
    pub dw: bool,
}

pub enum Outcome {
    /// exactly one path reached a leaf
    Hit(Cand),
    /// input ran out mid-tree, or a node had kids but no leaf act
    Incomplete,
    /// >1 path reached a leaf
    Ambiguous,
    /// the furthest token index where matching stalled
    NoMatch { tok_index: usize },
}

fn parse_ipv4(t: &str) -> Option<Ipv4Addr> {
    t.parse::<Ipv4Addr>().ok()
}

fn parse_mac(t: &str) -> Option<[u8; 6]> {
    // aabb.ccdd.eeff | aa:bb:cc:dd:ee:ff | aa-bb-cc-dd-ee-ff
    let parts: Vec<&str> = if t.contains('.') {
        t.split('.')
            .filter(|s| s.len() == 4 && s.is_ascii())
            .flat_map(|s| [&s[..2], &s[2..]])
            .collect()
    } else if t.contains(':') || t.contains('-') {
        t.split([':', '-']).filter(|s| s.len() == 2).collect()
    } else {
        return None;
    };
    if parts.len() != 6 {
        return None;
    }
    let mut mac = [0u8; 6];
    for (i, s) in parts.iter().enumerate() {
        mac[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(mac)
}

fn parse_prefix(t: &str) -> Option<(Ipv4Addr, u8)> {
    let (ip, plen) = t.rsplit_once('/')?;
    let ip = parse_ipv4(ip)?;
    let plen: u8 = plen.parse().ok()?;
    if plen > 32 {
        return None;
    }
    Some((ip, plen))
}

/// Expanded vlan lists are memory-bounded — a 2000-entry `1-4094,…`
/// token is a NoMatch, not an allocation bomb.
const VLAN_LIST_MAX: usize = 256;

pub fn parse_vlan_list(t: &str) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    for part in t.split(',') {
        if let Some((a, b)) = part.split_once('-') {
            let (a, b): (u16, u16) = (a.parse().ok()?, b.parse().ok()?);
            if a == 0 || b > 4094 || a > b {
                return None;
            }
            out.extend(a..=b);
        } else {
            let v: u16 = part.parse().ok()?;
            if v == 0 || v > 4094 {
                return None;
            }
            out.push(v);
        }
        if out.len() > VLAN_LIST_MAX {
            return None;
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// A literal sibling's relation to the typed token (already case-folded
/// once by the caller).
#[derive(PartialEq)]
enum LitHit {
    No,
    Exact,
    Prefix,
}

fn lit_hit(l: &'static str, t: &str) -> LitHit {
    if l == t {
        LitHit::Exact
    } else if !t.is_empty() && l.starts_with(t) {
        LitHit::Prefix
    } else {
        LitHit::No
    }
}

/// Try one node at toks[i]; returns (tokens consumed, argv produced).
/// Lit ambiguity across siblings is resolved by the caller, not here.
pub(crate) fn match_value(node: &GNode, toks: &[Token], i: usize, ifmap: &IfMap) -> Option<(usize, Vec<Arg>)> {
    if i >= toks.len() {
        return None;
    }
    let t = &toks[i].text;
    match node.tok {
        Tok::Lit(_) => None, // handled by caller
        Tok::Word => Some((1, vec![Arg::Str(t.clone())])),
        Tok::Line => {
            let rest = super::token::rest_of_line(toks, i);
            if rest.is_empty() {
                None
            } else {
                Some((toks.len() - i, vec![Arg::Str(rest)]))
            }
        }
        Tok::Range { lo, hi } => {
            let v: u32 = t.parse().ok()?;
            (v >= lo && v <= hi).then(|| (1, vec![Arg::Num(v)]))
        }
        Tok::Ipv4 => parse_ipv4(t).map(|a| (1, vec![Arg::Ip(a)])),
        Tok::Mask => parse_ipv4(t).map(|a| (1, vec![Arg::Mask(a)])),
        Tok::Ipv4Prefix => parse_prefix(t).map(|(a, n)| (1, vec![Arg::Prefix(a, n)])),
        Tok::Mac => parse_mac(t).map(|m| (1, vec![Arg::Mac(m)])),
        Tok::IfName => ifmap
            .resolve(t, toks.get(i + 1).map(|x| x.text.as_str()))
            .map(|(port, used)| (used, vec![Arg::Port(port)])),
        Tok::VlanList => parse_vlan_list(t).map(|v| (1, vec![Arg::Vlans(v)])),
        Tok::Names(_) => None,
    }
}

/// Per-descent state — the token list, the alias map, and the furthest
/// token index reached (for NoMatch caret placement).
struct Ctx<'a> {
    toks: &'a [Token],
    ifmap: &'a IfMap,
    furthest: usize,
}

/// One descend call's collected outcomes.
struct Sink {
    cands: Vec<Cand>,
    saw_incomplete: bool,
}

/// One matched kid → continue the path at `j`. `args` are the value
/// captures the kid produced (empty for literals). argv accumulates on
/// ONE shared vector — a hit clones it once into the Cand; the branch's
/// pushes are truncated back before the next sibling.
fn step_in(
    m: &mut Ctx<'_>,
    k: &GNode,
    j: usize,
    st: (Option<NegKind>, bool),
    args: Vec<Arg>,
    argv: &mut Vec<Arg>,
    sink: &mut Sink,
) {
    let base = argv.len();
    if k.emits {
        if let Tok::Lit(l) = k.tok {
            argv.push(Arg::Str(l.to_string()));
        }
    }
    argv.extend(args);
    let neg = match k.edge {
        Edge::Neg(nk) => Some(nk),
        _ => st.0,
    };
    let dw = st.1 || matches!(k.edge, Edge::Do);
    if j >= m.toks.len() {
        if let Some(tag) = k.act {
            sink.cands.push(Cand {
                tag,
                argv: argv.clone(),
                neg,
                dw,
            });
        } else if !k.kids.is_empty() {
            sink.saw_incomplete = true;
        }
    } else {
        match descend(m, k.kids, j, (neg, dw), argv) {
            Outcome::Hit(c) => sink.cands.push(c),
            Outcome::Incomplete | Outcome::Ambiguous => sink.saw_incomplete = true,
            Outcome::NoMatch { .. } => {}
        }
    }
    argv.truncate(base);
}

/// Recursive matcher. `argv` accumulates captures from ancestors —
/// discriminating literals and value seats all ride the same positional
/// vector to the leaf. `st` = (neg, do_wrap) recorded by edges.
fn descend(
    m: &mut Ctx<'_>,
    kids: &[GNode],
    i: usize,
    st: (Option<NegKind>, bool),
    argv: &mut Vec<Arg>,
) -> Outcome {
    let (neg, dw) = st;
    if i > m.furthest {
        m.furthest = i;
    }
    if i >= m.toks.len() {
        return Outcome::Incomplete;
    }
    let t = m.toks[i].text.to_lowercase();

    let mut exact: Option<&GNode> = None;
    let mut prefixes: Vec<&GNode> = Vec::new();
    let mut values: Vec<(&GNode, usize, Vec<Arg>)> = Vec::new();
    for k in kids {
        match k.tok {
            Tok::Lit(l) => match lit_hit(l, t.as_str()) {
                LitHit::Exact => exact = Some(k),
                LitHit::Prefix => prefixes.push(k),
                LitHit::No => {}
            },
            _ => {
                if let Some((used, args)) = match_value(k, m.toks, i, m.ifmap) {
                    values.push((k, used, args));
                }
            }
        }
    }

    let mut lit_ambiguous = false;
    let lit_set: Vec<&GNode> = match (exact, prefixes.len()) {
        (Some(e), _) => vec![e],
        (None, 0) => Vec::new(),
        (None, 1) => vec![prefixes[0]],
        (None, _) => {
            lit_ambiguous = true;
            prefixes
        }
    };

    let mut sink = Sink {
        cands: Vec::new(),
        saw_incomplete: false,
    };
    for k in lit_set {
        step_in(m, k, i + 1, (neg, dw), Vec::new(), argv, &mut sink);
    }
    for (k, used, args) in values {
        step_in(m, k, i + used, (neg, dw), args, argv, &mut sink);
    }

    match sink.cands.len() {
        1 => Outcome::Hit(sink.cands.pop().unwrap()),
        n if n > 1 || lit_ambiguous => Outcome::Ambiguous,
        _ => {
            if sink.saw_incomplete {
                Outcome::Incomplete
            } else {
                Outcome::NoMatch { tok_index: i }
            }
        }
    }
}

/// Match the whole token list against a mode root.
pub fn match_g(kids: &[GNode], toks: &[Token], ifmap: &IfMap) -> (Outcome, usize) {
    let mut m = Ctx {
        toks,
        ifmap,
        furthest: 0,
    };
    let mut argv = Vec::new();
    let out = descend(&mut m, kids, 0, (None, false), &mut argv);
    (out, m.furthest)
}
