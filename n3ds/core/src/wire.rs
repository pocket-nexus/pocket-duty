//! The guest's end of the interface's channel for PocketJS's C host (the
//! 3DS's `qjs.c`): it carries `ui.svcOpen`, `ui.svcPoll` and `ui.svcSend` to
//! the functions of `svcwire.h`, which its own build implements as a network
//! client. This program draws its own scene and links these instead: the
//! lines stay in the process.
use alloc::string::String;
use core::ffi::{c_char, CStr};

use duty_interface::channel;

/// A line the host had no room for when it asked: it is handed over at the next call.
static mut HELD: Option<String> = None;

/// # Safety
/// `app` is a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn svcwire_open(app: *const c_char) -> bool {
    CStr::from_ptr(app).to_str().is_ok_and(|service| channel().open(service))
}

#[no_mangle]
pub extern "C" fn svcwire_pump() {}

/// # Safety
/// `out` has room for `capacity` bytes. Called from the frame's thread only.
#[no_mangle]
pub unsafe extern "C" fn svcwire_recv_lines(out: *mut u8, capacity: usize) -> usize {
    let held = &mut *core::ptr::addr_of_mut!(HELD);
    let Some(line) = held.take().or_else(|| channel().poll()) else { return 0 };
    if line.len() > capacity {
        *held = Some(line);
        return 0;
    }
    core::ptr::copy_nonoverlapping(line.as_ptr(), out, line.len());
    line.len()
}

/// # Safety
/// `line` points at `length` bytes.
#[no_mangle]
pub unsafe extern "C" fn svcwire_send_line(line: *const u8, length: usize) {
    if let Ok(line) = core::str::from_utf8(core::slice::from_raw_parts(line, length)) {
        channel().receive(line);
    }
}

#[no_mangle]
pub extern "C" fn svcwire_reset() {}

#[no_mangle]
pub extern "C" fn svcwire_shutdown() {
    unsafe { channel().close() }
}
