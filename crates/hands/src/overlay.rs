//! What the person sees while an agent works: a glowing frame around the screen while an
//! agent is using Hands, a tag beside the pointer whenever it moves the mouse or types,
//! and captioned boxes for teaching ("click here"). Every overlay is click-through and
//! hidden while a screenshot is taken, so the agent never sees or hits it.
//! Linux (X11) and Windows; on macOS these calls do nothing yet.
//! Set ULTRON_HANDS_OVERLAY=0 to turn it off.
use std::time::Duration;

/// How long the screen frame stays after the agent's last call (it thinks between calls).
pub const FRAME_FOR: Duration = Duration::from_secs(12);
/// How long the pointer tag stays after the last click, move or keystroke.
pub const TAG_FOR: Duration = Duration::from_millis(3000);

pub fn enabled() -> bool {
    std::env::var("ULTRON_HANDS_OVERLAY").as_deref() != Ok("0")
}

#[cfg(windows)]
pub use win::{activity, clear, hidden, highlight};
#[cfg(target_os = "linux")]
pub use x::{activity, clear, hidden, highlight};

#[cfg(not(any(target_os = "linux", windows)))]
pub fn activity(_input: bool) {}
#[cfg(not(any(target_os = "linux", windows)))]
pub fn hidden<T>(f: impl FnOnce() -> T) -> T {
    f()
}
#[cfg(not(any(target_os = "linux", windows)))]
pub fn highlight(_x: i32, _y: i32, _w: i32, _h: i32, _caption: &str, _secs: f64) -> bool {
    false
}
#[cfg(not(any(target_os = "linux", windows)))]
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

