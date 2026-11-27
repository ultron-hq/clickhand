//! Mouse and keyboard through enigo (Windows, macOS, X11), with the original's key names.
use std::thread::sleep;
use std::time::Duration;

use enigo::{Axis, Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use crate::HandsError;

fn enigo() -> Result<Enigo, HandsError> {
    Enigo::new(&Settings::default()).map_err(|e| HandsError::new(format!("Input is unavailable ({e}). The screen may be locked.")))
}

fn fail(e: impl std::fmt::Display) -> HandsError {
    HandsError::new(format!("Input failed ({e}). The screen may be locked or the window gone."))
}

pub fn display() -> Result<(i32, i32), HandsError> {
    enigo()?.main_display().map_err(fail)
}

pub fn location() -> Result<(i32, i32), HandsError> {
    enigo()?.location().map_err(fail)
}

/// On macOS, synthetic "mouse moved" events don't always move the cursor (seen in macOS
/// VMs), and clicks are posted wherever the cursor is. Warp it there first.
#[cfg(target_os = "macos")]
fn warp(x: i32, y: i32) {
    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWarpMouseCursorPosition(p: CGPoint) -> i32;
        fn CGAssociateMouseAndMouseCursorPosition(connected: u32) -> i32;
    }
    unsafe {
        CGWarpMouseCursorPosition(CGPoint { x: x as f64, y: y as f64 });
        CGAssociateMouseAndMouseCursorPosition(1);
    }
}

/// Move the pointer and confirm it got there, so a click can never land somewhere else.
fn go(e: &mut Enigo, x: i32, y: i32) -> Result<(), HandsError> {
    #[cfg(target_os = "macos")]
    warp(x, y);
    e.move_mouse(x, y, Coordinate::Abs).map_err(fail)?;
    let mut at = (x, y);
    for _ in 0..25 {
        match e.location() {
            Ok(p) if (p.0 - x).abs() <= 2 && (p.1 - y).abs() <= 2 => return Ok(()),
            Ok(p) => at = p,
            Err(_) => return Ok(()), // can't read the pointer here; trust the move
        }
        sleep(Duration::from_millis(8));
    }
    Err(HandsError::new(format!(
        "The pointer didn't reach ({x},{y}) (it is at ({},{})), so nothing was clicked. Take a fresh screenshot and try again.",
        at.0, at.1
    )))
}

pub fn move_to(x: i32, y: i32) -> Result<(), HandsError> {
    go(&mut enigo()?, x, y)
}

pub fn button(name: &str) -> Result<Button, HandsError> {
    Ok(match name {
        "left" => Button::Left,
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => return Err(HandsError::new("button must be left/right/middle and count 1-3.")),
    })
}

pub fn click(x: i32, y: i32, b: Button, count: u32) -> Result<(), HandsError> {
    let mut e = enigo()?;
    go(&mut e, x, y)?;
    for i in 0..count {
        if i > 0 {
            sleep(Duration::from_millis(90));
        }
        e.button(b, Direction::Click).map_err(fail)?;
    }
    Ok(())
}

/// Press at the start, glide in 8 steps, and always release.
pub fn drag(from: (i32, i32), to: (i32, i32)) -> Result<(), HandsError> {
    let mut e = enigo()?;
    go(&mut e, from.0, from.1)?;
    e.button(Button::Left, Direction::Press).map_err(fail)?;
    let mut result = Ok(());
    for i in 1..=8 {
        let (x, y) = (from.0 + (to.0 - from.0) * i / 8, from.1 + (to.1 - from.1) * i / 8);
        #[cfg(target_os = "macos")]
        warp(x, y);
        if let Err(err) = e.move_mouse(x, y, Coordinate::Abs) {
            result = Err(fail(err));
            break;
        }
        sleep(Duration::from_millis(20));
    }
    let _ = e.button(Button::Left, Direction::Release);
    result
}

pub fn scroll(direction: &str, amount: i32) -> Result<(), HandsError> {
    let (axis, sign) = match direction {
        "up" => (Axis::Vertical, -1),
        "down" => (Axis::Vertical, 1),
        "left" => (Axis::Horizontal, -1),
        "right" => (Axis::Horizontal, 1),
        _ => return Err(HandsError::new("direction must be up/down/left/right and amount 1-15.")),
    };
    let mut e = enigo()?;
    for _ in 0..amount {
        e.scroll(sign, axis).map_err(fail)?;
        sleep(Duration::from_millis(40));
    }
    Ok(())
}

