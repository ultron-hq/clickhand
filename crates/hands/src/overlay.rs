//! What the person sees while an agent works: a glowing frame around the screen while an
//! agent is using Hands, a tag beside the pointer whenever it moves the mouse or types,
//! and captioned boxes for teaching ("click here"). Every overlay is click-through and
//! hidden while a screenshot is taken, so the agent never sees or hits it.
//! Linux (X11) for now; on other systems these calls do nothing yet.
//! Set ULTRON_HANDS_OVERLAY=0 to turn it off.
use std::time::Duration;

/// How long the screen frame stays after the agent's last call (it thinks between calls).
pub const FRAME_FOR: Duration = Duration::from_secs(12);
/// How long the pointer tag stays after the last click, move or keystroke.
pub const TAG_FOR: Duration = Duration::from_millis(3000);

pub fn enabled() -> bool {
    std::env::var("ULTRON_HANDS_OVERLAY").as_deref() != Ok("0")
}

#[cfg(target_os = "linux")]
pub use x::{activity, clear, hidden, highlight};

#[cfg(not(target_os = "linux"))]
pub fn activity(_input: bool) {}
#[cfg(not(target_os = "linux"))]
pub fn hidden<T>(f: impl FnOnce() -> T) -> T {
    f()
}
#[cfg(not(target_os = "linux"))]
pub fn highlight(_x: i32, _y: i32, _w: i32, _h: i32, _caption: &str, _secs: f64) -> bool {
    false
}
#[cfg(not(target_os = "linux"))]
pub fn clear() {}

/// Split a caption into lines of at most `width` characters (ASCII; others become '?').
pub fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let text: String = text
        .chars()
        .map(|c| {
            if c == '\n' || (c.is_ascii() && !c.is_ascii_control()) {
                c
            } else {
                '?'
            }
        })
        .collect();
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let word: String = word.chars().take(width).collect();
            if !line.is_empty() && line.len() + 1 + word.len() > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(&word);
        }
        lines.push(line);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(l) = lines.last_mut() {
            l.truncate(width.saturating_sub(3));
            l.push_str("...");
        }
    }
    lines
}

#[cfg(target_os = "linux")]
mod x {
    use std::error::Error;
    use std::sync::{LazyLock, Mutex};
    use std::time::{Duration, Instant};

    use x11rb::connection::{Connection, RequestConnection};
    use x11rb::protocol::shape::{self, ConnectionExt as _};
    use x11rb::protocol::xproto::*;
    use x11rb::rust_connection::RustConnection;
    use x11rb::wrapper::ConnectionExt as _;

    use super::{wrap, FRAME_FOR, TAG_FOR};

    const EMBER: u32 = 0xff7a2f;
    const INK: u32 = 0x160b09;
    const TEXT: u32 = 0xfff1e8;
    const EDGE: u16 = 4;
    const TAG: &str = "Ultron is using your mouse";
    const LINE: i16 = 16;

    #[derive(Default)]
    struct State {
        frame_until: Option<Instant>,
        tag_until: Option<Instant>,
        box_until: Option<Instant>,
        caption: Vec<String>,
        hidden: bool,
        /// edges, tag, box, caption
        shown: [bool; 4],
        raised: Option<Instant>,
    }

    struct Overlay {
        conn: RustConnection,
        root: Window,
        size: (u16, u16),
        edges: [Window; 4],
        tag: Window,
        frame: Window,
        caption: Window,
        gc: Gcontext,
        char_w: i16,
        state: Mutex<State>,
    }

    static OVERLAY: LazyLock<Option<Overlay>> = LazyLock::new(|| {
        if !super::enabled() {
            return None;
        }
        let o = Overlay::new().ok()?;
        std::thread::spawn(|| loop {
            if let Some(o) = OVERLAY.as_ref() {
                o.tick();
            }
            std::thread::sleep(Duration::from_millis(33));
        });
        Some(o)
    });

