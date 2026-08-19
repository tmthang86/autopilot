//! Running one role: render its prompt, invoke its tier's harness, and record
//! the role's start and end in the journal. Every role is a fresh invocation
//! with no session resumption — that is what "clean context" means.

use std::path::Path;

use crate::config::Config;
use crate::harness::{Classification, RunParams};
use crate::journal::{CostSource, Event, Journal};

const IMPLEMENT: &str = include_str!("../templates/prompt-implement.tmpl");
const TEST: &str = include_str!("../templates/prompt-test.tmpl");
const REVIEW: &str = include_str!("../templates/prompt-review.tmpl");

pub struct RoleResult {
    pub classify: Classification,
    pub cost_usd: Option<f64>,
    pub cost_source: CostSource,
    pub duration_s: u64,
}

pub fn lens_question(lens: &str) -> String {
    match lens {
        "plan-conformance" => {
            "Does this implement the approved task, and only that? Where has scope widened without being asked for?".into()
        }
        "partial-failure" => {
            "What happens on a crash, a restart, a half-written file, or a command that exits non-zero midway? What is asserted rather than checked?".into()
        }
        "documentation-truth" => {
            "For every row the sync table binds to this change, does the document now tell the truth?".into()
        }
        "adjudicate" => {
            "The tester rejected this work. Decide which party is right, repair the code if the tester is right, then write your verdict.".into()
        }
        other => format!("Answer the question: {other}"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn prompt(
    role: &str,
    cfg: &Config,
    issue: u64,
    title: &str,
    body: &str,
    work_branch: &str,
    intent: &[std::path::PathBuf],
    verdict_path: Option<&Path>,
    lens: Option<&str>,
) -> String {
    let template = match role {
        "implement" => IMPLEMENT,
        "test" => TEST,
        _ => REVIEW,
    };
    let intent_block = intent
        .iter()
        .map(|p| format!("  {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    let verdict = verdict_path
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let question = lens.map(lens_question).unwrap_or_default();

    template
        .replace("{{WORK_BRANCH}}", work_branch)
        .replace("{{PROJECT_NAME}}", &cfg.project.name)
        .replace("{{ISSUE_NUMBER}}", &issue.to_string())
        .replace("{{ISSUE_TITLE}}", title)
        .replace("{{INTENT_FILES}}", &intent_block)
        .replace("{{ISSUE_BODY}}", body)
        .replace("{{VERDICT_PATH}}", &verdict)
        .replace("{{LENS_QUESTION}}", &question)
}

#[allow(clippy::too_many_arguments)]
pub fn run_role(
    cfg: &Config,
    project: &Path,
    role: &str,
    tier_name: &str,
    round: u32,
    lens: Option<&str>,
    prompt: &str,
    journal: &mut Journal,
) -> Result<RoleResult, String> {
    let (_, binding) = crate::tier::resolve(cfg, tier_name, 0).map_err(|e| e.to_string())?;
    let harness = crate::harness::by_name(&binding.harness)
        .ok_or_else(|| format!("unknown harness: {}", binding.harness))?;

    let log_dir = project.join(".autopilot/logs");
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log = log_dir.join(format!("{role}-{round}-{}.jsonl", crate::log::unix_now()));

    journal
        .append(Event::RoleStart {
            role: role.to_string(),
            round,
            tier: tier_name.to_string(),
            harness: binding.harness.clone(),
            model: binding.model.clone(),
            lens: lens.map(str::to_string),
        })
        .ok();

    let params = RunParams {
        model: &binding.model,
        effort: &binding.effort,
        budget_usd: binding.budget_usd,
        timeout_s: cfg.pipeline.turn_timeout_s,
        permission_mode: &cfg.agent.permission_mode,
    };
    let outcome = harness
        .run(prompt, project, &log, &params)
        .map_err(|e| e.to_string())?;
    let classify = harness.classify(&outcome);

    journal
        .append(Event::RoleEnd {
            role: role.to_string(),
            round,
            tier: tier_name.to_string(),
            classify: classify_name(&classify).to_string(),
            cost_usd: outcome.cost_usd,
            cost_source: outcome.cost_source,
            duration_s: outcome.duration_s,
            lens: lens.map(str::to_string),
        })
        .ok();

    Ok(RoleResult {
        classify,
        cost_usd: outcome.cost_usd,
        cost_source: outcome.cost_source,
        duration_s: outcome.duration_s,
    })
}

fn classify_name(c: &Classification) -> &'static str {
    match c {
        Classification::Ok => "ok",
        Classification::TaskFailure => "task_failure",
        Classification::ProviderUnavailable => "provider_unavailable",
    }
}
