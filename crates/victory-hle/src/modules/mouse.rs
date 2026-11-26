//! The mouse as the pad's right stick (from XCL's `kbm.rs`, `crates/xcl/src/hle/`):
//! each frame reads the cursor, takes its offset from the window's centre as
//! the movement of that frame, and puts the cursor back at the centre. The
//! smoothed velocity becomes a stick deflection, winning over the pad's own
//! stick while it moves. No hooks and no raw input: the cursor can only
//! leave the centre when the title lacks the focus, which is exactly when
//! the mouse must do nothing. Off unless `VICTORY_MOUSE=1`.
//!
//! | PC | pad |
//! |---|---|
//! | mouse movement | right stick |
//! | left button | right trigger |
//! | right button | left trigger |
//! | middle button | right-stick click |

#![allow(non_snake_case)]

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

pub(super) const FULL_SPEED: f32 = 1800.0;
/// Slower than this (counts a second) the stick rests: sensor noise.
const REST_SPEED: f32 = 8.0;
/// Deflection the slowest movement starts at (past the inner dead zone).
const START: f32 = 0.3;
/// Time constant of the speed's smoothing (s): the mouse reports in bursts.
const SMOOTHING: f32 = 0.012;
/// The velocity is sampled at most this often (ms): the pad is polled many
/// times a frame, and each poll sees the cursor already back at the centre -
/// sampling every poll would decay the stick to rest within one frame.
const SAMPLE_MIN_MS: u64 = 8;

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
    /// Counts seen too soon after a sample, folded into the next one.
    pub pending_x: i32,
    pub pending_y: i32,
}

impl Velocity {
    pub fn update(&mut self, dx: i32, dy: i32, dt: f32) {
        let dt = dt.clamp(0.001, 0.1);
        let (dx, dy) = (dx + self.pending_x, dy + self.pending_y);
        self.pending_x = 0;
        self.pending_y = 0;
        let a = 1.0 - (-dt / SMOOTHING).exp();
        self.x += a * (dx as f32 / dt - self.x);
        self.y += a * (dy as f32 / dt - self.y);
    }
}

static WINDOW: AtomicIsize = AtomicIsize::new(0);
static STATE: Mutex<Option<Velocity>> = Mutex::new(None);
static LAST: Mutex<Option<Instant>> = Mutex::new(None);
static FEEDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetClientRect(hwnd: *mut core::ffi::c_void, rect: *mut Rect) -> i32;
    fn ClientToScreen(hwnd: *mut core::ffi::c_void, point: *mut Point) -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn GetForegroundWindow() -> *mut core::ffi::c_void;
}

pub(crate) fn enabled() -> bool {
    std::env::var_os("VICTORY_MOUSE").is_some_and(|v| v != "0")
}

/// The title's window is up: the mouse reports to it.
pub(crate) fn attach(hwnd: *mut core::ffi::c_void) {
    if !enabled() {
        return;
    }
    WINDOW.store(hwnd as isize, Ordering::Release);
    tracing::info!("the mouse is the pad's right stick (left button right trigger, right button left trigger)");
}

/// The window procedure's messages: nothing needed any more, the stick is
/// polled each frame; kept so the wiring holds.
pub(crate) fn on_message(_hwnd: isize, _message: u32, _wparam: usize, _lparam: isize) -> bool {
    false
}

/// The mouse's right stick for this frame, on top of `pad`'s own while it
/// moves: the cursor's offset from the window's centre, put back at the
/// centre. The buttons are set on `pad` too. Nothing while the window lacks
/// focus or `VICTORY_MOUSE` is unset.
pub(crate) fn apply(pad: &mut super::gameinput::PadState) {
    if !enabled() {
        return;
    }
    let hwnd = WINDOW.load(Ordering::Acquire);
    if hwnd == 0 || unsafe { GetForegroundWindow() } as isize != hwnd {
        return;
    }
    let hwnd = hwnd as *mut core::ffi::c_void;
    let mut r = Rect::default();
    if unsafe { GetClientRect(hwnd, &mut r) } == 0 {
        return;
    }
    let mut c = Point { x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 };
    unsafe { ClientToScreen(hwnd, &mut c) };
    let mut p = Point::default();
    if unsafe { GetCursorPos(&mut p) } == 0 {
        return;
    }
    let dx = p.x - c.x;
    let dy = p.y - c.y;
    // The cursor goes back to the centre whether it moved or not: a move of
    // zero keeps it pinned there.
    unsafe { SetCursorPos(c.x, c.y) };
    // The stick is held between samples: the pad's polls within one frame
    // must all see the same deflection, not a re-decayed one.
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let mut velocity = g.take().unwrap_or_default();
    let now = Instant::now();
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    // First poll (or a pause): sample now with a nominal dt, or nothing
    // would ever unblock the gate and the counts would pile up unsampled.
    let since = last.map(|l| now.duration_since(l)).unwrap_or_default();
    let sampled = if last.is_none() || since.as_millis() as u64 >= SAMPLE_MIN_MS {
        velocity.update(dx, dy, since.as_secs_f32().max(if last.is_none() { 0.008 } else { 0.004 }));
        *last = Some(now);
        true
    } else {
        // Too soon: keep the counts for the next sample instead.
        velocity.pending_x += dx;
        velocity.pending_y += dy;
        false
    };
    *g = Some(velocity);
    if sampled && !FEEDING.swap(dx != 0 || dy != 0, Ordering::Relaxed) && (dx != 0 || dy != 0) {
        tracing::info!("the mouse feeds the stick now (dx={dx}, dy={dy})");
    }
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
        assert_eq!(stick_from_mouse(4.0, 0.0), (0.0, 0.0));
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

    #[test]
    fn the_velocity_decays_to_rest() {
        let mut v = Velocity::default();
        v.update(400, 0, 0.016);
        let fast = v.x;
        for _ in 0..60 {
            v.update(0, 0, 0.016);
        }
        assert!(v.x < fast * 0.01, "{} -> {}", fast, v.x);
    }

    #[test]
    fn counts_seen_between_samples_are_kept() {
        let mut v = Velocity::default();
        v.pending_x = 90;
        v.update(10, 0, 0.008);
        assert_eq!(v.pending_x, 0);
        assert!(v.x > 5000.0, "{}", v.x);
    }
}
