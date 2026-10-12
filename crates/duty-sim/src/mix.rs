//! The mixer: playing sounds to a block of stereo samples. A device's output thread runs it once
//! per block, so it is integer arithmetic and holds everything it needs from the start.
//!
//! A sound is signed 16-bit mono samples in the pack's sample section (`duty_pack::sound`). A
//! [`Voice`] plays one sound at a pitch, with a gain for each side; [`Mixer::render`] reads every
//! voice with linear interpolation between samples and adds them up.

use duty_pack::sound::Sound;

/// Frames a second the mixer renders.
pub const OUTPUT_RATE: u32 = 22_050;
/// The gain that leaves a sample as it is, and the pitch that plays a sound as recorded.
pub const UNITY: u16 = 256;
/// Frames summed in one pass over the voices.
pub const BLOCK: usize = 512;

/// One playing sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Voice {
    /// False for a voice that plays nothing.
    pub active: bool,
    /// The sound plays end to start without a gap.
    pub looped: bool,
    /// When every voice is busy, a voice gives way to a new one of its priority or a higher one.
    pub priority: u8,
    /// The sound's place in `SNDS`.
    pub sound: u16,
    /// The gains of the two sides, 0…256.
    pub left: u16,
    pub right: u16,
    /// Frames of the sound per output frame, with 16 bits of fraction.
    pub step: u32,
    /// The place in the sound the next output frame reads, in frames with 16 bits of fraction.
    /// The whole part is wider than 16 bits: those would end at 65 536 frames, under 3 s at the
    /// output rate, and the bed of ambience is longer.
    pub position: u64,
}

impl Voice {
    pub const IDLE: Voice = Voice {
        active: false,
        looped: false,
        priority: 0,
        sound: 0,
        left: 0,
        right: 0,
        step: 0,
        position: 0,
    };
}

pub struct Mixer<const VOICES: usize> {
    voices: [Voice; VOICES],
    /// A block's sum before it is brought to 16 bits: left, right, left…
    sum: [i32; BLOCK * 2],
}

impl<const VOICES: usize> Default for Mixer<VOICES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const VOICES: usize> Mixer<VOICES> {
    pub const fn new() -> Self {
        Mixer {
            voices: [Voice::IDLE; VOICES],
            sum: [0; BLOCK * 2],
        }
    }

    /// Starts sound `id` at gains `left` and `right` (0…256) and `pitch` (256 plays it as
    /// recorded, 512 an octave up), and answers the voice it plays on. When every voice is busy
    /// it takes the place of the quietest voice of `priority` or lower; when there is none, and
    /// when the list has no such sound, it answers `None`.
    pub fn play(
        &mut self,
        sounds: &[Sound],
        id: u16,
        left: u16,
        right: u16,
        pitch: u16,
        priority: u8,
    ) -> Option<usize> {
        let sound = sounds.get(id as usize)?;
        if sound.frames == 0 {
            return None;
        }
        // A step of zero would never reach the sound's end.
        let step = (((sound.rate as u64) << 16) * pitch as u64
            / (OUTPUT_RATE as u64 * UNITY as u64))
            .clamp(1, u32::MAX as u64) as u32;
        let slot = match self.voices.iter().position(|v| !v.active) {
            Some(free) => free,
            None => {
                // The quietest; of two as quiet, the one of lower priority.
                let mut pick: Option<(usize, (u16, u8))> = None;
                for (i, v) in self.voices.iter().enumerate() {
                    let key = (v.left + v.right, v.priority);
                    if v.priority <= priority && pick.map_or(true, |(_, least)| key < least) {
                        pick = Some((i, key));
                    }
                }
                pick?.0
            }
        };
        self.voices[slot] = Voice {
            active: true,
            looped: sound.looped != 0,
            priority,
            sound: id,
            left: left.min(UNITY),
            right: right.min(UNITY),
            step,
            position: 0,
        };
        Some(slot)
    }

    pub fn stop(&mut self, slot: usize) {
        if let Some(v) = self.voices.get_mut(slot) {
            v.active = false;
        }
    }

    pub fn stop_all(&mut self) {
        for v in self.voices.iter_mut() {
            v.active = false;
        }
    }

