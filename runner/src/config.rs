//! Two-layer config: committed `config.json` (tier names, roles, pipeline) plus
//! gitignored `.autopilot/tiers.local.json` (machine bindings). Every declared
//! tier must have a binding, or the load fails by name.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub use crate::config_types::{
    AgentConfig, AutonomyConfig, JournalConfig, PacingConfig, PipelineConfig, ProjectConfig,
    QueueConfig, RoleTier, RolesConfig, TierBinding, VerifyCmd,
};

#[derive(Debug)]
pub enum ConfigError {
    NotFound(PathBuf),
    Invalid { path: PathBuf, msg: String },
    UnboundTier(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::NotFound(p) => write!(f, "config not found: {}", p.display()),
            ConfigError::Invalid { path, msg } => {
                write!(f, "config is not valid: {} ({msg})", path.display())
            }
            ConfigError::UnboundTier(t) => {
                write!(f, "tier \"{t}\" has no binding in .autopilot/tiers.local.json")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

fn default_journal() -> JournalConfig {
    JournalConfig {
        max_bytes: crate::config_types::default_max_bytes(),
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub project: ProjectConfig,
    pub queue: QueueConfig,
    pub tiers: Vec<String>,
    pub roles: RolesConfig,
    pub pipeline: PipelineConfig,
    pub agent: AgentConfig,
    pub journal: JournalConfig,
    pub verify: Vec<VerifyCmd>,
    pub pacing: PacingConfig,
    pub autonomy: AutonomyConfig,
    pub bindings: BTreeMap<String, TierBinding>,
}

#[derive(Deserialize)]
struct RawConfig {
    project: ProjectConfig,
    queue: QueueConfig,
    tiers: Vec<String>,
    roles: RolesConfig,
    pipeline: PipelineConfig,
    agent: AgentConfig,
    #[serde(default = "default_journal")]
    journal: JournalConfig,
    verify: Vec<VerifyCmd>,
    pacing: PacingConfig,
    autonomy: AutonomyConfig,
}

impl Config {
    pub fn load(project: &Path) -> Result<Config, ConfigError> {
        let cfg_path = project.join(".autopilot/config.json");
        let raw = std::fs::read_to_string(&cfg_path)
            .map_err(|_| ConfigError::NotFound(cfg_path.clone()))?;
        let val: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
            ConfigError::Invalid {
                path: cfg_path.clone(),
                msg: e.to_string(),
            }
        })?;

        // The model/effort/budget keys are gone; a config that still carries
        // them must fail loudly, naming what is missing, rather than silently
        // running every task on the wrong model.
        for key in ["default_model", "default_effort", "max_budget_usd"] {
            if val.pointer(&format!("/agent/{key}")).is_some() {
                return Err(ConfigError::Invalid {
                    path: cfg_path.clone(),
                    msg: format!(
                        "agent.{key} has been removed — declare tiers and bind them in .autopilot/tiers.local.json"
                    ),
                });
            }
        }

        let raw: RawConfig = serde_json::from_value(val).map_err(|e| ConfigError::Invalid {
            path: cfg_path.clone(),
            msg: e.to_string(),
        })?;

        let bind_path = project.join(".autopilot/tiers.local.json");
        let bindings: BTreeMap<String, TierBinding> = if bind_path.exists() {
            let b = std::fs::read_to_string(&bind_path).map_err(|e| ConfigError::Invalid {
                path: bind_path.clone(),
                msg: e.to_string(),
            })?;
            serde_json::from_str(&b).map_err(|e| ConfigError::Invalid {
                path: bind_path.clone(),
                msg: e.to_string(),
            })?
        } else {
            BTreeMap::new()
        };

        for tier in &raw.tiers {
            if !bindings.contains_key(tier) {
                return Err(ConfigError::UnboundTier(tier.clone()));
            }
        }

        Ok(Config {
            project: raw.project,
            queue: raw.queue,
            tiers: raw.tiers,
            roles: raw.roles,
            pipeline: raw.pipeline,
            agent: raw.agent,
            journal: raw.journal,
            verify: raw.verify,
            pacing: raw.pacing,
            autonomy: raw.autonomy,
            bindings,
        })
    }
}
