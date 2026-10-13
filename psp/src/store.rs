//! Storage: a map's pack with its sections read one by one, the files beside
//! it (the interface's bundle and what it asked to have kept), and the PSPLINK
//! mailbox.
//!
//! Each section is read straight into memory of its exact size (`mem`), which
//! keeps it for the rest of the run: the program holds one map's pack and
//! cannot give it back. So another map is another run. `map.txt` beside the
//! packs names the map a run reads (`locate`); when the interface asks for
//! another, the name is written there and the program is started again
//! (`change`, `serve`).
//!
//! The kernel keeps a working directory per thread, and only the thread the
//! program started on has one (the EBOOT's directory): a path without a
//! device fails anywhere else. So that thread opens what lies beside the
//! EBOOT before the frame thread starts (`locate`), and it is the one that
//! writes there. After that it serves the mailbox (a status file written, a
//! control file read, on the computer's share) and writes what the interface
//! asked to have kept (`serve`), at a lower priority than the frame's: it
//! runs while the frame waits for the GE or the display, so host I/O never
//! delays a frame.

use alloc::vec::Vec;
use core::ffi::c_void;
use core::mem::ManuallyDrop;
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

use duty_interface::maps::{Map, MAPS};
use psp::sys::*;

use crate::mem;

/// Records in memory of exactly their size, which keeps them for the rest of the run (`mem::vec`).
/// The allocator did not hand that memory out, so the vector is never dropped and never grows.
pub type Kept<T> = ManuallyDrop<Vec<T>>;

pub struct Section {
    pub tag: u32,
    pub offset: u32,
    pub size: u32,
}

pub struct PackFile {
    pub fd: SceUid,
    pub sections: Vec<Section>,
    pub bytes: u32,
    /// Whether the computer's share is there (PSPLINK): the pack came from it, or it holds `duty/boot.txt`.
    pub host: bool,
}

/// The pack sits beside the EBOOT, and so do the files that go with it; otherwise they are on the
/// computer's share.
static mut LOCAL: bool = true;
/// What `locate` opened: the pack, the interface's script and pak, what the interface kept.
static mut FOUND: [SceUid; 4] = [SceUid(-1); 4];
const PACK: usize = 0;
pub const SCRIPT: usize = 1;
pub const PAK: usize = 2;
const KEPT: usize = 3;

/// The computer's share, where the files are when they are not beside the EBOOT.
const SHARE: &[u8] = b"host0:/duty/";
/// The file that names the map a run reads.
const CHOICE: &[u8] = b"map.txt";

/// The map whose pack `locate` opened, as an index of `MAPS`, and the maps a pack was found for, a bit each.
static mut HELD: usize = 0;
static mut HAVE: u32 = 1;
/// The run was started by the last one for a match on another map: it begins in the match.
static mut PLAYS: bool = false;
/// What `map.txt` holds after a map's name when the next run is to begin in a match.
const PLAY: &str = " play";

/// A file's path with a NUL after it: beside the EBOOT, or on the computer's share.
fn path(local: bool, name: &[u8]) -> [u8; 64] {
    let mut out = [0u8; 64];
    let mut at = 0;
    for part in [if local { &[][..] } else { SHARE }, name] {
        let n = part.iter().position(|b| *b == 0).unwrap_or(part.len()).min(63 - at);
        out[at..at + n].copy_from_slice(&part[..n]);
        at += n;
    }
    out
}

unsafe fn open(local: bool, name: &[u8]) -> SceUid {
    sceIoOpen(path(local, name).as_ptr(), IoOpenFlags::RD_ONLY, 0)
}

