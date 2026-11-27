//! Screenshots with the cursor ring, and the mapping from screenshot pixels back to
//! the screen. Linux captures X11 directly (no system packages needed); Windows and
//! macOS use xcap. Input coordinates come from enigo, which on a Retina Mac are
//! points, not pixels, so the mapping goes screenshot px → screen px → input coords.
use std::sync::{LazyLock, Mutex};

use image::{imageops, ImageEncoder, Rgba, RgbaImage};

use crate::{input, HandsError};

/// Where the last screenshot came from, so its coordinates can be mapped back.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// screenshot px → screen px
    pub scale: f64,
    /// screen offset of the screenshot's top-left (region shots)
    pub ox: i32,
    pub oy: i32,
    pub width: u32,
    pub height: u32,
}

pub static FRAME: LazyLock<Mutex<Frame>> = LazyLock::new(|| {
    Mutex::new(Frame {
        scale: 1.0,
        ox: 0,
        oy: 0,
        width: 0,
        height: 0,
    })
});

pub fn frame() -> Frame {
    *FRAME.lock().unwrap()
}

/// One X connection for the life of the server, reopened once if it drops. Holding it
/// also stops a bare X server (Xvfb with no other clients) from resetting between calls,
/// which drops anyone connecting mid-reset.
#[cfg(target_os = "linux")]
fn with_x<T>(f: impl Fn(&x11rb::rust_connection::RustConnection, usize) -> Result<T, String>) -> Result<T, HandsError> {
    use x11rb::rust_connection::RustConnection;
    static CONN: LazyLock<Mutex<Option<(RustConnection, usize)>>> = LazyLock::new(|| Mutex::new(None));
    let mut conn = CONN.lock().unwrap_or_else(|e| e.into_inner());
    let mut last = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        if conn.is_none() {
            match x11rb::connect(None) {
                Ok(c) => *conn = Some(c),
                Err(e) => {
                    last = e.to_string();
                    continue;
                }
            }
        }
        let (c, n) = conn.as_ref().expect("connected above");
        match f(c, *n) {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = e;
                *conn = None;
            }
        }
    }
    Err(HandsError::new(format!(
        "Screen capture failed ({last}). The screen may be locked or asleep."
    )))
}

#[cfg(target_os = "linux")]
fn capture() -> Result<RgbaImage, HandsError> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};
    let (w, h, depth, data, bpp) = with_x(|conn, n| {
        let root = &conn.setup().roots[n];
        let (w, h) = (root.width_in_pixels, root.height_in_pixels);
        let reply = conn
            .get_image(ImageFormat::Z_PIXMAP, root.root, 0, 0, w, h, !0)
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?;
        let bpp = conn
            .setup()
            .pixmap_formats
            .iter()
            .find(|f| f.depth == reply.depth)
            .map(|f| f.bits_per_pixel)
            .unwrap_or(32);
        Ok((w, h, reply.depth, reply.data, bpp))
    })?;
    if bpp != 32 {
        return Err(HandsError::new(format!(
            "Screen capture failed (unsupported {bpp}-bit, depth {depth} display)."
        )));
    }
    // 32-bit ZPixmap, least significant byte first: B G R X
    let mut img = RgbaImage::new(w as u32, h as u32);
    for (px, chunk) in img.pixels_mut().zip(data.chunks_exact(4)) {
        *px = Rgba([chunk[2], chunk[1], chunk[0], 255]);
    }
    Ok(img)
}

#[cfg(not(target_os = "linux"))]
fn capture() -> Result<RgbaImage, HandsError> {
    let fail = |e: &dyn std::fmt::Display| HandsError::new(format!("Screen capture failed ({e}). The screen may be locked or asleep."));
    let monitors = xcap::Monitor::all().map_err(|e| fail(&e))?;
    let monitor = monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .ok_or_else(|| fail(&"no primary monitor"))?;
    let shot = monitor.capture_image().map_err(|e| fail(&e))?;
    let (w, h) = (shot.width(), shot.height());
    RgbaImage::from_raw(w, h, shot.into_raw()).ok_or_else(|| fail(&"unreadable capture"))
}

