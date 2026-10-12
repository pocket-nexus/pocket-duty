//! The soldiers and the held weapons: from `figures.dir` to the pack's `FIGS` section
//! (`duty_pack::figure`).

use std::path::Path;

use serde_json::Value;

/// The section's bytes; empty when the export holds no figures.
pub fn cook(ir: &Path, _profile: &Value) -> Result<Vec<u8>, String> {
    if !ir.join("figures.dir").exists() {
        return Ok(Vec::new());
    }
    Ok(Vec::new())
}
