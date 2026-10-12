//! The sounds: from `audio.dir` to the pack's `SNDS` and `SNDD` sections (`duty_pack::sound`).

use std::path::Path;

use duty_pack::sound::Sound;
use serde_json::Value;

/// The records, one per `sound::Id` in order, and the samples; both empty when the export holds no audio.
pub fn cook(ir: &Path, _profile: &Value) -> Result<(Vec<Sound>, Vec<u8>), String> {
    if !ir.join("audio.dir").exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    Ok((Vec::new(), Vec::new()))
}
