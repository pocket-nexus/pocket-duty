//! The sounds: from `audio.dir` to the pack's `SNDS` and `SNDD` sections (`duty_pack::sound`).
//!
//! The export holds every sound as floats at the rate the reference rendered it, at the level the
//! reference's own mix gave it. Each one is brought to the device's rate through a windowed-sinc
//! low-pass, cut to where it is heard, set to the loudness of its group ([`LEVELS`]) and stored as
//! signed 16-bit samples.
//!
//! `DUTY_SOUNDS=1` prints a line per sound: seconds, rate, bytes, peak, loudness, gain.

use std::path::Path;

use duty_pack::sound::{Id, Sound};
use serde_json::Value;

use crate::ir::Dir;

/// How loud every sound of a group is stored, by the start of its name: dB under the player's rifle
/// ([`loudness`]), and the highest peak it may have, in dB under full scale.
///
/// The rifle is the loudest thing: its three takes are brought to one loudness with their peaks at
/// the ceiling or under it, and the other two weapons stand under it as in the reference's mix. A
/// sound with a place in the world (a soldier's rifle, an impact, a voice, a burst) is stored as
/// loud as it is close by, and the game turns it down with distance. What is heard all the time
/// (steps, cloth, the bed of wind) lies lowest. The heartbeat is all under 80 Hz, where the measure
/// hears little: its level is its peak.
///
/// The first group is the one the others are measured against, and a name takes the first row it
/// starts with.
const LEVELS: &[(&str, f32, f32)] = &[
    ("RifleShot", 0.0, HEADROOM),
    ("SmgShot", 2.5, HEADROOM),
    ("PistolShot", 3.5, HEADROOM),
    ("EnemyShotNear", 1.0, HEADROOM),
    ("EnemyShotFar", 4.0, HEADROOM),
    ("DryFire", 18.0, HEADROOM),
    ("Reload", 13.0, HEADROOM),
    ("WeaponDraw", 17.0, HEADROOM),
    // Earth and a body answer a round more softly than stone, steel and wood.
    ("ImpactDirt", 12.0, HEADROOM),
    ("ImpactFlesh", 11.0, HEADROOM),
    ("Impact", 8.0, HEADROOM),
    ("Whizz", 5.0, HEADROOM),
    ("Step", 20.0, HEADROOM),
    ("Land", 13.0, HEADROOM),
    ("Hitmarker", 9.0, HEADROOM),
    ("Headshot", 8.0, HEADROOM),
    ("Kill", 6.0, HEADROOM),
    ("Hurt", 6.0, HEADROOM),
    ("Heartbeat", 0.0, 9.0),
    ("Bark", 7.0, HEADROOM),
    ("BodyFall", 15.0, HEADROOM),
    ("Explosion", 0.0, HEADROOM),
    ("GrenadeBounce", 14.0, HEADROOM),
    ("Ambience", 18.0, HEADROOM),
    ("DistantVolley", 12.0, HEADROOM),
    ("DistantBoom", 9.0, HEADROOM),
    ("UiMove", 20.0, HEADROOM),
    ("UiSelect", 16.0, HEADROOM),
];

/// dB between the loudest sample of any sound and full scale.
const HEADROOM: f32 = 3.0;
/// What counts as silence at a sound's ends: 60 dB under its peak.
const SILENCE: f32 = 0.001;
/// Samples over which a sound's cut end goes to zero.
const TAIL: usize = 64;

fn db(level: f32) -> f32 {
    20.0 * level.max(1e-9).log10()
}

/// A sound at the device's rate, before its level is set.
struct Cut {
    pcm: Vec<f32>,
    rate: u32,
    looped: bool,
    peak: f32,
    loudness: f32,
}