/// The screen size in pixels.
pub fn screen_size() -> Result<(u32, u32), HandsError> {
    #[cfg(target_os = "linux")]
    {
        use x11rb::connection::Connection;
        with_x(|conn, n| {
            let root = &conn.setup().roots[n];
            Ok((root.width_in_pixels as u32, root.height_in_pixels as u32))
        })
        .map_err(|e| HandsError::new(e.0.replace("Screen capture failed", "No display")))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let img = capture()?;
        Ok((img.width(), img.height()))
    }
}

/// Input coordinates per screen pixel (0.5 on a Retina Mac, 1 elsewhere).
fn input_per_pixel(screen_w: u32) -> f64 {
    match input::display() {
        Ok((w, _)) if w > 0 && screen_w > 0 => w as f64 / screen_w as f64,
        _ => 1.0,
    }
}

/// Draw the red cursor ring, like the original (a 22 px circle, 3 px wide).
fn ring(img: &mut RgbaImage, cx: i32, cy: i32) {
    if cx < 0 || cy < 0 || cx >= img.width() as i32 || cy >= img.height() as i32 {
        return;
    }
    for y in (cy - 12).max(0)..=(cy + 12).min(img.height() as i32 - 1) {
        for x in (cx - 12).max(0)..=(cx + 12).min(img.width() as i32 - 1) {
            let d = (((x - cx).pow(2) + (y - cy).pow(2)) as f64).sqrt();
            if (8.0..=11.0).contains(&d) {
                img.put_pixel(x as u32, y as u32, Rgba([255, 40, 40, 255]));
            }
        }
    }
}

/// Take a screenshot (optionally of a region, in screen pixels) no wider than `max_width`,
/// with the cursor ring drawn. Returns PNG bytes and remembers the frame for mapping.
pub fn shot(max_width: u32, region: Option<(u32, u32, u32, u32)>) -> Result<Vec<u8>, HandsError> {
    let full = capture()?;
    let per = input_per_pixel(full.width());
    let (mx, my) = input::location()
        .map(|(x, y)| ((x as f64 / per) as i32, (y as f64 / per) as i32))
        .unwrap_or((-1, -1));
    let (mut img, ox, oy) = match region {
        Some((x0, y0, x1, y1)) => (imageops::crop_imm(&full, x0, y0, x1 - x0, y1 - y0).to_image(), x0 as i32, y0 as i32),
        None => (full, 0, 0),
    };
    let scale = (max_width as f64 / img.width() as f64).min(1.0);
    if scale < 1.0 {
        let (w, h) = (
            (img.width() as f64 * scale).round() as u32,
            (img.height() as f64 * scale).round() as u32,
        );
        img = imageops::resize(&img, w, h, imageops::FilterType::Lanczos3);
    }
    ring(
        &mut img,
        ((mx - ox) as f64 * scale).round() as i32,
        ((my - oy) as f64 * scale).round() as i32,
    );
    *FRAME.lock().unwrap() = Frame {
        scale: 1.0 / scale,
        ox,
        oy,
        width: img.width(),
        height: img.height(),
    };
    let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| HandsError::new(format!("Could not encode the screenshot: {e}")))?;
    Ok(png)
}

/// Map a point in the last screenshot to input coordinates, refusing points off screen.
pub fn to_screen(x: i32, y: i32) -> Result<(i32, i32), HandsError> {
    let f = frame();
    let (sw, sh) = screen_size()?;
    let px = (x as f64 * f.scale).round() as i32 + f.ox;
    let py = (y as f64 * f.scale).round() as i32 + f.oy;
    if px < 0 || py < 0 || px >= sw as i32 || py >= sh as i32 {
        return Err(HandsError::new(format!(
            "({x},{y}) is outside the screen (last screenshot is {}×{}). Take a fresh screenshot and use its coordinates.",
            f.width, f.height
        )));
    }
    let per = input_per_pixel(sw);
    Ok(((px as f64 * per).round() as i32, (py as f64 * per).round() as i32))
}
