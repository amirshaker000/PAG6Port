//! The mouse as the pad's right stick (from XCL's `kbm.rs`, `crates/xcl/src/hle/`):
//! raw input counts go in through the hooked window procedure, and the
//! smoothed velocity becomes a stick deflection each frame, winning over the
//! pad's own stick while it moves. The cursor is held at the window's centre
//! while the title has the focus (`Alt+Tab` frees it). Off unless
//! `VICTORY_MOUSE=1`.
//!
//! | PC | pad |
//! |---|---|
//! | mouse movement | right stick |
//! | left button | right trigger |
//! | right button | left trigger |
//! | middle button | right-stick click |

#![allow(non_snake_case)]

use std::sync::atomic::{AtomicIsize, AtomicI32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

pub(super) const FULL_SPEED: f32 = 4000.0;
/// Slower than this (counts a second) the stick rests: sensor noise.
const REST_SPEED: f32 = 20.0;
/// Deflection the slowest movement starts at (past the inner dead zone).
const START: f32 = 0.15;
/// Time constant of the speed's smoothing (s): the mouse reports in bursts.
const SMOOTHING: f32 = 0.03;

/// The deflection for a mouse velocity (counts a second, screen axes).
pub(super) fn stick_from_mouse(vx: f32, vy: f32) -> (f32, f32) {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed < REST_SPEED {
        return (0.0, 0.0);
    }
    let m = (speed / FULL_SPEED).min(1.0);
    let out = START + (1.0 - START) * m;
    (vx / speed * out, -vy / speed * out)
}

/// The mouse's velocity, smoothed: counts since the last update over `dt` seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Velocity {
    pub x: f32,
    pub y: f32,
}

impl Velocity {
    pub fn update(&mut self, dx: i32, dy: i32, dt: f32) {
        let dt = dt.clamp(0.001, 0.1);
        let a = 1.0 - (-dt / SMOOTHING).exp();
        self.x += a * (dx as f32 / dt - self.x);
        self.y += a * (dy as f32 / dt - self.y);
    }
}

static WINDOW: AtomicIsize = AtomicIsize::new(0);
static DX: AtomicI32 = AtomicI32::new(0);
static DY: AtomicI32 = AtomicI32::new(0);
static CAPTURED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static REGISTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static STATE: Mutex<Option<Velocity>> = Mutex::new(None);

const WM_INPUT: u32 = 0x00FF;
const WM_ACTIVATE: u32 = 0x0021;
const RID_INPUT: u32 = 0x10000005;
const RIM_TYPEMOUSE: u32 = 0;
const MOUSE_MOVE_ABSOLUTE: u32 = 0x0001;

#[repr(C)]
struct RawInputDevice {
    usage_page: u16,
    usage: u16,
    flags: u32,
    target: *mut core::ffi::c_void,
}

#[repr(C)]
#[derive(Default)]
struct RawMouseInput {
    /// RAWINPUTHEADER: (DWORD dwType, DWORD dwSize, HANDLE hDevice,
    /// WPARAM wParam) - `dwType` is the FIRST dword (0: mouse).
    header: [u32; 8],
    /// RAWMOUSE: usFlags.
    flags: u32,
    /// usButtonFlags, usButtonData.
    button_flags: u16,
    button_data: u16,
    raw_buttons: u32,
    /// lLastX, lLastY.
    last_x: i32,
    last_y: i32,
    extra: [u32; 5],
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterRawInputDevices(devices: *const RawInputDevice, n: u32, size: u32) -> i32;
    fn GetRawInputData(input: isize, command: u32, data: *mut core::ffi::c_void, size: *mut u32, header: u32) -> u32;
    fn ClipCursor(rect: *const Rect) -> i32;
    fn GetClientRect(hwnd: *mut core::ffi::c_void, rect: *mut Rect) -> i32;
    fn ClientToScreen(hwnd: *mut core::ffi::c_void, point: *mut Point) -> i32;
    fn GetForegroundWindow() -> *mut core::ffi::c_void;
}

pub(crate) fn enabled() -> bool {
    std::env::var_os("VICTORY_MOUSE").is_some_and(|v| v != "0")
}

/// The title's window is up: raw mouse input reports to it.
pub(crate) fn attach(hwnd: *mut core::ffi::c_void) {
    if !enabled() {
        return;
    }
    WINDOW.store(hwnd as isize, Ordering::Release);
    // Generic desktop mouse.
    let dev = RawInputDevice { usage_page: 1, usage: 2, flags: 0, target: hwnd };
    let ok = unsafe { RegisterRawInputDevices(&dev, 1, std::mem::size_of::<RawInputDevice>() as u32) } != 0;
    REGISTERED.store(ok, Ordering::Release);
    tracing::info!(
        "the mouse is the pad's right stick (left button right trigger, right button left trigger){}",
        if ok { "" } else { " - raw input was refused: the mouse is not read" }
    );
}

