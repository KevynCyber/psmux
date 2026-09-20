//! Session layout persistence: saves the *shape* of a session (windows,
//! pane splits, each pane's cwd/start_command, sizes) to disk so it can be
//! reloaded after a machine or server restart.
//!
//! psmux is zero-third-party-dep (see Cargo.toml), so this is a hand-rolled
//! serialiser rather than a serde-based one. The on-disk format is plain
//! ASCII with explicit byte-length prefixes on every variable-length field,
//! so parsing never depends on splitting content by newline (a cwd or
//! start_command may itself contain one) and every read is bounds-checked
//! against the remaining buffer rather than trusting a declared count.
//!
//! Deliberately out of scope for this slice (see tests-rs/test_session_persist.rs):
//! floating panes, zoom state, per-window options/hooks/key-tables, and
//! actually spawning a process on restore -- `restore_sessions` here is a
//! pure name-collision gate only.

use std::io;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;

/// Upper bound on any declared count (sessions/windows/children/sizes) or
/// string length while parsing untrusted state-file content. Well above any
/// real layout, so it never rejects legitimate data, but it stops a hostile
/// file from making us loop or allocate absurdly before the bounds-checked
/// byte reads themselves would fail.
const MAX_COUNT: usize = 1_000_000;

/// Maximum nesting depth of a split tree while parsing. A layout is a few
/// levels deep at most; this caps recursion so a hostile file cannot blow
/// the stack by nesting NODE_SPLIT arbitrarily deep.
const MAX_SPLIT_DEPTH: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct PaneSnapshot {
    pub id: usize,
    pub cwd: String,
    pub start_command: String,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitKind {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeSnapshot {
    Leaf(PaneSnapshot),
    Split {
        kind: SplitKind,
        sizes: Vec<u16>,
        children: Vec<NodeSnapshot>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowSnapshot {
    pub id: usize,
    pub name: String,
    pub root: NodeSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub name: String,
    pub windows: Vec<WindowSnapshot>,
}

#[derive(Debug)]
pub enum LoadError {
    UnsupportedSchemaVersion(u32),
    Corrupt(String),
}

#[derive(Debug)]
pub enum RestoreError {
    NameCollision(String),
}

/// Path to the state file for a given data dir + namespace (`-L` value, or
/// `None` for the default namespace). Reuses the same hashed-name scheme as
/// `paths::namespace_instance_file` so an arbitrary namespace string can
/// never collide with, or escape into, another namespace's file, and so this
/// resolves purely from its arguments -- never the process cwd.
pub fn state_file_path(dir: &Path, ns: Option<&str>) -> PathBuf {
    let mut p = crate::paths::namespace_instance_file(dir, ns);
    p.set_extension("state");
    p
}

/// Serialise `sessions` and write them to the namespace's state file under
/// `dir`, creating the parent directory if needed.
pub fn save_state(dir: &Path, ns: Option<&str>, sessions: &[SessionSnapshot]) -> io::Result<()> {
    let path = state_file_path(dir, ns);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut w = Writer::new();
    w.write_kv_u32("SCHEMA_VERSION", SCHEMA_VERSION);
    w.write_kv_usize("SESSION_COUNT", sessions.len());
    for session in sessions {
        write_session(&mut w, session);
    }
    std::fs::write(path, w.into_bytes())
}

/// Load every persisted session from the namespace's state file under `dir`.
/// A missing file means "nothing has ever been saved" and is not an error.
pub fn load_state(dir: &Path, ns: Option<&str>) -> Result<Vec<SessionSnapshot>, LoadError> {
    let path = state_file_path(dir, ns);
    let raw = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(LoadError::Corrupt(format!("could not read state file: {e}"))),
    };

    let mut r = Reader::new(&raw);
    let version = r.read_kv_u32("SCHEMA_VERSION")?;
    if version != SCHEMA_VERSION {
        return Err(LoadError::UnsupportedSchemaVersion(version));
    }
    let count = r.read_kv_usize("SESSION_COUNT")?;
    let mut sessions = Vec::new();
    for _ in 0..count {
        sessions.push(read_session(&mut r)?);
    }
    Ok(sessions)
}

/// Pure name-collision gate for restoring persisted sessions: no ConPTY is
/// spawned here (a later slice's job). Refuses the whole batch with the
/// first colliding name rather than silently skipping or overwriting a live
/// session.
pub fn restore_sessions(
    snapshots: &[SessionSnapshot],
    live_session_names: &[String],
) -> Result<Vec<String>, RestoreError> {
    for snap in snapshots {
        if live_session_names.iter().any(|n| n == &snap.name) {
            return Err(RestoreError::NameCollision(snap.name.clone()));
        }
    }
    Ok(snapshots.iter().map(|s| s.name.clone()).collect())
}

// ---------------------------------------------------------------------
// Hand-rolled (de)serialisation
// ---------------------------------------------------------------------

fn write_session(w: &mut Writer, session: &SessionSnapshot) {
    w.write_kv_str("SESSION_NAME", &session.name);
    w.write_kv_usize("WINDOW_COUNT", session.windows.len());
    for window in &session.windows {
        write_window(w, window);
    }
}

fn read_session(r: &mut Reader) -> Result<SessionSnapshot, LoadError> {
    let name = r.read_kv_str("SESSION_NAME")?;
    let count = r.read_kv_usize("WINDOW_COUNT")?;
    let mut windows = Vec::new();
    for _ in 0..count {
        windows.push(read_window(r)?);
    }
    Ok(SessionSnapshot { name, windows })
}

fn write_window(w: &mut Writer, window: &WindowSnapshot) {
    w.write_kv_usize("WINDOW_ID", window.id);
    w.write_kv_str("WINDOW_NAME", &window.name);
    write_node(w, &window.root);
}

fn read_window(r: &mut Reader) -> Result<WindowSnapshot, LoadError> {
    let id = r.read_kv_usize("WINDOW_ID")?;
    let name = r.read_kv_str("WINDOW_NAME")?;
    let root = read_node(r, 0)?;
    Ok(WindowSnapshot { id, name, root })
}

fn write_node(w: &mut Writer, node: &NodeSnapshot) {
    match node {
        NodeSnapshot::Leaf(pane) => {
            w.write_tag("NODE_LEAF");
            w.write_kv_usize("PANE_ID", pane.id);
            w.write_kv_str("PANE_CWD", &pane.cwd);
            w.write_kv_str("PANE_CMD", &pane.start_command);
            w.write_kv_u32("PANE_ROWS", pane.rows as u32);
            w.write_kv_u32("PANE_COLS", pane.cols as u32);
        }
        NodeSnapshot::Split { kind, sizes, children } => {
            w.write_tag("NODE_SPLIT");
            let kind_str = match kind {
                SplitKind::Horizontal => "H",
                SplitKind::Vertical => "V",
            };
            w.write_kv_str_raw("SPLIT_KIND", kind_str);
            w.write_kv_usize("SPLIT_SIZES_COUNT", sizes.len());
            for size in sizes {
                w.write_kv_u32("SPLIT_SIZE", *size as u32);
            }
            w.write_kv_usize("SPLIT_CHILDREN_COUNT", children.len());
            for child in children {
                write_node(w, child);
            }
        }
    }
}

fn read_node(r: &mut Reader, depth: usize) -> Result<NodeSnapshot, LoadError> {
    if depth > MAX_SPLIT_DEPTH {
        return Err(LoadError::Corrupt("split tree exceeds max depth".to_string()));
    }
    let tag = r.read_tag()?;
    match tag.as_str() {
        "NODE_LEAF" => {
            let id = r.read_kv_usize("PANE_ID")?;
            let cwd = r.read_kv_str("PANE_CWD")?;
            let start_command = r.read_kv_str("PANE_CMD")?;
            let rows = r.read_kv_u32("PANE_ROWS")? as u16;
            let cols = r.read_kv_u32("PANE_COLS")? as u16;
            Ok(NodeSnapshot::Leaf(PaneSnapshot { id, cwd, start_command, rows, cols }))
        }
        "NODE_SPLIT" => {
            let kind_str = r.read_kv_str_raw("SPLIT_KIND")?;
            let kind = match kind_str.as_str() {
                "H" => SplitKind::Horizontal,
                "V" => SplitKind::Vertical,
                other => return Err(LoadError::Corrupt(format!("unknown split kind {other:?}"))),
            };
            let sizes_count = r.read_kv_usize("SPLIT_SIZES_COUNT")?;
            let mut sizes = Vec::new();
            for _ in 0..sizes_count {
                sizes.push(r.read_kv_u32("SPLIT_SIZE")? as u16);
            }
            let children_count = r.read_kv_usize("SPLIT_CHILDREN_COUNT")?;
            let mut children = Vec::new();
            for _ in 0..children_count {
                children.push(read_node(r, depth + 1)?);
            }
            Ok(NodeSnapshot::Split { kind, sizes, children })
        }
        other => Err(LoadError::Corrupt(format!("unknown node tag {other:?}"))),
    }
}

/// Minimal append-only byte writer for the hand-rolled format. Every field
/// is `KEY <value>\n` for scalars, or `KEY_LEN <byte_len>\n<raw bytes>\n` for
/// strings, so a string's exact byte count -- not a newline scan -- decides
/// where its content ends.
struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Writer { buf: Vec::new() }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    fn write_tag(&mut self, tag: &str) {
        self.buf.extend_from_slice(tag.as_bytes());
        self.buf.push(b'\n');
    }

    fn write_kv_u32(&mut self, key: &str, value: u32) {
        self.buf.extend_from_slice(key.as_bytes());
        self.buf.push(b' ');
        self.buf.extend_from_slice(value.to_string().as_bytes());
        self.buf.push(b'\n');
    }

    fn write_kv_usize(&mut self, key: &str, value: usize) {
        self.buf.extend_from_slice(key.as_bytes());
        self.buf.push(b' ');
        self.buf.extend_from_slice(value.to_string().as_bytes());
        self.buf.push(b'\n');
    }

    /// `KEY_LEN <len>\n<raw bytes>\n` -- for content that is itself
    /// untrusted (cwd, command, names): length-prefixed so embedded
    /// newlines can never desync the reader.
    fn write_kv_str(&mut self, key: &str, value: &str) {
        self.write_kv_usize(&format!("{key}_LEN"), value.len());
        self.buf.extend_from_slice(value.as_bytes());
        self.buf.push(b'\n');
    }

    /// Same framing as `write_kv_str` but for short internal enum tags
    /// (e.g. split kind) that are never attacker content -- kept separate
    /// only so call sites read clearly.
    fn write_kv_str_raw(&mut self, key: &str, value: &str) {
        self.write_kv_str(key, value);
    }
}

/// Cursor-based reader mirroring `Writer`'s framing. Every read is
/// bounds-checked against the remaining slice; nothing here can panic or
/// index out of range on a truncated or adversarial buffer.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    /// Reads up to (and consuming) the next `\n`, as a UTF-8 line. Corrupt
    /// on missing newline (truncated file) or invalid UTF-8.
    fn read_line(&mut self) -> Result<String, LoadError> {
        let rest = &self.data[self.pos..];
        let nl = rest
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| LoadError::Corrupt("unexpected end of file".to_string()))?;
        let line = std::str::from_utf8(&rest[..nl])
            .map_err(|_| LoadError::Corrupt("invalid utf-8 in state file".to_string()))?
            .to_string();
        self.pos += nl + 1;
        Ok(line)
    }

    /// Reads exactly `n` raw bytes as UTF-8, bounds-checked against the
    /// remaining buffer, then consumes the trailing `\n` separator.
    fn read_bytes_field(&mut self, n: usize) -> Result<String, LoadError> {
        if n > MAX_COUNT.saturating_mul(64) {
            return Err(LoadError::Corrupt("declared field length too large".to_string()));
        }
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| LoadError::Corrupt("field length overflow".to_string()))?;
        if end >= self.data.len() {
            return Err(LoadError::Corrupt("unexpected end of file".to_string()));
        }
        let s = std::str::from_utf8(&self.data[self.pos..end])
            .map_err(|_| LoadError::Corrupt("invalid utf-8 in state file".to_string()))?
            .to_string();
        if self.data[end] != b'\n' {
            return Err(LoadError::Corrupt("missing field terminator".to_string()));
        }
        self.pos = end + 1;
        Ok(s)
    }

    fn read_tag(&mut self) -> Result<String, LoadError> {
        self.read_line()
    }

    fn split_kv(line: &str, expected_key: &str) -> Result<String, LoadError> {
        let mut parts = line.splitn(2, ' ');
        let key = parts
            .next()
            .ok_or_else(|| LoadError::Corrupt(format!("expected {expected_key}, got empty line")))?;
        if key != expected_key {
            return Err(LoadError::Corrupt(format!("expected {expected_key}, got {key:?}")));
        }
        let value = parts
            .next()
            .ok_or_else(|| LoadError::Corrupt(format!("missing value for {expected_key}")))?;
        Ok(value.to_string())
    }

    fn read_kv_u32(&mut self, key: &str) -> Result<u32, LoadError> {
        let line = self.read_line()?;
        let value = Self::split_kv(&line, key)?;
        value
            .parse::<u32>()
            .map_err(|_| LoadError::Corrupt(format!("invalid u32 for {key}: {value:?}")))
    }

    fn read_kv_usize(&mut self, key: &str) -> Result<usize, LoadError> {
        let line = self.read_line()?;
        let value = Self::split_kv(&line, key)?;
        let parsed = value
            .parse::<usize>()
            .map_err(|_| LoadError::Corrupt(format!("invalid count for {key}: {value:?}")))?;
        if parsed > MAX_COUNT {
            return Err(LoadError::Corrupt(format!("{key} exceeds max allowed count")));
        }
        Ok(parsed)
    }

    /// Reads a `KEY_LEN <n>\n<raw bytes>\n` string field written by
    /// `write_kv_str`.
    fn read_kv_str(&mut self, key: &str) -> Result<String, LoadError> {
        let len = self.read_kv_usize(&format!("{key}_LEN"))?;
        self.read_bytes_field(len)
    }

    fn read_kv_str_raw(&mut self, key: &str) -> Result<String, LoadError> {
        self.read_kv_str(key)
    }
}

#[cfg(test)]
#[path = "../tests-rs/test_session_persist.rs"]
mod tests_session_persist;