/// One key name → (the original's xdotool name, for messages; the enigo key).
fn key_name(p: &str) -> Option<(String, Key)> {
    let named = |n: &str, k: Key| Some((n.to_string(), k));
    match p {
        "ctrl" | "control" => named("ctrl", Key::Control),
        "alt" | "option" => named("alt", Key::Alt),
        "shift" => named("shift", Key::Shift),
        "super" | "win" | "windows" | "cmd" | "meta" => named("super", Key::Meta),
        "enter" | "return" => named("Return", Key::Return),
        "esc" | "escape" => named("Escape", Key::Escape),
        "tab" => named("Tab", Key::Tab),
        "space" | "spacebar" => named("space", Key::Space),
        "backspace" => named("BackSpace", Key::Backspace),
        "delete" | "del" => named("Delete", Key::Delete),
        "up" => named("Up", Key::UpArrow),
        "down" => named("Down", Key::DownArrow),
        "left" => named("Left", Key::LeftArrow),
        "right" => named("Right", Key::RightArrow),
        "home" => named("Home", Key::Home),
        "end" => named("End", Key::End),
        "pageup" | "pgup" => named("Prior", Key::PageUp),
        "pagedown" | "pgdn" => named("Next", Key::PageDown),
        #[cfg(not(target_os = "macos"))]
        "insert" => named("Insert", Key::Insert),
        #[cfg(not(target_os = "macos"))]
        "printscreen" => named("Print", Key::PrintScr),
        #[cfg(not(target_os = "macos"))]
        "capslock" => named("Caps_Lock", Key::CapsLock),
        "plus" => named("plus", Key::Unicode('+')),
        "minus" => named("minus", Key::Unicode('-')),
        "comma" => named("comma", Key::Unicode(',')),
        "period" => named("period", Key::Unicode('.')),
        "slash" => named("slash", Key::Unicode('/')),
        _ => {
            let f = p
                .strip_prefix('f')
                .and_then(|n| n.parse::<u8>().ok())
                .filter(|n| (1..=12).contains(n));
            if let Some(n) = f {
                let keys = [
                    Key::F1,
                    Key::F2,
                    Key::F3,
                    Key::F4,
                    Key::F5,
                    Key::F6,
                    Key::F7,
                    Key::F8,
                    Key::F9,
                    Key::F10,
                    Key::F11,
                    Key::F12,
                ];
                return named(&format!("F{n}"), keys[n as usize - 1]);
            }
            let mut chars = p.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => named(&c.to_string(), Key::Unicode(c)),
                _ => None,
            }
        }
    }
}

/// 'Ctrl+Shift+T' / 'alt f4' / 'enter' → the chord's display name and keys.
pub fn parse_keys(spec: &str) -> Result<(String, Vec<Key>), HandsError> {
    let s = spec
        .trim()
        .to_lowercase()
        .replace("page up", "pageup")
        .replace("page down", "pagedown");
    let parts: Vec<&str> = s.split(|c: char| c == '+' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Err(HandsError::new("Empty key spec."));
    }
    let mut names = Vec::new();
    let mut keys = Vec::new();
    for p in parts {
        let (n, k) = key_name(p).ok_or_else(|| {
            HandsError::new(format!(
                "Unknown key '{p}'. Use names like ctrl, shift, alt, super, enter, esc, tab, f5, pageup, a-z, 0-9."
            ))
        })?;
        names.push(n);
        keys.push(k);
    }
    Ok((names.join("+"), keys))
}

/// Hold every key but the last, tap the last, release in reverse.
pub fn chord(keys: &[Key]) -> Result<(), HandsError> {
    let mut e = enigo()?;
    let (last, held) = keys.split_last().ok_or_else(|| HandsError::new("Empty key spec."))?;
    let mut pressed = Vec::new();
    let mut result = Ok(());
    for k in held {
        match e.key(*k, Direction::Press) {
            Ok(()) => pressed.push(*k),
            Err(err) => {
                result = Err(fail(err));
                break;
            }
        }
    }
    if result.is_ok() {
        result = e.key(*last, Direction::Click).map_err(fail);
    }
    for k in pressed.iter().rev() {
        let _ = e.key(*k, Direction::Release);
    }
    result
}

/// Type text; newlines press Enter, tabs become four spaces.
pub fn type_text(text: &str) -> Result<(), HandsError> {
    let mut e = enigo()?;
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            e.key(Key::Return, Direction::Click).map_err(fail)?;
        }
        let line = line.replace('\t', "    ");
        let chars: Vec<char> = line.chars().collect();
        for chunk in chars.chunks(32) {
            e.text(&chunk.iter().collect::<String>()).map_err(fail)?;
            sleep(Duration::from_millis(8));
        }
    }
    Ok(())
}
