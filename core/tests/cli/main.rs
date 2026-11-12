//! The corpus driver — the verification surface for the IOS CLI engine.
//! `cargo test` asserts goldens; `NETLAB_GOLDEN=write cargo test` is the
//! ONLY writer of `.golden` files and `web/_test/corpus-steps.json`.

mod fixtures;
mod properties;
mod runner;

use std::env;
use std::fs;
use std::path::Path;

#[test]
fn corpus_transcripts() {
    let write = env::var("NETLAB_GOLDEN").as_deref() == Ok("write");
    let raws = runner::corpus_files();
    assert!(
        !raws.is_empty(),
        "0 cases found in {}",
        runner::CORPUS_DIR
    );
    let orphans = runner::orphan_goldens();
    assert!(
        orphans.is_empty(),
        "orphan golden — delete or author the .raw: {}",
        orphans
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut fails: Vec<String> = Vec::new();
    let mut emitted: Vec<(runner::Case, u64, String)> = Vec::new();
    for raw in &raws {
        match drive(raw, write) {
            Ok(Some(e)) => emitted.push(e),
            Ok(None) => {}
            Err(m) => fails.push(m),
        }
    }
    if write && fails.is_empty() {
        let refs: Vec<(&runner::Case, u64, &str)> = emitted
            .iter()
            .map(|(c, s, b)| (c, *s, b.as_str()))
            .collect();
        match runner::emit_steps(&refs) {
            Ok(_) => println!("EMITTED {}", runner::STEPS_JSON),
            Err(e) => fails.push(e),
        }
    }
    assert!(fails.is_empty(), "\n{}", fails.join("\n\n"));
}

fn drive(
    path: &Path,
    write: bool,
) -> Result<Option<(runner::Case, u64, String)>, String> {
    let case = runner::parse_case(path).map_err(|e| e.to_string())?;
    let frames = runner::run_case(&case).map_err(|m| format!("{}: {m}", case.id))?;
    let mut masks = runner::MaskSet::new(&case.masks);
    let body = runner::normalize(&frames, &mut masks);
    masks.check_used().map_err(|m| format!("{}: {m}", case.id))?;
    let raw_src = fs::read_to_string(path)
        .map_err(|e| format!("{}: read .raw: {e}", case.id))?;
    let sha = runner::src_sha(&runner::normalize_raw(&raw_src));
    let gpath = path.with_extension("golden");
    if write {
        let golden = runner::render_golden(&case, sha, &body);
        fs::write(&gpath, &golden)
            .map_err(|e| format!("{}: write golden: {e}", case.id))?;
        println!("WRITTEN {}", case.id);
        return Ok(Some((case, sha, body)));
    }
    let text = match fs::read_to_string(&gpath) {
        Ok(t) => t,
        Err(_) => {
            return Err(format!(
                "{}: missing golden {} — run `NETLAB_GOLDEN=write cargo test -p netlab-core`",
                case.id,
                gpath.display()
            ))
        }
    };
    let g = runner::parse_golden(&text)
        .map_err(|m| format!("{}: bad golden: {m}", case.id))?;
    if g.id != case.id {
        return Err(format!(
            "{}: golden id mismatch `{}`",
            case.id, g.id
        ));
    }
    if g.provenance != case.provenance {
        return Err(format!("{}: golden provenance mismatch", case.id));
    }
    if g.sha != sha {
        return Err(format!(
            "{}: stale golden — regenerate via `NETLAB_GOLDEN=write cargo test -p netlab-core`",
            case.id
        ));
    }
    runner::compare(&g.body, &body)
        .map_err(|d| format!("{}: golden diff\n{}", case.id, d.report))?;
    Ok(None)
}
