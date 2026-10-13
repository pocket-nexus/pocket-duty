//! Memory at its exact size.
//!
//! PocketJS's arena is the program's allocator (`pocketjs-psp` installs it):
//! one kernel block, handed out in power-of-two classes so that QuickJS's
//! churn recycles in constant time. A 4.8 MB vertex buffer from it would take
//! 8 MB, and the arena takes nothing back that it cut at an exact size.
//!
//! So a map's sections lie in two stretches cut once: what the 2 MB the arena
//! leaves the kernel have beyond `KEEP` (one kernel block), and a stretch of
//! the arena's uncarved tail as long as the largest map needs. A map's
//! sections fill them from their starts; another map takes them over from the
//! start again (`release_map`). The interface's pak, which stays for the run,
//! comes from `permanent`.
//!
//! Only the frame thread allocates.

use alloc::vec::Vec;
use core::ffi::c_void;
use core::ptr;

use pocketjs_psp::arena;
use psp::sys::{self, SceSysMemBlockTypes, SceSysMemPartitionId, SceUid};

/// What stays with the kernel: the stacks of the threads started after loading (the reader's, the
/// sound's) and its own objects.
const KEEP: usize = 320 * 1024;
/// Below this a request is not worth a kernel object.
const SMALL: usize = 64 * 1024;

/// Bytes held in kernel blocks.
pub static mut KERNEL_BYTES: usize = 0;
/// Bytes a test run keeps from the program (`hold_back`).
pub static mut HELD_BACK: usize = 0;

/// Keeps `bytes` of the kernel's memory from the program, as one block that is never used. Call
/// before anything allocates: the arena takes its block at the first allocation, as long as the
/// kernel's largest free block less 2 MB, so a run started with more free memory than another kind of
/// start has (PSPLINK or an emulator against the XMB) gets that start's arena and that start's
/// remainder. False when the kernel has no such block.
pub unsafe fn hold_back(bytes: usize) -> bool {
    let id = sys::sceKernelAllocPartitionMemory(SceSysMemPartitionId::SceKernelPrimaryUserPartition, b"held\0".as_ptr(), SceSysMemBlockTypes::Low, bytes as u32, ptr::null_mut::<c_void>());
    if id.0 >= 0 {
        HELD_BACK += bytes;
    }
    id.0 >= 0
}

/// A kernel block of `bytes` while the kernel keeps `KEEP` after it: its address and id.
unsafe fn kernel_block(bytes: usize) -> Option<(*mut u8, SceUid)> {
    // The arena takes its block on first use; until then the kernel's free memory is not what is left over.
    let _ = arena::stats();
    if (sys::sceKernelMaxFreeMemSize() as usize) < bytes + KEEP {
        return None;
    }
    let id = sys::sceKernelAllocPartitionMemory(SceSysMemPartitionId::SceKernelPrimaryUserPartition, b"duty\0".as_ptr(), SceSysMemBlockTypes::Low, bytes as u32, ptr::null_mut::<c_void>());
    if id.0 < 0 {
        return None;
    }
    let p = sys::sceKernelGetBlockHeadAddr(id) as *mut u8;
    (!p.is_null()).then_some((p, id))
}

/// `bytes` of uninitialized memory for the rest of the run, 64-byte aligned. Null when there is none.
pub unsafe fn permanent(bytes: usize) -> *mut u8 {
    let bytes = bytes.max(16);
    if bytes >= SMALL {
        // Kernel blocks start on 256-byte boundaries.
        if let Some((p, _)) = kernel_block(bytes) {
            KERNEL_BYTES += bytes;
            return p;
        }
    }
    arena::alloc_permanent(bytes, 64)
}

// ---------------------------------------------------------------- one map's memory

/// A stretch of memory: where it starts, its length, and how much of it the held map's sections take.
#[derive(Clone, Copy)]
struct Stretch {
    base: usize,
    bytes: usize,
    used: usize,
}