    /// Sets the gains (0…256) of the voice on `slot`, if one plays there.
    pub fn set_gain(&mut self, slot: usize, left: u16, right: u16) {
        if let Some(v) = self.voices.get_mut(slot).filter(|v| v.active) {
            v.left = left.min(UNITY);
            v.right = right.min(UNITY);
        }
    }

    /// The voice playing on `slot`. A slot is given to another sound once its voice ended: the
    /// voice's `sound` says whether it is still the one that was started.
    pub fn voice(&self, slot: usize) -> Option<&Voice> {
        self.voices.get(slot).filter(|v| v.active)
    }

    /// Voices playing now.
    pub fn playing(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Adds every voice to `out` (left, right, left…), holding each sum inside 16 bits, and moves
    /// the voices on. A voice ends at its sound's last sample unless the sound loops, and at once
    /// when its sound does not lie in `samples`.
    ///
    /// `samples` is the pack's sample section and must start on an even address (a pack's
    /// sections start on 64-byte boundaries); on an odd one nothing is rendered.
    pub fn render(&mut self, sounds: &[Sound], samples: &[u8], out: &mut [i16]) {
        let Some(pcm) = frames_of(samples) else {
            debug_assert!(false, "the sample section starts on an odd address");
            return;
        };
        for block in out.chunks_mut(BLOCK * 2) {
            let sum = &mut self.sum[..block.len() & !1];
            let mut any = false;
            for voice in self.voices.iter_mut().filter(|v| v.active) {
                let Some(data) = sound_of(sounds, pcm, voice.sound) else {
                    voice.active = false;
                    continue;
                };
                if !any {
                    sum.fill(0);
                    any = true;
                }
                mix(voice, data, sum);
            }
            if any {
                for (o, s) in block.iter_mut().zip(sum.iter()) {
                    *o = (*o as i32 + *s).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                }
            }
        }
    }
}

/// The sample section as 16-bit samples, when it starts on an even address.
fn frames_of(samples: &[u8]) -> Option<&[i16]> {
    if samples.as_ptr() as usize & 1 != 0 || cfg!(target_endian = "big") {
        return None;
    }
    // SAFETY: the address is even, `len / 2` 16-bit samples lie inside the slice, the samples are
    // stored low byte first as this machine reads them, and every pair of bytes is an `i16`.
    Some(unsafe { core::slice::from_raw_parts(samples.as_ptr() as *const i16, samples.len() / 2) })
}

/// The samples of sound `id`, when the list has it and they lie inside the section.
fn sound_of<'a>(sounds: &[Sound], pcm: &'a [i16], id: u16) -> Option<&'a [i16]> {
    let sound = sounds.get(id as usize)?;
    if sound.offset & 1 != 0 || sound.frames == 0 {
        return None;
    }
    let first = (sound.offset / 2) as usize;
    pcm.get(first..first.checked_add(sound.frames as usize)?)
}

/// One output sample between `a` and its successor `b`: `fraction` is 16 bits, of which 15 are
/// used so the product stays inside 32 bits.
#[inline(always)]
fn between(a: i32, b: i32, fraction: u32) -> i32 {
    a + (((b - a) * (fraction >> 1) as i32) >> 15)
}