/// Opens a map's pack and the files that go with it, beside the EBOOT (a packaged copy) or else on the
/// computer's share. The map is the one `map.txt` names when its pack is there, else the first. Call on
/// the thread the program started on, before the frame thread exists. It allocates nothing.
pub unsafe fn locate() {
    // The first map's pack says where everything is: every copy of the game has it.
    let mut name = [0u8; 32];
    MAPS[0].pack_name(&mut name);
    let first = open(true, &name);
    LOCAL = first.0 >= 0;
    if first.0 >= 0 {
        sceIoClose(first);
    }
    // The maps there is a pack for.
    HAVE = 0;
    for (i, map) in MAPS.iter().enumerate().take(32) {
        map.pack_name(&mut name);
        let fd = open(LOCAL, &name);
        if fd.0 >= 0 {
            sceIoClose(fd);
            HAVE |= 1 << i;
        }
    }
    // The map chosen in an earlier run.
    HELD = 0;
    let choice = open(LOCAL, CHOICE);
    if choice.0 >= 0 {
        let mut text = [0u8; 32];
        let n = sceIoRead(choice, text.as_mut_ptr() as *mut c_void, text.len() as u32).max(0) as usize;
        sceIoClose(choice);
        let note = core::str::from_utf8(&text[..n]).unwrap_or("").trim_end();
        let id = note.strip_suffix(PLAY).unwrap_or(note);
        if let Some(i) = MAPS.iter().position(|m| m.id == id).filter(|i| *i < 32 && HAVE & (1 << i) != 0) {
            HELD = i;
            PLAYS = note.len() != id.len();
        }
        // The match is begun once: a later start of the program opens on this map's title.
        if note.len() != id.len() {
            write_choice(id.as_bytes(), b"");
        }
    }
    MAPS[HELD].pack_name(&mut name);
    FOUND[PACK] = open(LOCAL, &name);
    for (file, name) in [(SCRIPT, &b"duty.js"[..]), (PAK, b"duty.pak"), (KEPT, b"interface.json")] {
        FOUND[file] = open(LOCAL, name);
    }
}

/// Writes `map.txt`: a map's name, and what follows it.
unsafe fn write_choice(id: &[u8], then: &[u8]) {
    let fd = sceIoOpen(path(LOCAL, CHOICE).as_ptr(), IoOpenFlags::WR_ONLY | IoOpenFlags::CREAT | IoOpenFlags::TRUNC, 0o644);
    if fd.0 >= 0 {
        sceIoWrite(fd, id.as_ptr() as *const c_void, id.len());
        sceIoWrite(fd, then.as_ptr() as *const c_void, then.len());
        sceIoClose(fd);
    }
}

/// The map whose pack this run reads.
pub fn map() -> &'static Map {
    unsafe { &MAPS[HELD] }
}

/// Whether the last run started this one for a match on this map.
pub fn plays() -> bool {
    unsafe { PLAYS }
}

/// The maps a pack was found for, in the list's order.
pub fn maps() -> impl Iterator<Item = &'static Map> {
    let have = unsafe { HAVE };
    MAPS.iter().enumerate().filter(move |(i, _)| *i < 32 && have & (1 << i) != 0).map(|(_, map)| map)
}

/// One of the files `locate` opened, once: its handle and length.
unsafe fn found(file: usize) -> Option<(SceUid, usize)> {
    let fd = core::mem::replace(&mut FOUND[file], SceUid(-1));
    if fd.0 < 0 {
        return None;
    }
    let n = sceIoLseek32(fd, 0, IoWhence::End).max(0) as usize;
    sceIoLseek32(fd, 0, IoWhence::Set);
    Some((fd, n))
}

/// A file beside the pack (`SCRIPT`), whole, with `tail` appended, in memory the allocator takes back.
pub unsafe fn read_beside(file: usize, tail: &[u8]) -> Option<Vec<u8>> {
    let (fd, n) = found(file)?;
    let mut v = Vec::<u8>::new();
    let ok = n > 0 && v.try_reserve_exact(n + tail.len()).is_ok() && read_exact(fd, v.as_mut_ptr(), n);
    sceIoClose(fd);
    if !ok {
        return None;
    }
    v.set_len(n);
    v.extend_from_slice(tail);
    Some(v)
}

/// A file beside the pack (`PAK`), whole, in memory that keeps it for the rest of the run.
pub unsafe fn keep_beside(file: usize) -> Option<&'static [u8]> {
    let (fd, n) = found(file)?;
    let p = if n > 0 { mem::permanent(n) } else { ptr::null_mut() };
    let ok = !p.is_null() && read_exact(fd, p, n);
    sceIoClose(fd);
    ok.then(|| core::slice::from_raw_parts(p as *const u8, n))
}

