//! The tier ladder: an ordered array the project names itself. `tier_offset`
//! resolves "one step up"; the top tier offsets to itself rather than
//! overflowing, and the bottom tier offsets downward to itself.

use crate::config::{Config, ConfigError, TierBinding};

pub fn resolve<'a>(
    cfg: &'a Config,
    name: &str,
    offset: i32,
) -> Result<(&'a str, &'a TierBinding), ConfigError> {
    let idx = cfg
        .tiers
        .iter()
        .position(|t| t == name)
        .ok_or_else(|| ConfigError::UnboundTier(name.to_string()))?;
    let target = (idx as i32 + offset).clamp(0, cfg.tiers.len() as i32 - 1) as usize;
    let tname = &cfg.tiers[target];
    let binding = cfg
        .bindings
        .get(tname)
        .ok_or_else(|| ConfigError::UnboundTier(tname.clone()))?;
    Ok((tname.as_str(), binding))
}