    impl Overlay {
        fn new() -> Result<Self, Box<dyn Error>> {
            let (conn, n) = x11rb::connect(None)?;
            let screen = conn.setup().roots[n].clone();
            if screen.root_depth < 24 {
                return Err("needs a true-colour display".into());
            }
            conn.extension_information(shape::X11_EXTENSION_NAME)?.ok_or("no SHAPE extension")?;
            let root = screen.root;
            let (sw, sh) = (screen.width_in_pixels, screen.height_in_pixels);
            let make = |color: u32, border: u16| -> Result<Window, Box<dyn Error>> {
                let w = conn.generate_id()?;
                let aux = CreateWindowAux::new()
                    .background_pixel(color)
                    .border_pixel(EMBER)
                    .override_redirect(1)
                    .save_under(1);
                conn.create_window(
                    x11rb::COPY_DEPTH_FROM_PARENT,
                    w,
                    root,
                    0,
                    0,
                    1,
                    1,
                    border,
                    WindowClass::INPUT_OUTPUT,
                    x11rb::COPY_FROM_PARENT,
                    &aux,
                )?;
                // An empty input shape: clicks go straight through to what's below.
                conn.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, w, 0, 0, &[])?;
                conn.change_property8(PropMode::REPLACE, w, AtomEnum::WM_NAME, AtomEnum::STRING, b"ultron-hands overlay")?;
                Ok(w)
            };
            let edges = [make(EMBER, 0)?, make(EMBER, 0)?, make(EMBER, 0)?, make(EMBER, 0)?];
            let place = [
                (0, 0, sw, EDGE),
                (0, sh - EDGE, sw, EDGE),
                (0, 0, EDGE, sh),
                (sw - EDGE, 0, EDGE, sh),
            ];
            for (w, (x, y, ww, hh)) in edges.iter().zip(place) {
                conn.configure_window(
                    *w,
                    &ConfigureWindowAux::new().x(x as i32).y(y as i32).width(ww as u32).height(hh as u32),
                )?;
            }
            let tag = make(INK, 1)?;
            let frame = make(EMBER, 0)?;
            let caption = make(INK, 1)?;
            let font = conn.generate_id()?;
            let mut char_w = 7;
            if conn
                .open_font(font, b"-misc-fixed-bold-r-normal--13-120-75-75-c-70-iso8859-1")?
                .check()
                .is_err()
            {
                conn.open_font(font, b"fixed")?.check()?;
                char_w = 6;
            }
            let gc = conn.generate_id()?;
            conn.create_gc(gc, root, &CreateGCAux::new().foreground(TEXT).background(INK).font(font))?;
            conn.flush()?;
            Ok(Overlay {
                conn,
                root,
                size: (sw, sh),
                edges,
                tag,
                frame,
                caption,
                gc,
                char_w,
                state: Mutex::new(State::default()),
            })
        }