/// Closes a file `locate` opened that nothing will read.
pub unsafe fn leave_beside(file: usize) {
    if let Some((fd, _)) = found(file) {
        sceIoClose(fd);
    }
}

/// Reads exactly `len` bytes; storage returns short reads on large requests.
pub unsafe fn read_exact(fd: SceUid, dst: *mut u8, len: usize) -> bool {
    let mut at = 0;
    while at < len {
        let n = sceIoRead(fd, dst.add(at) as *mut c_void, (len - at).min(256 * 1024) as u32);
        if n <= 0 {
            return false;
        }
        at += n as usize;
    }
    true
}

impl PackFile {
    /// The pack `locate` opened: its section table.
    pub unsafe fn open() -> Result<PackFile, &'static str> {
        let (fd, bytes) = found(PACK).ok_or("no pack beside the program or on host0:/duty")?;
        let mut head = [0u32; 4];
        if !read_exact(fd, head.as_mut_ptr().cast(), 16) || head[0] != duty_pack::MAGIC {
            return Err("duty.pack is not a pack");
        }
        if head[1] != duty_pack::VERSION {
            return Err("duty.pack has another version");
        }
        let n = head[2] as usize;
        if n > 64 {
            return Err("duty.pack section table");
        }
        let mut table = alloc::vec![0u32; n * 4];
        if !read_exact(fd, table.as_mut_ptr().cast(), n * 16) {
            return Err("duty.pack section table");
        }
        let sections = (0..n).map(|i| Section { tag: table[i * 4], offset: table[i * 4 + 1], size: table[i * 4 + 2] }).collect();
        // The mailbox is there when the computer's share has the game's directory, wherever the pack came from.
        let probe = sceIoOpen(b"host0:/duty/boot.txt\0".as_ptr(), IoOpenFlags::RD_ONLY, 0);
        if probe.0 >= 0 {
            sceIoClose(probe);
        }
        Ok(PackFile { fd, sections, bytes: bytes as u32, host: !LOCAL || probe.0 >= 0 })
    }

    /// Bytes of the sections: all of them stay in memory.
    pub fn resident_bytes(&self) -> usize {
        self.sections.iter().map(|s| s.size as usize).sum()
    }

    pub fn section(&self, tag: u32) -> Result<&Section, &'static str> {
        self.sections.iter().find(|s| s.tag == tag).ok_or("the pack lacks a section")
    }

    /// Reads `len` bytes at `offset` of a section into `dst`.
    pub unsafe fn read_into(&self, tag: u32, offset: usize, dst: *mut u8, len: usize) -> Result<(), &'static str> {
        let s = self.section(tag)?;
        if offset + len > s.size as usize {
            return Err("read past a section's end");
        }
        sceIoLseek32(self.fd, (s.offset as usize + offset) as i32, IoWhence::Set);
        if read_exact(self.fd, dst, len) {
            Ok(())
        } else {
            Err("reading the pack failed")
        }
    }

    /// A whole section as `T` records, in memory of exactly its size that keeps it for the rest of the run.
    pub unsafe fn resident<T: Copy>(&self, tag: u32) -> Result<Kept<T>, &'static str> {
        let s = self.section(tag)?;
        let n = s.size as usize / core::mem::size_of::<T>();
        let mut v = ManuallyDrop::new(mem::vec::<T>(n).ok_or("no memory for the pack")?);
        self.read_into(tag, 0, v.as_mut_ptr().cast(), n * core::mem::size_of::<T>())?;
        v.set_len(n);
        Ok(v)
    }

    /// A whole section as `T` records (`T` is plain data), in memory the allocator takes back: a small
    /// table, or one read to be parsed.
    pub unsafe fn records<T: Copy>(&self, tag: u32) -> Result<Vec<T>, &'static str> {
        let s = self.section(tag)?;
        let n = s.size as usize / core::mem::size_of::<T>();
        let mut v = Vec::<T>::with_capacity(n.max(1));
        self.read_into(tag, 0, v.as_mut_ptr().cast(), n * core::mem::size_of::<T>())?;
        v.set_len(n);
        Ok(v)
    }
}

