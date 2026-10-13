//! Sound: a thread of its own mixes the playing sounds (`duty_sim::mix`) into blocks and hands
//! each to the output, which holds the thread until it has room for the next. The frame thread
//! says what to play through a queue that neither side waits on.

use core::ffi::c_void;
use core::ptr::{self, addr_of, addr_of_mut};
use core::sync::atomic::{AtomicU32, Ordering};

use duty_pack::sound::{Id, Sound};
use duty_sim::mix::{self, Mixer, OUTPUT_RATE, UNITY};
use psp::sys::*;
use psp::Align16;

/// Frames in a block, 23 ms: what the mixer sums in one pass over its voices.
const BLOCK: usize = mix::BLOCK;
/// Sounds heard at once.
const VOICES: usize = 16;
/// Commands the queue holds (a power of two). The thread empties it before every block.
const QUEUE: usize = 64;
/// The bed's priority. `play` stays under it, so no other sound takes the bed's voice.
const BED: u8 = u8::MAX;

// The channel below is reserved at the rate the mixer steps its voices by.
const _: () = assert!(OUTPUT_RATE == 22_050 && BLOCK == 512);

#[derive(Clone, Copy)]
enum Command {
    Play { id: u16, left: u16, right: u16, pitch: u16, priority: u8 },
    Bed { id: u16, gain: u16 },
    Silence,
}

static mut COMMANDS: [Command; QUEUE] = [Command::Silence; QUEUE];
/// Commands written and commands taken since the start; the difference is what waits. The frame
/// thread alone moves the first and the output thread alone the second.
static WRITTEN: AtomicU32 = AtomicU32::new(0);
static TAKEN: AtomicU32 = AtomicU32::new(0);

static STARTED: AtomicU32 = AtomicU32::new(0);
static RUNNING: AtomicU32 = AtomicU32::new(0);
/// The frame thread asks the output thread to let go of the sections: 1 asked, 2 let go, 0 playing.
static HOLD: AtomicU32 = AtomicU32::new(0);

// What the output thread works on. `start` writes the two sections before the thread exists, and
// again for another map while the thread has let go of them (`rest`); the mixer is the thread's alone.
static mut MIXER: Mixer<VOICES> = Mixer::new();
static mut SOUNDS: &[Sound] = &[];
static mut SAMPLES: &[u8] = &[];

/// Starts the output thread on the pack's `SNDS` and `SNDD` sections. False when the thread
/// could not be made; whether it got the output channel is [`running`].
///
/// # Safety
/// From the frame thread, as every other function here.
pub unsafe fn start(sounds: &'static [Sound], samples: &'static [u8]) -> bool {
    addr_of_mut!(SOUNDS).write(sounds);
    addr_of_mut!(SAMPLES).write(samples);
    if STARTED.load(Ordering::Relaxed) != 0 {
        // Another map's sections: the thread, which let go of the last map's (`rest`), takes these.
        HOLD.store(0, Ordering::Release);
        return true;
    }
    if sounds.is_empty() {
        return false;
    }
    // Above the frame thread's priority (a lower number): a block is due every 23 ms whatever a frame takes.
    let id = sceKernelCreateThread(b"duty_audio\0".as_ptr(), output, 24, 32 * 1024, ThreadAttributes::USER, ptr::null_mut());
    if id.0 < 0 || sceKernelStartThread(id, 0, ptr::null_mut()) < 0 {
        return false;
    }
    STARTED.store(1, Ordering::Relaxed);
    true
}

/// Has the output thread let go of the sound sections, whose memory another map is about to take:
/// every sound stops, and it plays silence until `start` hands it the next map's. Returns once it has.
pub unsafe fn rest() {
    if !running() {
        return;
    }
    HOLD.store(1, Ordering::Release);
    while HOLD.load(Ordering::Acquire) != 2 {
        sceKernelDelayThread(1_000);
    }
}

/// True once the thread holds the output channel.
pub fn running() -> bool {
    RUNNING.load(Ordering::Relaxed) == 1
}

/// Plays `id` once: gains of the two sides 0…256, `pitch` 256 as recorded. When 16 sounds play,
/// the quietest of `priority` or lower gives way, and without one this sound is not played.
pub fn play(id: Id, left: u16, right: u16, pitch: u16, priority: u8) {
    push(Command::Play { id: id as u16, left, right, pitch, priority: priority.min(BED - 1) });
}

