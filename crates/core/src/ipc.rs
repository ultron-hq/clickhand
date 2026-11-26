//! Single instance + tiny line protocol, ported from ultron/ipc.py.
//!
//! `ultron --prompt` tells the running Ultron to open the bar by sending one
//! newline-terminated line. A lock file guarantees one Ultron at a time: the
//! socket alone can't, because it closes a moment before the process exits.
//!
//! On Linux the socket and lock live at the same paths as the Python original
//! ($XDG_RUNTIME_DIR/ultron.sock|.lock), so either version answers the other's
//! CLI and they never run together (ADR-004/005). Windows uses a named pipe.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use fs4::fs_std::FileExt;
use interprocess::local_socket::{prelude::*, GenericFilePath, GenericNamespaced, Name, Stream};

/// One message on the wire. Text form matches the Python protocol exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Ping,
    Prompt,
    Cursor,
    Voice,
    Quit,
    Say(String),
}

impl Message {
    pub fn to_line(&self) -> String {
        match self {
            Message::Ping => "ping".into(),
            Message::Prompt => "prompt".into(),
            Message::Cursor => "cursor".into(),
            Message::Voice => "voice".into(),
            Message::Quit => "quit".into(),
            Message::Say(t) => format!("say {t}"),
        }
    }

    pub fn parse(line: &str) -> Option<Message> {
        let line = line.trim_end_matches(['\r', '\n']);
        Some(match line {
            "ping" => Message::Ping,
            "prompt" => Message::Prompt,
            "cursor" => Message::Cursor,
            "voice" => Message::Voice,
            "quit" => Message::Quit,
            _ => Message::Say(line.strip_prefix("say ")?.to_string()),
        })
    }
}

/// $XDG_RUNTIME_DIR, else the temp dir (as tempfile.gettempdir() in the original).
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

pub fn lock_path() -> PathBuf {
    runtime_dir().join("ultron.lock")
}

fn socket_name() -> std::io::Result<Name<'static>> {
    if cfg!(windows) {
        "ultron.sock".to_ns_name::<GenericNamespaced>()
    } else {
        runtime_dir()
            .join("ultron.sock")
            .to_fs_name::<GenericFilePath>()
            .map(|n| n.into_owned())
    }
}

/// True if a running Ultron got the message.
pub fn send(msg: &Message) -> bool {
    let Ok(name) = socket_name() else { return false };
    let Ok(mut stream) = Stream::connect(name) else { return false };
    stream.write_all(format!("{}\n", msg.to_line()).as_bytes()).is_ok()
}

/// Keep trying to reach a running Ultron for `secs` (it may be starting or restarting).
pub fn deliver(msg: &Message, secs: f64) -> bool {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    loop {
        if send(msg) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The single-instance lock, held until dropped (or the process exits).
pub struct InstanceLock(File);

/// Take the lock, waiting up to `wait`. None when another Ultron holds it.
pub fn lock(wait: Duration) -> std::io::Result<Option<InstanceLock>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path())?;
    let deadline = Instant::now() + wait;
    loop {
        if file.try_lock_exclusive()? {
            return Ok(Some(InstanceLock(file)));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_round_trip_like_the_python_protocol() {
        for m in [
            Message::Ping,
            Message::Prompt,
            Message::Cursor,
            Message::Voice,
            Message::Quit,
            Message::Say("héllo 👋".into()),
        ] {
            assert_eq!(Message::parse(&format!("{}\n", m.to_line())), Some(m));
        }
        assert_eq!(Message::Say("hi".into()).to_line(), "say hi");
        assert_eq!(Message::parse("nonsense"), None);
    }

    #[test]
    fn second_lock_is_refused_while_the_first_is_held() {
        let dir = std::env::temp_dir().join(format!("ultron-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
        let first = lock(Duration::ZERO).unwrap();
        assert!(first.is_some());
        // a second handle in the same process: flock locks are per open file
        let other = OpenOptions::new().read(true).write(true).open(lock_path()).unwrap();
        assert!(!other.try_lock_exclusive().unwrap());
        drop(first);
        assert!(other.try_lock_exclusive().unwrap());
    }
}