/// The kernel's stretch, then the arena's. A base of 0 is a stretch not cut yet.
static mut STRETCHES: [Stretch; 2] = [Stretch { base: 0, bytes: 0, used: 0 }; 2];
/// The second stretch is this much longer than the sections' sizes call for: a section's records can
/// take a few bytes more than its size says (an empty one takes a record).
const SLACK: usize = 4096;

/// What a request takes of a stretch: 64-byte aligned, 16 bytes at least.
const fn taken(bytes: usize) -> usize {
    ((if bytes < 16 { 16 } else { bytes }) + 63) & !63
}

/// Bytes of the second stretch that sections of these sizes take, read in this order, when the first
/// stretch is `first` long: each goes to the first stretch while it fits there (`of_map` does the same).
pub fn tail_needed(sizes: impl Iterator<Item = usize>, first: usize) -> usize {
    let (mut a, mut b) = (0, SLACK);
    for bytes in sizes {
        if a + taken(bytes) <= first {
            a += taken(bytes);
        } else {
            b += taken(bytes);
        }
    }
    b
}

/// The kernel's stretch, cut on the first call: all the kernel can give as one block beyond what it
/// keeps. Its length, 0 when the kernel has less than a block is worth.
pub unsafe fn kernel_stretch() -> usize {
    let stretch = &mut (*ptr::addr_of_mut!(STRETCHES))[0];
    if stretch.base == 0 {
        // (kernel blocks start on 256-byte boundaries)
        let room = kernel_room() & !255;
        *stretch = match if room >= SMALL { kernel_block(room) } else { None } {
            Some((p, _)) => {
                KERNEL_BYTES += room;
                Stretch { base: p as usize, bytes: room, used: 0 }
            }
            None => Stretch { base: usize::MAX, bytes: 0, used: 0 },
        };
    }
    stretch.bytes
}

/// Cuts the arena's stretch, `bytes` long, once. False when the arena's tail is shorter than that.
pub unsafe fn reserve_tail(bytes: usize) -> bool {
    let stretch = &mut (*ptr::addr_of_mut!(STRETCHES))[1];
    if stretch.base != 0 {
        return stretch.bytes >= bytes;
    }
    let p = arena::alloc_permanent(bytes.max(64), 64);
    if p.is_null() {
        return false;
    }
    *stretch = Stretch { base: p as usize, bytes: bytes.max(64), used: 0 };
    true
}

/// The arena's stretch as it was cut.
pub unsafe fn tail_reserved() -> usize {
    (*ptr::addr_of!(STRETCHES))[1].bytes
}

/// Bytes of the two stretches the held map's sections take.
pub unsafe fn map_bytes() -> usize {
    (*ptr::addr_of!(STRETCHES)).iter().map(|stretch| stretch.used).sum()
}

/// `bytes` of the held map's memory, 64-byte aligned and uninitialized. Null when neither stretch has them.
pub unsafe fn of_map(bytes: usize) -> *mut u8 {
    for stretch in (*ptr::addr_of_mut!(STRETCHES)).iter_mut() {
        if stretch.base != 0 && stretch.used + taken(bytes) <= stretch.bytes {
            let p = stretch.base + stretch.used;
            stretch.used += taken(bytes);
            return p as *mut u8;
        }
    }
    ptr::null_mut()
}

/// A vector of capacity `n` over the held map's memory, empty. It must not grow and is never dropped:
/// its memory goes with the map.
pub unsafe fn map_vec<T: Copy>(n: usize) -> Option<Vec<T>> {
    let p = of_map(n.max(1) * core::mem::size_of::<T>());
    (!p.is_null()).then(|| Vec::from_raw_parts(p.cast(), 0, n.max(1)))
}

/// Hands the held map's memory to the next map. Nothing may point into it any more.
pub unsafe fn release_map() {
    for stretch in (*ptr::addr_of_mut!(STRETCHES)).iter_mut() {
        stretch.used = 0;
    }
}

/// Bytes the kernel can still give as one block, beyond what it keeps.
pub unsafe fn kernel_room() -> usize {
    let _ = arena::stats();
    (sys::sceKernelMaxFreeMemSize() as usize).saturating_sub(KEEP)
}