/// Starts the looping bed `id` at `gain` (0…256) on both sides, or sets the gain of the bed
/// when it plays `id` already. A gain of 0 stops it.
pub fn ambience(id: Id, gain: u16) {
    push(Command::Bed { id: id as u16, gain });
}

/// Stops every sound and the bed (the game is paused). `ambience` starts the bed again.
pub fn silence() {
    push(Command::Silence);
}

/// Leaves a command for the output thread. A command that finds the queue full is dropped.
fn push(command: Command) {
    let written = WRITTEN.load(Ordering::Relaxed);
    if written.wrapping_sub(TAKEN.load(Ordering::Acquire)) as usize >= QUEUE {
        return;
    }
    // SAFETY: the frame thread alone writes the queue, and the output thread reads this place
    // only after `WRITTEN` has moved past it.
    unsafe { (addr_of_mut!(COMMANDS) as *mut Command).add(written as usize % QUEUE).write(command) };
    WRITTEN.store(written.wrapping_add(1), Ordering::Release);
}

/// Does what the frame thread asked for since the last block. `bed` is the voice the bed plays
/// on and its sound.
unsafe fn take(mixer: &mut Mixer<VOICES>, sounds: &[Sound], bed: &mut Option<(usize, u16)>) {
    let written = WRITTEN.load(Ordering::Acquire);
    let mut taken = TAKEN.load(Ordering::Relaxed);
    while taken != written {
        // SAFETY: the frame thread wrote this place before it moved `WRITTEN` past it, and
        // writes it again only after `TAKEN` has passed it.
        let command = (addr_of!(COMMANDS) as *const Command).add(taken as usize % QUEUE).read();
        taken = taken.wrapping_add(1);
        match command {
            Command::Play { id, left, right, pitch, priority } => {
                mixer.play(sounds, id, left, right, pitch, priority);
            }
            Command::Bed { id, gain } => {
                // The bed's voice, while it still plays the bed's sound.
                let held = bed.filter(|&(slot, sound)| mixer.voice(slot).is_some_and(|v| v.sound == sound));
                match held {
                    Some((slot, sound)) if sound == id && gain > 0 => mixer.set_gain(slot, gain, gain),
                    _ => {
                        if let Some((slot, _)) = held {
                            mixer.stop(slot);
                        }
                        *bed = if gain > 0 { mixer.play(sounds, id, gain, gain, UNITY, BED).map(|slot| (slot, id)) } else { None };
                    }
                }
            }
            Command::Silence => {
                mixer.stop_all();
                *bed = None;
            }
        }
    }
    TAKEN.store(taken, Ordering::Release);
}

unsafe extern "C" fn output(_: usize, _: *mut c_void) -> i32 {
    if sceAudioSRCChReserve(BLOCK as i32, AudioOutputFrequency::Khz22_05, 2) < 0 {
        return 0;
    }
    RUNNING.store(1, Ordering::Relaxed);
    // SAFETY: see the three statics.
    let mixer = &mut *addr_of_mut!(MIXER);
    let mut block = Align16([0i16; BLOCK * 2]);
    let mut bed = None;
    loop {
        // The mixer adds to what the block holds.
        block.0 = [0; BLOCK * 2];
        if HOLD.load(Ordering::Acquire) != 0 {
            // A change of map: nothing plays and no section is read until the next map's are there.
            mixer.stop_all();
            bed = None;
            HOLD.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire).ok();
        } else {
            // (the sections are the held map's: read where they are now)
            let sounds = addr_of!(SOUNDS).read();
            let samples = addr_of!(SAMPLES).read();
            take(mixer, sounds, &mut bed);
            mixer.render(sounds, samples, &mut block.0);
        }
        if sceAudioSRCOutputBlocking(0x8000, block.0.as_mut_ptr() as *mut c_void) < 0 {
            // The output took nothing and held nobody: wait a block's time, or this loop would
            // keep the frame thread from running.
            sceKernelDelayThread((BLOCK as u32 * 1_000_000) / OUTPUT_RATE);
        }
    }
}