// ---------------------------------------------------------------- the mailbox

static mut HOST: bool = false;
/// The kernel's answer when a start-up call fails, for the failure record.
pub static mut LAST_CODE: i32 = 0;

// The mailbox: the frame thread hands a status text over and takes control text back.
const STATUS_MAX: usize = 4096;
static mut STATUS: [u8; STATUS_MAX] = [0; STATUS_MAX];
static mut STATUS_LEN: usize = 0;
static STATUS_FLAG: AtomicU32 = AtomicU32::new(0);
const CONTROL_MAX: usize = 512;
static mut CONTROL: [u8; CONTROL_MAX] = [0; CONTROL_MAX];
static mut CONTROL_LEN: usize = 0;
static CONTROL_FLAG: AtomicU32 = AtomicU32::new(0);

// What the interface asked to have kept, on its way to `interface.json` beside the pack.
pub const KEPT_MAX: usize = 1024;
static mut KEPT_TEXT: [u8; KEPT_MAX] = [0; KEPT_MAX];
static mut KEPT_LEN: usize = 0;
static KEPT_FLAG: AtomicU32 = AtomicU32::new(0);

// Another map: 0 none asked for, 1 asked for (`CHANGE_TO` of `MAPS`), 2 its name is written and the
// kernel would not start the program again.
static CHANGE_FLAG: AtomicU32 = AtomicU32::new(0);
static mut CHANGE_TO: usize = 0;

/// Asks for a run on another map: `serve` writes its name to `map.txt` and starts the program again.
pub unsafe fn change(to: &'static Map) {
    if let Some(i) = MAPS.iter().position(|m| m.id == to.id) {
        CHANGE_TO = i;
        CHANGE_FLAG.store(1, Ordering::Release);
    }
}

/// Whether the map asked for is written down and the program was not started again: the next run reads
/// it, and a person has to start that run.
pub fn change_waits() -> bool {
    CHANGE_FLAG.load(Ordering::Acquire) == 2
}

/// Writes the map's name where the next run reads it, and starts the program again. Returns only when
/// the kernel would not. On the thread the program started on: the only one with a working directory.
///
/// The program's file is named with its whole path: the kernel hands the path to the next run as its
/// first argument, and that run's working directory comes from it. The path is not ours to ask for, so
/// the places a copy stands are tried: where a package puts it on a Memory Stick or in a PSP Go's
/// storage (tools/psp.ts `package`), and where PPSSPP mounts the directory it was started from. Anywhere
/// else nothing is started.
unsafe fn start_again(to: &'static Map) {
    write_choice(to.id.as_bytes(), PLAY.as_bytes());
    for directory in [&b"ms0:/PSP/GAME/PocketDuty/"[..], b"ef0:/PSP/GAME/PocketDuty/", b"umd0:/"] {
        let mut name = [0u8; 64];
        name[..directory.len()].copy_from_slice(directory);
        if LOCAL {
            // The files are beside the program: a directory is this program's when the name just
            // written is read back from it.
            name[directory.len()..directory.len() + CHOICE.len()].copy_from_slice(CHOICE);
            let fd = sceIoOpen(name.as_ptr(), IoOpenFlags::RD_ONLY, 0);
            if fd.0 < 0 {
                continue;
            }
            let mut text = [0u8; 32];
            let n = sceIoRead(fd, text.as_mut_ptr() as *mut c_void, text.len() as u32).max(0) as usize;
            sceIoClose(fd);
            if text[..n].strip_suffix(PLAY.as_bytes()) != Some(to.id.as_bytes()) {
                continue;
            }
        } else if directory != b"umd0:/" {
            // The files are on the computer's share: the program is PSPLINK's to start, or an emulator's.
            continue;
        }
        name[directory.len()..directory.len() + 10].copy_from_slice(b"EBOOT.PBP\0");
        LAST_CODE = sceKernelLoadExec(name.as_ptr(), ptr::null_mut());
    }
    // In a directory with another name the program is not started again: a run started by a path
    // without its directory has no working directory, and finds none of its files. The name is
    // written down, and the screen says to start the program again (`change_waits`).
}