/// The records, one per `sound::Id` in order, and the samples; both empty when the export holds no audio.
pub fn cook(ir: &Path, profile: &Value) -> Result<(Vec<Sound>, Vec<u8>), String> {
    let path = ir.join("audio.dir");
    if !path.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let dir = Dir::open(&path)?;
    let listed = dir.meta["sounds"].as_array().ok_or("audio.dir lists no sounds")?;
    if listed.len() != Id::NAMES.len() {
        return Err(format!("audio.dir holds {} sounds, the pack's list has {} (web/src/pocket/audio.js follows crates/duty-pack/src/sound.rs)", listed.len(), Id::NAMES.len()));
    }
    let rate = profile["audio"]["rate"].as_u64().unwrap_or(22_050) as u32;
    let bed_rate = profile["audio"]["ambienceRate"].as_u64().unwrap_or(11_025) as u32;

    let mut cuts = Vec::with_capacity(listed.len());
    for (i, (name, entry)) in Id::NAMES.iter().zip(listed).enumerate() {
        let have = entry["name"].as_str().unwrap_or("");
        if have != *name {
            return Err(format!("audio.dir: sound {i} is {have}, the pack's list has {name} there (web/src/pocket/audio.js follows crates/duty-pack/src/sound.rs)"));
        }
        let from = entry["rate"].as_f64().filter(|r| *r > 0.0).ok_or_else(|| format!("audio.dir: {name} has no rate"))?;
        let looped = entry["looped"].as_bool().unwrap_or(false);
        let to = if *name == "Ambience" { bed_rate } else { rate };
        let mut pcm = resample(&dir.f32s(name), from, to as f64, looped);
        if !looped {
            trim(&mut pcm);
        }
        let peak = pcm.iter().fold(0f32, |m, v| m.max(v.abs()));
        if pcm.is_empty() || !(peak > 0.0) {
            return Err(format!("audio.dir: {name} is silent"));
        }
        let loudness = loudness(&pcm, to as f32);
        cuts.push(Cut { pcm, rate: to, looped, peak, loudness });
    }

    let level_of = |name: &str| LEVELS.iter().find(|(start, ..)| name.starts_with(start)).ok_or_else(|| format!("audio: no level is chosen for {name} (LEVELS in crates/duty-cook/src/audio.rs)"));
    // The loudness everything is set against: what the quietest take of the first group has with its
    // peak at that group's ceiling, so every take of it can reach it.
    let (first, _, ceiling) = LEVELS[0];
    let reference = Id::NAMES.iter().zip(&cuts).filter(|(name, _)| name.starts_with(first)).map(|(_, c)| db(c.loudness) - ceiling - db(c.peak)).fold(f32::INFINITY, f32::min);
    if !reference.is_finite() {
        return Err(format!("audio: no sound is named {first}…, and the levels are set against it"));
    }

    let table = std::env::var_os("DUTY_SOUNDS").is_some();
    let mut records = Vec::with_capacity(cuts.len());
    let mut bytes: Vec<u8> = Vec::new();
    for (name, cut) in Id::NAMES.iter().zip(&cuts) {
        let (_, under, ceiling) = *level_of(name)?;
        let gain_db = (reference - under - db(cut.loudness)).min(-ceiling - db(cut.peak));
        let gain = 10f32.powf(gain_db / 20.0);
        // Each sound's samples start on a 4-byte boundary.
        bytes.resize(bytes.len().next_multiple_of(4), 0);
        records.push(Sound { offset: bytes.len() as u32, frames: cut.pcm.len() as u32, rate: cut.rate, looped: cut.looped as u32 });
        bytes.extend(cut.pcm.iter().flat_map(|v| ((v * gain * 32767.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes()));
        if table {
            println!(
                "  {name:<16} {:5.2} s  {:5} Hz  {:6} bytes  peak {:5.1} dB  {:5.1} dB under the rifle  (gain {:+5.1} dB)",
                cut.pcm.len() as f32 / cut.rate as f32,
                cut.rate,
                cut.pcm.len() * 2,
                db(cut.peak) + gain_db,
                reference - db(cut.loudness) - gain_db,
                gain_db
            );
        }
    }
    if table {
        println!("  {} sounds, {} bytes", records.len(), bytes.len());
    }
    Ok((records, bytes))
}

/// How loud a sound is to an ear at a small speaker: the level of its loudest eighth of a second
/// (a sound-level meter's "fast" time) after the low side of the A curve, four one-pole high-passes
/// (20.6 Hz twice, 107.7 Hz, 737.9 Hz) that leave 1 kHz as it is. A sound shorter than the eighth
/// counts with its energy spread over it, as the ear takes a click.
fn loudness(pcm: &[f32], rate: f32) -> f32 {
    let mut weighted = pcm.to_vec();
    for corner in [20.6f32, 20.6, 107.7, 737.9] {
        let rc = 1.0 / (core::f32::consts::TAU * corner);
        let a = rc / (rc + 1.0 / rate);
        let (mut out, mut last) = (0f32, 0f32);
        for v in weighted.iter_mut() {
            out = a * (out + *v - last);
            last = *v;
            *v = out;
        }
    }
    let window = (rate * 0.125) as usize;
    let squares: Vec<f64> = weighted.iter().map(|v| (*v as f64) * (*v as f64)).collect();
    let (mut sum, mut most) = (0f64, 0f64);
    for i in 0..squares.len() {
        sum += squares[i];
        if i >= window {
            sum -= squares[i - window];
        }
        most = most.max(sum);
    }
    // 1.2506 brings the four filters' level at 1 kHz back to 1.
    1.2506 * (most / window as f64).sqrt() as f32
}

/// Drops what lies under [`SILENCE`] before the sound and after it, and brings the cut end to zero.
fn trim(pcm: &mut Vec<f32>) {
    let floor = pcm.iter().fold(0f32, |m, v| m.max(v.abs())) * SILENCE;
    let Some(last) = pcm.iter().rposition(|v| v.abs() >= floor) else { return };
    pcm.truncate(last + 1);
    let first = pcm.iter().position(|v| v.abs() >= floor).unwrap_or(0);
    pcm.drain(..first);
    let n = TAIL.min(pcm.len() / 4);
    let len = pcm.len();
    for (i, v) in pcm[len - n..].iter_mut().enumerate() {
        *v *= (n - i) as f32 / (n + 1) as f32;
    }
}

/// Zero-order modified Bessel function of the first kind, for the Kaiser window.
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > sum * 1e-12 {
        term *= (x / (2.0 * k)) * (x / (2.0 * k));
        sum += term;
        k += 1.0;
    }
    sum
}

/// The low-pass a change of rate needs, as a table: a sinc under a Kaiser window, sampled
/// [`Kernel::STEPS`] times per input sample from its centre to where it ends.
struct Kernel {
    /// Input samples from the centre to the window's edge.
    reach: f64,
    table: Vec<f32>,
}

impl Kernel {
    const STEPS: usize = 512;
    /// 80 dB under the pass band beyond the stop band's edge.
    const BETA: f64 = 7.857;
    /// The pass band ends at this part of the lower rate's half, and the stop band starts at the half.
    const PASS: f64 = 0.84;

    /// `ratio` is input samples per output sample.
    fn new(ratio: f64) -> Kernel {
        // In cycles per input sample: the half of the lower of the two rates, the band the filter
        // falls over, and the point halfway down it.
        let nyquist = 0.5 / ratio.max(1.0);
        let fall = nyquist * (1.0 - Self::PASS);
        let cutoff = nyquist - fall / 2.0;
        // The length a Kaiser window needs for that fall at that depth (Kaiser's estimate).
        let reach = ((80.0 - 7.95) / (14.36 * fall) / 2.0).ceil();
        let size = (reach * Self::STEPS as f64) as usize;
        let i0 = bessel_i0(Self::BETA);
        let table = (0..=size + 1)
            .map(|i| {
                let t = i as f64 / Self::STEPS as f64;
                if t >= reach {
                    return 0.0;
                }
                let x = core::f64::consts::PI * 2.0 * cutoff * t;
                let sinc = if x == 0.0 { 1.0 } else { x.sin() / x };
                let window = bessel_i0(Self::BETA * (1.0 - (t / reach) * (t / reach)).sqrt()) / i0;
                (2.0 * cutoff * sinc * window) as f32
            })
            .collect();
        Kernel { reach, table }
    }

    /// The filter `distance` input samples from its centre.
    #[inline]
    fn at(&self, distance: f64) -> f32 {
        let t = distance.abs() * Self::STEPS as f64;
        let i = t as usize;
        if i + 1 >= self.table.len() {
            return 0.0;
        }
        let f = (t - i as f64) as f32;
        self.table[i] + (self.table[i + 1] - self.table[i]) * f
    }
}

/// `input` at `from` samples a second as samples at `to` a second. A sound that plays end to start
/// (`wrap`) is filtered around its end, and comes out as a whole number of frames of the same length
/// in time.
fn resample(input: &[f32], from: f64, to: f64, wrap: bool) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    let n = input.len();
    let frames = ((n as f64 * to / from).round() as usize).max(1);
    // Input samples per output sample. A loop's step makes its last frame's successor its first.
    let ratio = if wrap { n as f64 / frames as f64 } else { from / to };
    let kernel = Kernel::new(ratio);
    (0..frames)
        .map(|frame| {
            let centre = frame as f64 * ratio;
            let (first, last) = ((centre - kernel.reach).ceil() as i64, (centre + kernel.reach).floor() as i64);
            let (mut sum, mut weight) = (0f64, 0f64);
            for k in first..=last {
                let w = kernel.at(k as f64 - centre) as f64;
                weight += w;
                if wrap {
                    sum += w * input[k.rem_euclid(n as i64) as usize] as f64;
                } else if k >= 0 && (k as usize) < n {
                    sum += w * input[k as usize] as f64;
                }
            }
            // The weights of one output sample sum to 1 but for the window's ripple.
            (sum / weight) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, rate: f64, frames: usize) -> Vec<f32> {
        (0..frames).map(|i| (core::f64::consts::TAU * hz * i as f64 / rate).sin() as f32).collect()
    }

    /// The largest sample away from the ends, where the filter has all its taps.
    fn level(pcm: &[f32]) -> f32 {
        pcm[pcm.len() / 4..pcm.len() * 3 / 4].iter().fold(0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn the_pass_band_keeps_its_level() {
        for hz in [50.0, 1000.0, 5000.0, 9000.0] {
            let out = resample(&sine(hz, 48_000.0, 48_000), 48_000.0, 22_050.0, false);
            assert_eq!(out.len(), 22_050);
            assert!((level(&out) - 1.0).abs() < 0.01, "{hz} Hz came out at {}", level(&out));
        }
    }

    #[test]
    fn what_the_lower_rate_cannot_hold_is_removed() {
        for hz in [11_100.0, 13_000.0, 18_000.0, 23_000.0] {
            let out = resample(&sine(hz, 48_000.0, 48_000), 48_000.0, 22_050.0, false);
            assert!(level(&out) < 0.0002, "{hz} Hz came out at {}", level(&out));
        }
        let out = resample(&sine(6000.0, 48_000.0, 48_000), 48_000.0, 11_025.0, false);
        assert!(level(&out) < 0.0002);
    }

    #[test]
    fn a_sample_keeps_its_place_in_time() {
        let mut input = vec![0f32; 4800];
        input[2400] = 1.0;
        let out = resample(&input, 48_000.0, 22_050.0, false);
        let at = out.iter().enumerate().max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())).unwrap().0;
        assert!((at as f64 - 2400.0 * 22_050.0 / 48_000.0).abs() <= 1.0);
    }

    #[test]
    fn a_loop_stays_a_loop() {
        // Eight seconds at 48 kHz are a whole number of frames at 11 025 Hz, and 100 Hz fits both.
        let out = resample(&sine(100.0, 48_000.0, 384_000), 48_000.0, 11_025.0, true);
        assert_eq!(out.len(), 88_200);
        let want = sine(100.0, 11_025.0, 88_200);
        let worst = out.iter().zip(&want).fold(0f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(worst < 0.001, "off by {worst}");
    }

    #[test]
    fn silence_is_cut_at_both_ends() {
        let mut pcm = vec![0f32; 100];
        pcm.extend(sine(1000.0, 22_050.0, 2000));
        pcm.extend(vec![0.0002f32; 300]);
        trim(&mut pcm);
        assert!(pcm.len() <= 2000 && pcm.len() > 1990);
        assert!(pcm[0].abs() >= 0.001);
        assert!(pcm[pcm.len() - 1].abs() < 0.05);
    }
}