/// The cursor is held at the window's centre while `on`, freed otherwise.
fn hold_cursor(hwnd: *mut core::ffi::c_void, on: bool) {
    if !on {
        if CAPTURED.swap(false, Ordering::AcqRel) {
            unsafe { ClipCursor(std::ptr::null()) };
        }
        return;
    }
    // A point at the client area's centre: the cursor stays there, hidden.
    let mut r = Rect { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(hwnd, &mut r) };
    let mut c = Point { x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 };
    unsafe { ClientToScreen(hwnd, &mut c) };
    let at = Rect { left: c.x, top: c.y, right: c.x + 1, bottom: c.y + 1 };
    unsafe { ClipCursor(&at) };
    CAPTURED.store(true, Ordering::Release);
}

/// The window procedure's messages: `true` when handled (the caller returns 0).
pub(crate) fn on_message(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> bool {
    if WINDOW.load(Ordering::Acquire) != hwnd || !REGISTERED.load(Ordering::Acquire) {
        return false;
    }
    match message {
        WM_INPUT => {
            let mut raw = RawMouseInput::default();
            let mut size = std::mem::size_of::<RawMouseInput>() as u32;
            // SAFETY: `lparam` is a HRAWINPUT of this thread's message, and
            // the buffer holds the header the size asked for.
            let n = unsafe {
                GetRawInputData(lparam, RID_INPUT, &mut raw as *mut _ as *mut core::ffi::c_void, &mut size, 24)
            };
            if n != u32::MAX
                && raw.header[0] == RIM_TYPEMOUSE
                && CAPTURED.load(Ordering::Acquire)
                && raw.flags & MOUSE_MOVE_ABSOLUTE == 0
            {
                DX.fetch_add(raw.last_x, Ordering::Relaxed);
                DY.fetch_add(raw.last_y, Ordering::Relaxed);
            }
            false
        }
        WM_ACTIVATE => {
            let active = wparam & 0xFFFF != 0;
            hold_cursor(hwnd as *mut core::ffi::c_void, active);
            if !active {
                DX.store(0, Ordering::Relaxed);
                DY.store(0, Ordering::Relaxed);
            }
            false
        }
        _ => false,
    }
}

/// The mouse's right stick for this frame, on top of `pad`'s own while it
/// moves: the smoothed velocity of the counts since the last call. The
/// buttons are set on `pad` too. Nothing while the window lacks focus or
/// `VICTORY_MOUSE` is unset.
pub(crate) fn apply(pad: &mut super::gameinput::PadState) {
    if !enabled() {
        return;
    }
    let hwnd = WINDOW.load(Ordering::Acquire);
    if hwnd == 0 || unsafe { GetForegroundWindow() } as isize != hwnd {
        return;
    }
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut velocity = g.take().unwrap_or_default();
    let now = Instant::now();
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let dt = last.map(|l| now.duration_since(l).as_secs_f32()).unwrap_or(0.016);
    *last = Some(now);
    velocity.update(DX.swap(0, Ordering::Relaxed), DY.swap(0, Ordering::Relaxed), dt);
    *g = Some(velocity);
    let (rx, ry) = stick_from_mouse(velocity.x, velocity.y);
    if (rx != 0.0 || ry != 0.0) && rx * rx + ry * ry >= pad.right_x * pad.right_x + pad.right_y * pad.right_y {
        pad.right_x = rx;
        pad.right_y = ry;
    }
    // SAFETY: plain queries of button state.
    if unsafe { GetAsyncKeyState(0x01) } < 0 {
        pad.right_trigger = 1.0;
    }
    if unsafe { GetAsyncKeyState(0x02) } < 0 {
        pad.left_trigger = 1.0;
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(key: i32) -> i16;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_still_mouse_rests_the_stick() {
        assert_eq!(stick_from_mouse(0.0, 0.0), (0.0, 0.0));
        assert_eq!(stick_from_mouse(10.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn a_fast_mouse_gives_full_deflection() {
        let (x, y) = stick_from_mouse(4000.0, 0.0);
        assert!((x - 1.0).abs() < 1e-4, "{x}");
        assert_eq!(y, 0.0);
    }

    #[test]
    fn screen_y_is_flipped() {
        let (x, y) = stick_from_mouse(0.0, -1000.0);
        assert_eq!(x, 0.0);
        assert!(y > 0.0, "up on screen is stick up: {y}");
    }
}