/// Windows: topmost, layered, click-through (WS_EX_TRANSPARENT) tool windows on their own
/// thread. They are excluded from screen capture (WDA_EXCLUDEFROMCAPTURE, Windows 10 2004+),
/// so the agent's screenshots never contain them; older Windows hides them instead.
#[cfg(windows)]
mod win {
    use std::sync::{LazyLock, Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::{wrap, FRAME_FOR, TAG_FOR};

    /// COLORREF is 0x00BBGGRR.
    const EMBER: u32 = 0x002f_7aff;
    const INK: u32 = 0x0009_0b16;
    const TEXT: u32 = 0x00e8_f1ff;
    const EDGE: i32 = 4;
    const LINE: i32 = 18;
    const TAG: &str = "Ultron is using your mouse";

    type Rect = (i32, i32, i32, i32);

    #[derive(Default)]
    struct State {
        frame_until: Option<Instant>,
        tag_until: Option<Instant>,
        box_until: Option<Instant>,
        caption: Vec<String>,
        /// frame and caption rectangles (x, y, w, h), placed by the next tick
        boxes: Option<(Rect, Rect)>,
        hidden: bool,
        excluded: bool,
        /// edge ring, tag, frame, caption (HWNDs as isize so the state is Send)
        wins: [isize; 4],
        shown: [bool; 4],
        font: isize,
        tag_w: i32,
    }

    static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));
    static READY: LazyLock<bool> = LazyLock::new(|| {
        if !super::enabled() {
            return false;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || unsafe { run(tx) });
        rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false)
    });

    fn state() -> MutexGuard<'static, State> {
        STATE.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    fn hwnd(v: isize) -> HWND {
        v as HWND
    }

    unsafe fn text_width(font: isize, s: &str) -> i32 {
        let dc = GetDC(std::ptr::null_mut());
        let old = SelectObject(dc, font as HGDIOBJ);
        let w: Vec<u16> = s.encode_utf16().collect();
        let mut size = SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(dc, w.as_ptr(), w.len() as i32, &mut size);
        SelectObject(dc, old);
        ReleaseDC(std::ptr::null_mut(), dc);
        size.cx
    }

    /// A window-shaped ring: the rectangle minus its inside, `t` pixels thick.
    unsafe fn ring(h: HWND, w: i32, hh: i32, t: i32) {
        let outer = CreateRectRgn(0, 0, w, hh);
        let inner = CreateRectRgn(t, t, w - t, hh - t);
        let r = CreateRectRgn(0, 0, 0, 0);
        CombineRgn(r, outer, inner, RGN_DIFF);
        DeleteObject(outer as HGDIOBJ);
        DeleteObject(inner as HGDIOBJ);
        SetWindowRgn(h, r, 1);
    }

    unsafe fn place(h: HWND, x: i32, y: i32, w: i32, hh: i32) {
        SetWindowPos(h, HWND_TOPMOST, x, y, w, hh, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }

    unsafe extern "system" fn proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        match msg {
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            WM_TIMER => {
                tick();
                0
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(h, &mut ps);
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(h, &mut rc);
                let (lines, font, dark) = {
                    let st = state();
                    let v = h as isize;
                    if v == st.wins[1] {
                        (vec![TAG.to_string()], st.font, true)
                    } else if v == st.wins[3] {
                        (st.caption.clone(), st.font, true)
                    } else {
                        (vec![], st.font, false)
                    }
                };
                let brush = CreateSolidBrush(if dark { INK } else { EMBER });
                FillRect(dc, &rc, brush);
                DeleteObject(brush as HGDIOBJ);
                if dark {
                    let edge = CreateSolidBrush(EMBER);
                    FrameRect(dc, &rc, edge);
                    DeleteObject(edge as HGDIOBJ);
                    SelectObject(dc, font as HGDIOBJ);
                    SetBkMode(dc, TRANSPARENT as _);
                    SetTextColor(dc, TEXT);
                    for (i, l) in lines.iter().enumerate() {
                        let w: Vec<u16> = l.encode_utf16().collect();
                        TextOutW(dc, 9, 3 + i as i32 * LINE, w.as_ptr(), w.len() as i32);
                    }
                }
                EndPaint(h, &ps);
                0
            }
            _ => DefWindowProcW(h, msg, wp, lp),
        }
    }

    unsafe fn run(ready: std::sync::mpsc::Sender<bool>) {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = wide("UltronHandsOverlay");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let title = wide("ultron-hands overlay");
        let ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let mut wins = [0isize; 4];
        let mut excluded = true;
        for w in wins.iter_mut() {
            let h = CreateWindowExW(
                ex,
                class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                inst,
                std::ptr::null(),
            );
            if h.is_null() {
                let _ = ready.send(false);
                return;
            }
            SetLayeredWindowAttributes(h, 0, 245, LWA_ALPHA);
            excluded &= SetWindowDisplayAffinity(h, WDA_EXCLUDEFROMCAPTURE) != 0;
            *w = h as isize;
        }
        let face = wide("Segoe UI");
        let font = CreateFontW(
            -15,
            0,
            0,
            0,
            FW_SEMIBOLD as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            face.as_ptr(),
        ) as isize;
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        ring(hwnd(wins[0]), sw, sh, EDGE);
        SetWindowPos(hwnd(wins[0]), HWND_TOPMOST, 0, 0, sw, sh, SWP_NOACTIVATE | SWP_HIDEWINDOW);
        {
            let mut st = state();
            st.wins = wins;
            st.excluded = excluded;
            st.font = font;
            st.tag_w = text_width(font, TAG) + 20;
        }
        SetTimer(hwnd(wins[0]), 1, 33, None);
        let _ = ready.send(true);
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    unsafe fn show(st: &mut State, i: usize, on: bool) {
        let h = hwnd(st.wins[i]);
        if on && !st.shown[i] {
            SetWindowPos(
                h,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        } else if !on && st.shown[i] {
            ShowWindow(h, SW_HIDE);
        }
        st.shown[i] = on;
    }

    unsafe fn tick() {
        let now = Instant::now();
        let mut st = state();
        let live = |t: Option<Instant>| !st.hidden && t.is_some_and(|t| now < t);
        let (frame_on, tag_on, box_on) = (live(st.frame_until), live(st.tag_until), live(st.box_until));
        if let Some(((fx, fy, fw, fh), (cx, cy, cw, ch))) = st.boxes.take() {
            ring(hwnd(st.wins[2]), fw, fh, 3);
            place(hwnd(st.wins[2]), fx, fy, fw, fh);
            place(hwnd(st.wins[3]), cx, cy, cw, ch);
            st.shown[2] = true;
            st.shown[3] = true;
            InvalidateRect(hwnd(st.wins[3]), std::ptr::null(), 1);
        }
        if tag_on {
            let mut p = POINT { x: 0, y: 0 };
            GetCursorPos(&mut p);
            let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            let w = st.tag_w;
            let mut x = p.x + 20;
            let mut y = p.y + 22;
            if x + w > sw - 8 {
                x = p.x - w - 12;
            }
            if y + 26 > sh - 8 {
                y = p.y - 36;
            }
            place(hwnd(st.wins[1]), x, y, w, 24);
            st.shown[1] = true;
        }
        show(&mut st, 0, frame_on || tag_on);
        show(&mut st, 1, tag_on);
        show(&mut st, 2, box_on);
        let has_caption = !st.caption.is_empty();
        show(&mut st, 3, box_on && has_caption);
    }

    pub fn activity(input: bool) {
        if !*READY {
            return;
        }
        let now = Instant::now();
        let mut st = state();
        st.frame_until = Some(now + FRAME_FOR);
        if input {
            st.tag_until = Some(now + TAG_FOR);
        }
    }

    pub fn hidden<T>(f: impl FnOnce() -> T) -> T {
        if !*READY || state().excluded {
            return f();
        }
        {
            let mut st = state();
            st.hidden = true;
            for i in 0..4 {
                unsafe { ShowWindow(hwnd(st.wins[i]), SW_HIDE) };
                st.shown[i] = false;
            }
        }
        std::thread::sleep(Duration::from_millis(60));
        let out = f();
        state().hidden = false;
        out
    }

    pub fn highlight(x: i32, y: i32, w: i32, h: i32, caption: &str, secs: f64) -> bool {
        if !*READY {
            return false;
        }
        let (t, pad) = (3, 4);
        let frame = (x - pad - t, y - pad - t, w + 2 * (pad + t), h + 2 * (pad + t));
        let lines = wrap(caption, 52, 5);
        let mut st = state();
        let font = st.font;
        let cw = lines.iter().map(|l| unsafe { text_width(font, l) }).max().unwrap_or(0) + 20;
        let ch = lines.len() as i32 * LINE + 8;
        let (sw, sh) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        let cx = frame.0.clamp(8, (sw - cw - 8).max(8));
        let cy = if frame.1 + frame.3 + 6 + ch < sh - 8 {
            frame.1 + frame.3 + 6
        } else {
            (frame.1 - ch - 6).max(8)
        };
        st.caption = lines;
        st.boxes = Some((frame, (cx, cy, cw, ch)));
        st.box_until = Some(Instant::now() + Duration::from_secs_f64(secs.clamp(0.0, 120.0)));
        true
    }

    pub fn clear() {
        state().box_until = None;
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
