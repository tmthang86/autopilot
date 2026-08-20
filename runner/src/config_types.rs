//! Serde DTOs for the two config layers, kept in their own file so the loader
//! stays small enough to audit in one sitting.

use serde::Deserialize;

pub fn default_name() -> String {
    "project".into()
}
pub fn default_tier_prefix() -> String {
    "tier:".into()
}
pub fn default_autonomy() -> String {
    "full".into()
}
pub fn default_max_bytes() -> u64 {
    1 << 20
}
pub fn default_verify_timeout() -> u64 {
    600
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProjectConfig {
    #[serde(default = "default_name")]
    pub name: String,
    pub main_branch: String,
    pub work_branch: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct QueueConfig {
    pub ready_label: String,
    pub exclude_labels: Vec<String>,
    pub intent_marker: String,
    #[serde(default = "default_tier_prefix")]
    pub tier_label_prefix: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RoleTier {
    pub tier_offset: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RolesConfig {
    #[serde(default)]
    pub context_docs: Vec<String>,
    pub implement: RoleTier,
    pub test: RoleTier,
    pub review: RoleTier,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PipelineConfig {
    pub max_rounds: u32,
    pub turn_timeout_s: u64,
    pub wake_timeout_s: u64,
    pub wake_budget_usd: f64,
    #[serde(default)]
    pub review_lenses: Vec<String>,
    #[serde(default = "default_verify_timeout")]
    pub verify_timeout_s: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AgentConfig {
    pub permission_mode: String,
    pub default_tier: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct JournalConfig {
    #[serde(default = "default_max_bytes")]
    pub max_bytes: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VerifyCmd {
    pub name: String,
    pub cmd: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PacingConfig {
    pub daily_task_cap: i64,
    #[serde(default)]
    pub quiet_hours: Vec<String>,
    pub max_attempts_per_issue: u32,
    pub circuit_breaker_failures: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AutonomyConfig {
    #[serde(default = "default_autonomy")]
    #[allow(dead_code)] // One autonomy class today; see open-items.
    pub default: String,
    pub prepare_only_label: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TierBinding {
    pub harness: String,
    pub model: String,
    pub effort: String,
    #[serde(default)]
    pub budget_usd: f64,
}
