//! Tokenizer — whitespace is exactly {space, tab}; `\r` never reaches
//! this point (stripped at line split). Byte offsets are recorded per
//! token so the error caret lands on Token[tok_index].start.

use super::types::Token;

pub fn tokenize(line: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' || bytes[i] == b'\t' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' {
            i += 1;
        }
        out.push(Token {
            text: line[start..i].to_string(),
            start,
        });
    }
    out
}

/// Remainder of the line starting at `toks[i]` — joins the typed tokens
/// with single spaces (LINE capture preserves case, not spacing).
pub fn rest_of_line(toks: &[Token], i: usize) -> String {
    toks[i..]
        .iter()
        .map(|t| t.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}
