//! Pocket Duty drawn with wgpu: the game of the PSP build in a browser tab
//! over WebGPU, and on the build machine, where frames go to files.
//!
//! The match is `duty_sim` and the flow of `duty_interface`, as the PSP links
//! them. The pack is the PSP's (`profiles/psp30.json`), read over HTTP
//! ([`pack`]); what the GE draws from it with no program is drawn here by
//! WGSL programs ([`render`], [`figures`]). [`app`] is the shell around them.
//! What is not this game's is PocketJS's browser kernel, `pocket_web_wgpu`
//! (`vendor/pocketjs/devices/web/pocket-web-wgpu`).

pub mod app;
pub mod figures;
pub mod pack;
pub mod render;
#[cfg(target_arch = "wasm32")]
mod web;
