//! IfMap — the interface-name authority. The adapter injects canonical
//! netlab names (g0/0, fa0/1, e0, eth0, vl1) in declared port order;
//! names[i] binds positionally to PortId(i). The IOS alias family is
//! generated from the type-word: g→gigabitethernet, fa→fastethernet,
//! e/eth→ethernet, vl→vlan (dormant, two-token).

use std::collections::{HashMap, HashSet};

use super::types::PortId;

#[derive(Debug, Clone, Default)]
pub struct IfMap {
    /// folded alias -> port
    names: HashMap<String, PortId>,
    /// ports in the vlan family — their aliases may split across two
    /// tokens ("vlan 1": type-word prefix, numeric suffix next)
    vlan_ports: HashSet<PortId>,
    len: usize,
}

/// The IOS type-word for a canonical short form, or None for non-canonical.
fn type_word(t: &str) -> Option<&'static str> {
    match t {
        "g" => Some("gigabitethernet"),
        "fa" => Some("fastethernet"),
        "e" | "eth" => Some("ethernet"),
        "vl" => Some("vlan"),
        _ => None,
    }
}

/// Byte length of a folded alias's alpha head — the numeric tail
/// (digits and '/') never joins the prefix match.
fn alpha_end(k: &str) -> usize {
    k.find(|c: char| c.is_ascii_digit()).unwrap_or(k.len())
}

impl IfMap {
    /// `names[i]` must be a canonical netlab name — anything else is a
    /// build defect (die class → Err).
    pub fn new(names: &[String]) -> Result<IfMap, String> {
        let mut map = IfMap {
            names: HashMap::new(),
            vlan_ports: HashSet::new(),
            len: names.len(),
        };
        for (i, raw) in names.iter().enumerate() {
            let name = raw.to_lowercase();
            let (t, idx) = name
                .find(|c: char| c.is_ascii_digit())
                .map(|p| name.split_at(p))
                .unwrap_or((name.as_str(), ""));
            if idx.is_empty()
                || !idx.chars().all(|c| c.is_ascii_digit() || c == '/')
            {
                return Err(format!("non-canonical interface name: {raw}"));
            }
            let tw = match type_word(t) {
                Some(tw) => tw,
                None => return Err(format!("non-canonical interface name: {raw}")),
            };
            if t == "vl" {
                map.vlan_ports.insert(i);
            }
            for k in 1..=tw.len() {
                map.names
                    .entry(format!("{}{}", &tw[..k], idx))
                    .or_insert(i);
            }
            map.names.entry(name).or_insert(i);
        }
        Ok(map)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Resolve the typed token. Returns (port, tokens-consumed): a
    /// vlan-family prefix consumes a second token carrying the suffix.
    /// `None` = no match (honest gap — never silently normalized).
    pub fn resolve(&self, tok: &str, next: Option<&str>) -> Option<(PortId, usize)> {
        let key = tok.to_lowercase();
        // a resolved alias maps to exactly one port — multi-port is the
        // ambiguous case the dup flags below detect
        if let Some(&port) = self.names.get(&key) {
            return Some((port, 1));
        }
        let mut tt_hit: Option<PortId> = None;
        let mut tt_dup = false;
        let mut p_hit: Option<PortId> = None;
        let mut p_dup = false;
        for (k, &p) in &self.names {
            if self.vlan_ports.contains(&p)
                && k[..alpha_end(k)].starts_with(key.as_str())
            {
                tt_dup |= tt_hit.map(|h| h != p).unwrap_or(false);
                tt_hit = Some(p);
            }
            if k.starts_with(key.as_str()) {
                p_dup |= p_hit.map(|h| h != p).unwrap_or(false);
                p_hit = Some(p);
            }
        }
        // a vlan type-word prefix takes its suffix from the next token,
        // and only when "vlan<sfx>" is this port's own alias
        if let Some(p) = tt_hit.filter(|_| !tt_dup) {
            if let Some(sfx) = next {
                if !sfx.is_empty()
                    && sfx.bytes().all(|b| b.is_ascii_digit())
                    && self.names.get(&format!("vlan{sfx}")) == Some(&p)
                {
                    return Some((p, 2));
                }
            }
        }
        p_hit.filter(|_| !p_dup).map(|p| (p, 1))
    }
}