/// Adds `voice` reading `data` to `sum` (left, right, left…) and moves it on by the frames of `sum`.
fn mix(voice: &mut Voice, data: &[i16], sum: &mut [i32]) {
    let end = (data.len() as u64) << 16;
    let (left, right, step) = (voice.left as i32, voice.right as i32, voice.step);
    let total = sum.len() / 2;
    let mut position = voice.position;
    let mut at = 0;
    while at < total {
        if position >= end {
            if !voice.looped {
                break;
            }
            position %= end;
        }
        let frame = (position >> 16) as usize;
        let fraction = (position & 0xffff) as u32;
        // Frames of the sound after this one.
        let room = data.len() - 1 - frame;
        if room == 0 {
            // The last frame: what follows it is the first frame of a loop, and silence otherwise.
            let next = if voice.looped { data[0] as i32 } else { 0 };
            let s = between(data[frame] as i32, next, fraction);
            sum[at * 2] += (s * left) >> 8;
            sum[at * 2 + 1] += (s * right) >> 8;
            position += step as u64;
            at += 1;
            continue;
        }
        // A run of output frames that each read a frame and its successor inside the sound. The
        // place of each, counted from `frame`, stays under `limit`: short of the last frame, and
        // inside 32 bits so the loop below counts in one register.
        let limit = ((room as u64) << 16).min(1 << 32);
        let wanted = total - at;
        let last = fraction as u64 + (wanted as u64 - 1) * step as u64;
        let n = if last < limit {
            wanted
        } else {
            (((limit - 1) as u32 - fraction) / step) as usize + 1
        };
        let run = &mut sum[at * 2..(at + n) * 2];
        if step == 1 << 16 && fraction == 0 {
            // As recorded at the output rate: every output frame is a sample.
            for (pair, &s) in run.chunks_exact_mut(2).zip(&data[frame..frame + n]) {
                pair[0] += (s as i32 * left) >> 8;
                pair[1] += (s as i32 * right) >> 8;
            }
        } else {
            let source = data[frame..].as_ptr();
            let mut place = fraction;
            for pair in run.chunks_exact_mut(2) {
                let i = (place >> 16) as usize;
                // SAFETY: `place` is under `limit` for each of the run's `n` frames, so `i` is at
                // most `room - 1` and `frame + i + 1` is at most the sound's last frame.
                let (a, b) = unsafe { (*source.add(i) as i32, *source.add(i + 1) as i32) };
                let s = between(a, b, place & 0xffff);
                pair[0] += (s * left) >> 8;
                pair[1] += (s * right) >> 8;
                place = place.wrapping_add(step);
            }
        }
        position += n as u64 * step as u64;
        at += n;
    }
    if position >= end {
        if voice.looped {
            position %= end;
        } else {
            voice.active = false;
        }
    }
    voice.position = position;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// A sample section and its records, as the compiler lays them out: each sound on a 4-byte boundary.
    struct Bank {
        pcm: Vec<i16>,
        sounds: Vec<Sound>,
    }

    impl Bank {
        fn new(list: &[(&[i16], u32, bool)]) -> Bank {
            let mut bank = Bank {
                pcm: Vec::new(),
                sounds: Vec::new(),
            };
            for (data, rate, looped) in list {
                if bank.pcm.len() % 2 == 1 {
                    bank.pcm.push(0);
                }
                bank.sounds.push(Sound {
                    offset: bank.pcm.len() as u32 * 2,
                    frames: data.len() as u32,
                    rate: *rate,
                    looped: *looped as u32,
                });
                bank.pcm.extend_from_slice(data);
            }
            bank
        }

        fn samples(&self) -> &[u8] {
            // SAFETY: the bytes of the samples, which any `i16` has.
            unsafe {
                core::slice::from_raw_parts(self.pcm.as_ptr() as *const u8, self.pcm.len() * 2)
            }
        }

        /// `frames` of the mixer's output from silence.
        fn render<const N: usize>(&self, mixer: &mut Mixer<N>, frames: usize) -> Vec<i16> {
            let mut out = vec![0i16; frames * 2];
            mixer.render(&self.sounds, self.samples(), &mut out);
            out
        }
    }

    fn side(out: &[i16], which: usize) -> Vec<i16> {
        out.chunks_exact(2).map(|pair| pair[which]).collect()
    }

    /// A sound that never repeats and reaches both ends of the range.
    fn noise(frames: usize) -> Vec<i16> {
        let mut x = 0x2545_f491u32;
        (0..frames)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x >> 16) as i16
            })
            .collect()
    }

    #[test]
    fn as_recorded_it_is_its_samples() {
        let data = noise(1500);
        let bank = Bank::new(&[(&data, OUTPUT_RATE, false)]);
        let mut mixer = Mixer::<4>::new();
        assert_eq!(mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1), Some(0));
        // More than a block, and more than the sound.
        let out = bank.render(&mut mixer, 1600);
        assert_eq!(&side(&out, 0)[..1500], &data[..]);
        assert_eq!(&side(&out, 1)[..1500], &data[..]);
        assert!(out[3000..].iter().all(|s| *s == 0));
        assert_eq!(mixer.playing(), 0);
    }

    #[test]
    fn half_the_rate_is_twice_as_long() {
        let data = [1000i16, 3000, -2000, 400, 401];
        let bank = Bank::new(&[(&data, OUTPUT_RATE / 2, false)]);
        let mut mixer = Mixer::<4>::new();
        mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1).unwrap();
        assert_eq!(mixer.voice(0).unwrap().step, 0x8000);
        let out = bank.render(&mut mixer, 12);
        // Each sample, then the point halfway to the next (rounded down); silence follows the last.
        assert_eq!(
            side(&out, 0),
            [1000, 2000, 3000, 500, -2000, -800, 400, 400, 401, 200, 0, 0]
        );
        assert_eq!(mixer.playing(), 0);
    }

    #[test]
    fn an_octave_up_takes_every_second_sample() {
        let data = noise(64);
        let bank = Bank::new(&[(&data, OUTPUT_RATE, false)]);
        let mut mixer = Mixer::<4>::new();
        mixer.play(&bank.sounds, 0, UNITY, UNITY, 512, 1).unwrap();
        let out = bank.render(&mut mixer, 40);
        let want: Vec<i16> = data.iter().step_by(2).copied().collect();
        assert_eq!(&side(&out, 0)[..32], &want[..]);
        assert!(out[64..].iter().all(|s| *s == 0));
    }

    #[test]
    fn a_loop_wraps_without_a_gap() {
        let data = noise(300);
        let bank = Bank::new(&[(&data, OUTPUT_RATE, true)]);
        let mut mixer = Mixer::<4>::new();
        mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1).unwrap();
        let out = bank.render(&mut mixer, 1000);
        for (i, s) in side(&out, 0).iter().enumerate() {
            assert_eq!(*s, data[i % 300], "frame {i}");
        }
        assert_eq!(mixer.playing(), 1);

        // At half the rate the last sample leads into the first.
        let short = [100i16, 300, -500];
        let bank = Bank::new(&[(&short, OUTPUT_RATE / 2, true)]);
        let mut mixer = Mixer::<4>::new();
        mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1).unwrap();
        assert_eq!(
            side(&bank.render(&mut mixer, 9), 0),
            [100, 200, 300, -100, -500, -200, 100, 200, 300]
        );
    }

    #[test]
    fn a_block_in_pieces_is_the_same_block() {
        let data = noise(700);
        let bank = Bank::new(&[(&data, 16_000, true), (&data[..333], 30_000, false)]);
        let start = |mixer: &mut Mixer<4>| {
            mixer.play(&bank.sounds, 0, 200, 90, 301, 1).unwrap();
            mixer.play(&bank.sounds, 1, 256, 256, 197, 1).unwrap();
        };
        let mut whole = Mixer::<4>::new();
        start(&mut whole);
        let want = bank.render(&mut whole, 3000);
        let mut pieces = Mixer::<4>::new();
        start(&mut pieces);
        let mut got = Vec::new();
        for frames in [1usize, 7, 512, 3, 513, 1024, 940] {
            got.extend(bank.render(&mut pieces, frames));
        }
        assert_eq!(got, want);
        assert_eq!(pieces.voice(0), whole.voice(0));
    }

    #[test]
    fn each_side_has_its_gain() {
        let data = [8000i16, -8000, 2001, -1];
        let bank = Bank::new(&[(&data, OUTPUT_RATE, false)]);
        let mut mixer = Mixer::<4>::new();
        let slot = mixer.play(&bank.sounds, 0, 128, 64, UNITY, 1).unwrap();
        let out = bank.render(&mut mixer, 2);
        assert_eq!(out, [4000, 2000, -4000, -2000]);
        // A gain over 256 is 256, and a change holds from the next block.
        mixer.set_gain(slot, 0, 9999);
        let out = bank.render(&mut mixer, 2);
        assert_eq!(out, [0, 2001, 0, -1]);
    }

    #[test]
    fn a_sum_stays_inside_16_bits() {
        let loud = [30_000i16, -30_000, 30_000];
        let bank = Bank::new(&[(&loud, OUTPUT_RATE, false)]);
        let mut mixer = Mixer::<4>::new();
        for _ in 0..3 {
            mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1).unwrap();
        }
        let out = bank.render(&mut mixer, 3);
        assert_eq!(side(&out, 0), [32_767, -32_768, 32_767]);

        // What `out` already holds is part of the sum.
        mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1).unwrap();
        let mut out = [10_000i16, -40, -10_000, 0, -20_000, 5];
        mixer.render(&bank.sounds, bank.samples(), &mut out);
        assert_eq!(out, [32_767, 29_960, -32_768, -30_000, 10_000, 30_005]);
    }

    #[test]
    fn a_new_sound_takes_the_quietest_voice_it_may() {
        let data = noise(4000);
        let bank = Bank::new(&[(&data, OUTPUT_RATE, false)]);
        let mut mixer = Mixer::<3>::new();
        assert_eq!(mixer.play(&bank.sounds, 0, 10, 10, UNITY, 9), Some(0));
        assert_eq!(mixer.play(&bank.sounds, 0, 200, 100, UNITY, 2), Some(1));
        assert_eq!(mixer.play(&bank.sounds, 0, 60, 90, UNITY, 1), Some(2));
        // The quietest voice is of a higher priority: the quietest of the others gives way.
        assert_eq!(mixer.play(&bank.sounds, 0, 256, 256, UNITY, 2), Some(2));
        assert_eq!(mixer.voice(2).unwrap().priority, 2);
        // Under every voice's priority there is none to take.
        assert_eq!(mixer.play(&bank.sounds, 0, 256, 256, UNITY, 1), None);
        // At the highest priority the quietest of all gives way.
        assert_eq!(mixer.play(&bank.sounds, 0, 256, 256, UNITY, 9), Some(0));
        // A voice that ended or was stopped is free again.
        mixer.stop(1);
        assert_eq!(mixer.play(&bank.sounds, 0, 1, 1, UNITY, 0), Some(1));
        assert_eq!(mixer.playing(), 3);
    }

    #[test]
    fn a_long_sound_plays_past_65_536_frames() {
        let data = noise(70_000);
        let bank = Bank::new(&[(&data, OUTPUT_RATE, false)]);
        let mut mixer = Mixer::<1>::new();
        mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1).unwrap();
        let out = bank.render(&mut mixer, 70_010);
        assert_eq!(&side(&out, 1)[..70_000], &data[..]);
        assert_eq!(mixer.playing(), 0);
    }

    #[test]
    fn a_sound_outside_the_section_is_not_played() {
        let data = noise(100);
        let mut bank = Bank::new(&[(&data, OUTPUT_RATE, false)]);
        let mut mixer = Mixer::<2>::new();
        assert_eq!(mixer.play(&bank.sounds, 5, UNITY, UNITY, UNITY, 1), None);
        mixer.play(&bank.sounds, 0, UNITY, UNITY, UNITY, 1).unwrap();
        bank.sounds[0].frames = 101;
        assert!(bank.render(&mut mixer, 8).iter().all(|s| *s == 0));
        assert_eq!(mixer.playing(), 0);
    }

    #[test]
    fn the_ends_of_the_pitch_range_end_too() {
        // No pitch at all still moves, by the smallest step.
        let slow = Bank::new(&[(&[5i16, 6], 1, false)]);
        let mut mixer = Mixer::<1>::new();
        mixer.play(&slow.sounds, 0, UNITY, UNITY, 0, 1).unwrap();
        assert_eq!(mixer.voice(0).unwrap().step, 1);

        // The highest pitch on a 48 kHz sound steps 557.27 frames at once: 2000 frames last 4.
        let data = noise(2000);
        let fast = Bank::new(&[(&data, 48_000, false)]);
        let mut mixer = Mixer::<1>::new();
        mixer
            .play(&fast.sounds, 0, UNITY, UNITY, u16::MAX, 1)
            .unwrap();
        assert_eq!(mixer.voice(0).unwrap().step >> 16, 557);
        let out = fast.render(&mut mixer, 600);
        assert_eq!(out[0], data[0]);
        assert!(out[8..].iter().all(|s| *s == 0));
        assert_eq!(mixer.playing(), 0);
    }
}