/// Tells `serve` whether the computer's share is there.
pub unsafe fn start(pack: &PackFile) {
    HOST = pack.host;
}

/// Serves storage for the rest of the run: what the interface asked to have kept, and the mailbox.
/// The thread the program started on runs it, below the frame thread's priority. It allocates nothing.
pub unsafe fn serve() -> ! {
    let mut last_control = [0u8; CONTROL_MAX];
    let mut last_len = usize::MAX;
    let mut beat = 0u32;
    loop {
        sceKernelDelayThread(200_000);
        if KEPT_FLAG.load(Ordering::Acquire) == 1 {
            let path: &[u8] = if LOCAL { b"interface.json\0" } else { b"host0:/duty/interface.json\0" };
            let fd = sceIoOpen(path.as_ptr(), IoOpenFlags::WR_ONLY | IoOpenFlags::CREAT | IoOpenFlags::TRUNC, 0o644);
            if fd.0 >= 0 {
                sceIoWrite(fd, ptr::addr_of!(KEPT_TEXT) as *const c_void, KEPT_LEN);
                sceIoClose(fd);
            }
            KEPT_FLAG.store(0, Ordering::Release);
        }
        // (after what the interface asked to have kept: a run that ends here has written it)
        if CHANGE_FLAG.load(Ordering::Acquire) == 1 {
            start_again(&MAPS[CHANGE_TO]);
            CHANGE_FLAG.store(2, Ordering::Release);
        }
        if !HOST {
            continue;
        }
        if STATUS_FLAG.load(Ordering::Acquire) == 1 {
            let fd = sceIoOpen(b"host0:/duty/status.json\0".as_ptr(), IoOpenFlags::WR_ONLY | IoOpenFlags::CREAT | IoOpenFlags::TRUNC, 0o644);
            if fd.0 >= 0 {
                sceIoWrite(fd, ptr::addr_of!(STATUS) as *const c_void, STATUS_LEN);
                sceIoClose(fd);
            }
            STATUS_FLAG.store(0, Ordering::Release);
        }
        beat += 1;
        if beat % 2 == 0 && CONTROL_FLAG.load(Ordering::Acquire) == 0 {
            let fd = sceIoOpen(b"host0:/duty/control.txt\0".as_ptr(), IoOpenFlags::RD_ONLY, 0);
            if fd.0 >= 0 {
                let mut buf = [0u8; CONTROL_MAX];
                let n = sceIoRead(fd, buf.as_mut_ptr() as *mut c_void, CONTROL_MAX as u32).max(0) as usize;
                sceIoClose(fd);
                // What is there at launch is left over from an earlier run: only changes after it count.
                if last_len == usize::MAX {
                    last_control = buf;
                    last_len = n;
                } else if n != last_len || buf[..n] != last_control[..n] {
                    last_control = buf;
                    last_len = n;
                    CONTROL = buf;
                    CONTROL_LEN = n;
                    CONTROL_FLAG.store(1, Ordering::Release);
                }
            }
        }
    }
}

/// Hands what the interface asked to have kept to `serve`, which writes it beside the pack.
/// False while the previous one is still being written: the caller offers it again.
pub unsafe fn keep(text: &str) -> bool {
    if KEPT_FLAG.load(Ordering::Acquire) != 0 {
        return false;
    }
    let n = text.len().min(KEPT_MAX);
    ptr::copy_nonoverlapping(text.as_ptr(), ptr::addr_of_mut!(KEPT_TEXT) as *mut u8, n);
    KEPT_LEN = n;
    KEPT_FLAG.store(1, Ordering::Release);
    true
}

/// What was kept in an earlier run, read once at the start.
pub unsafe fn kept(buf: &mut [u8]) -> Option<&str> {
    let (fd, _) = found(KEPT)?;
    let n = sceIoRead(fd, buf.as_mut_ptr() as *mut c_void, buf.len() as u32).max(0) as usize;
    sceIoClose(fd);
    core::str::from_utf8(&buf[..n]).ok()
}

