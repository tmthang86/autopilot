//! Pure queue helpers kept separate so `queue.rs` stays small enough to audit.

pub fn deps(body: &str) -> Vec<u64> {
    let mut out = Vec::new();
    for line in body.lines() {
        let lower = line.to_lowercase();
        if let Some(idx) = lower.find("depends on #") {
            let rest = &line[idx + "depends on #".len()..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<u64>() {
                out.push(n);
            }
        }
    }
    out
}

pub fn is_closed(repo: &str, issue: u64) -> bool {
    let out = crate::gh::run(
        &[
            "issue",
            "view",
            &issue.to_string(),
            "--repo",
            repo,
            "--json",
            "state",
            "-q",
            ".state",
        ],
        crate::gh::TIMEOUT_S,
    );
    match out {
        Ok(o) if o.ok() => o.stdout.trim() == "CLOSED",
        Ok(_) => {
            crate::log::warn(&format!(
                "unrecognised state for #{issue} in {repo}; treating as still open"
            ));
            false
        }
        Err(_) => false,
    }
}

pub fn edit_claim_label(repo: &str, issue: u64, flag: &str, what: &str) -> Result<(), String> {
    let out = crate::gh::run(
        &[
            "issue",
            "edit",
            &issue.to_string(),
            "--repo",
            repo,
            flag,
            "status:in-progress",
        ],
        crate::gh::TIMEOUT_S,
    )
    .map_err(|e| e.to_string())?;
    if out.ok() {
        Ok(())
    } else {
        crate::log::error(&format!(
            "could not {what} #{issue} in {repo} (status:in-progress unchanged): {}",
            out.stderr
        ));
        Err(out.stderr)
    }
}