        fn state(&self) -> std::sync::MutexGuard<'_, State> {
            self.state.lock().unwrap_or_else(|e| e.into_inner())
        }

        fn set(&self, shown: &mut bool, wins: &[Window], on: bool, raise: bool) {
            if on && (!*shown || raise) {
                for w in wins {
                    let _ = self.conn.map_window(*w);
                    let _ = self
                        .conn
                        .configure_window(*w, &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE));
                }
            } else if !on && *shown {
                for w in wins {
                    let _ = self.conn.unmap_window(*w);
                }
            }
            *shown = on;
        }

        fn text(&self, win: Window, lines: &[String]) {
            for (i, l) in lines.iter().enumerate() {
                let _ = self.conn.image_text8(win, self.gc, 8, 15 + i as i16 * LINE, l.as_bytes());
            }
        }

        fn tick(&self) {
            let now = Instant::now();
            let mut st = self.state();
            let live = |t: Option<Instant>| !st.hidden && t.is_some_and(|t| now < t);
            let (frame_on, tag_on, box_on) = (live(st.frame_until), live(st.tag_until), live(st.box_until));
            let raise = st.raised.is_none_or(|t| now - t > Duration::from_millis(500));
            if raise {
                st.raised = Some(now);
            }
            if tag_on {
                if let Ok(Ok(p)) = self.conn.query_pointer(self.root).map(|c| c.reply()) {
                    let w = TAG.len() as i32 * self.char_w as i32 + 16;
                    let (sw, sh) = (self.size.0 as i32, self.size.1 as i32);
                    let mut x = p.root_x as i32 + 20;
                    let mut y = p.root_y as i32 + 22;
                    if x + w > sw - 8 {
                        x = p.root_x as i32 - w - 12;
                    }
                    if y + 24 > sh - 8 {
                        y = p.root_y as i32 - 34;
                    }
                    let _ = self
                        .conn
                        .configure_window(self.tag, &ConfigureWindowAux::new().x(x).y(y).width(w as u32).height(22));
                }
            }
            let mut shown = st.shown;
            self.set(&mut shown[0], &self.edges, frame_on || tag_on, raise);
            self.set(&mut shown[1], &[self.tag], tag_on, raise);
            self.set(&mut shown[2], &[self.frame], box_on, raise);
            self.set(&mut shown[3], &[self.caption], box_on && !st.caption.is_empty(), raise);
            st.shown = shown;
            if tag_on {
                self.text(self.tag, &[TAG.to_string()]);
            }
            if box_on {
                self.text(self.caption, &st.caption);
            }
            let _ = self.conn.flush();
        }

        fn highlight(&self, x: i32, y: i32, w: i32, h: i32, caption: &str, secs: f64) -> Result<(), Box<dyn Error>> {
            let (t, pad) = (3i32, 4i32);
            let (fx, fy, fw, fh) = (x - pad - t, y - pad - t, w + 2 * (pad + t), h + 2 * (pad + t));
            self.conn.configure_window(
                self.frame,
                &ConfigureWindowAux::new()
                    .x(fx)
                    .y(fy)
                    .width(fw.max(1) as u32)
                    .height(fh.max(1) as u32),
            )?;
            let r = |x: i32, y: i32, w: i32, h: i32| Rectangle {
                x: x as i16,
                y: y as i16,
                width: w as u16,
                height: h as u16,
            };
            let ring = [r(0, 0, fw, t), r(0, fh - t, fw, t), r(0, 0, t, fh), r(fw - t, 0, t, fh)];
            self.conn
                .shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, ClipOrdering::UNSORTED, self.frame, 0, 0, &ring)?;
            let lines = wrap(caption, 52, 5);
            if !lines.is_empty() {
                let cw = lines.iter().map(String::len).max().unwrap_or(0) as i32 * self.char_w as i32 + 16;
                let ch = lines.len() as i32 * LINE as i32 + 8;
                let (sw, sh) = (self.size.0 as i32, self.size.1 as i32);
                let cx = fx.clamp(8, (sw - cw - 8).max(8));
                let cy = if fy + fh + 6 + ch < sh - 8 {
                    fy + fh + 6
                } else {
                    (fy - ch - 6).max(8)
                };
                self.conn.configure_window(
                    self.caption,
                    &ConfigureWindowAux::new().x(cx).y(cy).width(cw as u32).height(ch as u32),
                )?;
            }
            let mut st = self.state();
            st.caption = lines;
            st.box_until = Some(Instant::now() + Duration::from_secs_f64(secs.clamp(0.0, 120.0)));
            st.raised = None;
            drop(st);
            self.conn.flush()?;
            Ok(())
        }
    }

    /// The agent made a call: keep the frame up, and the pointer tag too if it acted.
    pub fn activity(input: bool) {
        if let Some(o) = OVERLAY.as_ref() {
            let now = Instant::now();
            let mut st = o.state();
            st.frame_until = Some(now + FRAME_FOR);
            if input {
                st.tag_until = Some(now + TAG_FOR);
            }
        }
    }

    /// Run `f` (a screen capture) with every overlay off the screen.
    pub fn hidden<T>(f: impl FnOnce() -> T) -> T {
        let Some(o) = OVERLAY.as_ref() else { return f() };
        {
            let mut st = o.state();
            st.hidden = true;
            if st.shown.iter().any(|s| *s) {
                for w in o.edges.iter().chain([&o.tag, &o.frame, &o.caption]) {
                    let _ = o.conn.unmap_window(*w);
                }
                st.shown = [false; 4];
                drop(st);
                let _ = o.conn.sync();
                // Give a compositor a frame to repaint without them.
                std::thread::sleep(Duration::from_millis(40));
            }
        }
        let out = f();
        o.state().hidden = false;
        out
    }

    /// Draw a box around a screen rectangle with a caption, for `secs` seconds.
    pub fn highlight(x: i32, y: i32, w: i32, h: i32, caption: &str, secs: f64) -> bool {
        OVERLAY.as_ref().is_some_and(|o| o.highlight(x, y, w, h, caption, secs).is_ok())
    }

    pub fn clear() {
        if let Some(o) = OVERLAY.as_ref() {
            o.state().box_until = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn captions_wrap_and_stay_ascii() {
        assert_eq!(wrap("Click Export → PNG here", 12, 5), vec!["Click Export", "? PNG here"]);
        assert_eq!(wrap("a b c d e f", 3, 2), vec!["a b", "..."]);
        assert_eq!(wrap("one\ntwo", 20, 5), vec!["one", "two"]);
        assert!(wrap("", 20, 5).is_empty());
    }
}