/// Hands a status record to the mailbox; dropped when the previous one is still being written.
pub unsafe fn publish(text: &str) {
    if !HOST || STATUS_FLAG.load(Ordering::Acquire) != 0 {
        return;
    }
    let n = text.len().min(STATUS_MAX);
    ptr::copy_nonoverlapping(text.as_ptr(), ptr::addr_of_mut!(STATUS) as *mut u8, n);
    STATUS_LEN = n;
    STATUS_FLAG.store(1, Ordering::Release);
}

/// New control text from the computer, once.
pub unsafe fn control(f: impl FnOnce(&str)) {
    if CONTROL_FLAG.load(Ordering::Acquire) != 1 {
        return;
    }
    let bytes = core::slice::from_raw_parts(ptr::addr_of!(CONTROL) as *const u8, CONTROL_LEN);
    if let Ok(text) = core::str::from_utf8(bytes) {
        f(text);
    }
    CONTROL_FLAG.store(0, Ordering::Release);
}

/// A small text file from the computer, read once (`host0:/duty/boot.txt`: control words applied at start).
pub unsafe fn boot_text(buf: &mut [u8]) -> Option<&str> {
    let fd = sceIoOpen(b"host0:/duty/boot.txt\0".as_ptr(), IoOpenFlags::RD_ONLY, 0);
    if fd.0 < 0 {
        return None;
    }
    let n = sceIoRead(fd, buf.as_mut_ptr() as *mut c_void, buf.len() as u32).max(0) as usize;
    sceIoClose(fd);
    core::str::from_utf8(&buf[..n]).ok()
}

/// Writes a frame the frame thread copied out of video memory to `host0:/duty/shot.raw`.
pub unsafe fn write_shot(pixels: &[u8]) {
    let fd = sceIoOpen(b"host0:/duty/shot.raw\0".as_ptr(), IoOpenFlags::WR_ONLY | IoOpenFlags::CREAT | IoOpenFlags::TRUNC, 0o644);
    if fd.0 >= 0 {
        let mut at = 0;
        while at < pixels.len() {
            let n = sceIoWrite(fd, pixels.as_ptr().add(at) as *const c_void, (pixels.len() - at).min(64 * 1024));
            if n <= 0 {
                break;
            }
            at += n as usize;
        }
        sceIoClose(fd);
    }
}

/// Writes a film's frame to `host0:/duty/film/<index>.raw`, five digits, in the frame thread.
pub unsafe fn write_frame(index: u32, pixels: &[u8]) {
    let mut name = *b"host0:/duty/film/00000.raw\0";
    let mut n = index;
    for digit in (17..22).rev() {
        name[digit] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    if index == 0 {
        sceIoMkdir(b"host0:/duty/film\0".as_ptr(), 0o755);
    }
    let fd = sceIoOpen(name.as_ptr(), IoOpenFlags::WR_ONLY | IoOpenFlags::CREAT | IoOpenFlags::TRUNC, 0o644);
    if fd.0 >= 0 {
        let mut at = 0;
        while at < pixels.len() {
            let n = sceIoWrite(fd, pixels.as_ptr().add(at) as *const c_void, (pixels.len() - at).min(64 * 1024));
            if n <= 0 {
                break;
            }
            at += n as usize;
        }
        sceIoClose(fd);
    }
}

/// Writes a small text file for the computer (`name` ends in a NUL).
pub unsafe fn write_text(name: &[u8], text: &str) {
    let fd = sceIoOpen(name.as_ptr(), IoOpenFlags::WR_ONLY | IoOpenFlags::CREAT | IoOpenFlags::TRUNC, 0o644);
    if fd.0 >= 0 {
        sceIoWrite(fd, text.as_ptr() as *const c_void, text.len());
        sceIoClose(fd);
    }
}

/// Writes one message for the computer while loading (the frame loop is not running yet).
pub unsafe fn note(text: &str) {
    let fd = sceIoOpen(b"host0:/duty/status.json\0".as_ptr(), IoOpenFlags::WR_ONLY | IoOpenFlags::CREAT | IoOpenFlags::TRUNC, 0o644);
    if fd.0 >= 0 {
        sceIoWrite(fd, text.as_ptr() as *const c_void, text.len());
        sceIoClose(fd);
    }
}
