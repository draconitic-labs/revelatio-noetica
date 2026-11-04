//! What the atlas remembers between runs, and shareable scene links.
//!
//! Everything lives in one JSON file under the user's config directory. A bad
//! or missing file means a fresh start, never a crash.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Interface choices. `None` means "pick from the window size".
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Prefs {
    pub atlas_open: Option<bool>,
    pub panel_open: Option<bool>,
    pub panel_width: Option<f32>,
    pub film_open: bool,
    pub ui_scale: f32,
    pub reduce_motion: bool,
    pub closed_sections: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            atlas_open: None,
            panel_open: None,
            panel_width: None,
            film_open: true,
            ui_scale: 1.0,
            reduce_motion: false,
            closed_sections: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub name: String,
    pub chamber: String,
    pub scene: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Session {
    pub prefs: Prefs,
    /// Views seen so far, as "chamber/view".
    pub seen: Vec<String>,
    /// Most recent first, as "chamber/view".
    pub recent: Vec<String>,
    pub bookmarks: Vec<Bookmark>,
    /// The scene on screen when last saved, to resume from.
    pub last_scene: Option<Value>,
    /// The file on disk could not be backed up, so it must not be replaced.
    #[serde(skip)]
    pub hold_saves: bool,
    /// The bookmarks on disk when this window last read or wrote them. Anything that
    /// differs from this now was changed since, here or in another window.
    #[serde(skip)]
    synced: Vec<Bookmark>,
    /// The damaged file found at load was already copied aside.
    #[serde(skip)]
    copied_at_load: bool,
}

const MAX_RECENT: usize = 12;

pub fn view_key(chamber: &str, view: &str) -> String {
    format!("{chamber}/{view}")
}

impl Session {
    pub fn mark_seen(&mut self, chamber: &str, view: &str) {
        let key = view_key(chamber, view);
        if !self.seen.contains(&key) {
            self.seen.push(key.clone());
        }
        self.recent.retain(|k| k != &key);
        self.recent.insert(0, key);
        self.recent.truncate(MAX_RECENT);
    }

    pub fn has_seen(&self, chamber: &str, view: &str) -> bool {
        self.seen.iter().any(|k| {
            k.split_once('/')
                .is_some_and(|(c, v)| c == chamber && v == view)
        })
    }

    pub fn has_visited(&self, chamber: &str) -> bool {
        self.seen
            .iter()
            .any(|k| k.split_once('/').is_some_and(|(c, _)| c == chamber))
    }

    /// Load from disk. The message, if any, is shown to the user.
    pub fn load() -> (Self, Option<String>) {
        match state_path() {
            Some(path) => Self::load_from(&path),
            None => (Self::default(), None),
        }
    }

    /// Read what can be read. If anything has to be dropped, the original bytes are
    /// first copied aside, because the next save replaces this file. If that copy
    /// fails, saving stays off for this run so the file is left exactly as it is.
    fn load_from(path: &Path) -> (Self, Option<String>) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            // No file yet: a first run.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), None);
            }
            // It exists but cannot be read, so it cannot be copied either.
            Err(e) => {
                let session = Self {
                    hold_saves: true,
                    ..Self::default()
                };
                let message = format!(
                    "Could not read {} ({e}). Nothing will be saved this run, so the file stays as it is.",
                    path.display()
                );
                return (session, Some(message));
            }
        };
        let (mut session, dropped) = parse_tolerant(&bytes);
        session.synced = session.bookmarks.clone();
        if dropped == 0 {
            return (session, None);
        }
        match keep_a_copy(path, &bytes) {
            Ok(copy) => {
                session.copied_at_load = true;
                (
                    session,
                    Some(format!(
                        "Part of your session could not be read. The original is saved at {}.",
                        copy.display()
                    )),
                )
            }
            Err(e) => {
                session.hold_saves = true;
                (
                    session,
                    Some(format!(
                        "Part of {} could not be read, and no copy could be made ({e}). Nothing will be saved this run, so the file stays as it is.",
                        path.display()
                    )),
                )
            }
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let path = state_path().context("no config directory is known for this user")?;
        self.save_to(&path)
    }

    /// Write atomically: a crash mid-save must not lose the old file. Another window
    /// may have saved since this one last looked, so its changes are merged first,
    /// with a lock so that two windows saving at once cannot miss each other.
    fn save_to(&mut self, path: &Path) -> Result<()> {
        if self.hold_saves {
            return Ok(());
        }
        let dir = path.parent().context("state path has no parent")?;
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating folder {}", dir.display()))?;
        // Advisory, and released when this returns. A folder that cannot lock can still
        // save; it only loses the protection against the same instant.
        let lock = std::fs::File::create(path.with_extension("lock"));
        if let Ok(lock) = &lock {
            let _ = lock.lock();
        }
        self.merge_from(path)?;
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
        self.synced = self.bookmarks.clone();
        Ok(())
    }

    /// Take in what another window saved since `synced`: bookmarks it added stay, and
    /// bookmarks it removed go, while what this window removed stays removed.
    fn merge_from(&mut self, path: &Path) -> Result<()> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let (disk, dropped) = parse_tolerant(&bytes);
        if dropped > 0 && !self.copied_at_load {
            keep_a_copy(path, &bytes)
                .with_context(|| format!("keeping a copy of damaged {}", path.display()))?;
        }
        self.copied_at_load = false;
        // In a damaged file, a missing bookmark says nothing about anyone removing it.
        if dropped == 0 {
            self.bookmarks
                .retain(|b| !self.synced.contains(b) || disk.bookmarks.contains(b));
        }
        for b in disk.bookmarks {
            if !self.synced.contains(&b) && !self.bookmarks.contains(&b) {
                self.bookmarks.push(b);
            }
        }
        for key in disk.seen {
            if !self.seen.contains(&key) {
                self.seen.push(key);
            }
        }
        Ok(())
    }
}

/// Parse a session field by field, so one bad entry costs only that entry. Returns
/// the session and how many things had to be dropped.
fn parse_tolerant(bytes: &[u8]) -> (Session, usize) {
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(bytes) else {
        return (Session::default(), 1);
    };
    let mut dropped = 0;
    let field = |key: &str| map.get(key).cloned();
    let mut session = Session::default();
    if let Some(v) = field("prefs") {
        match serde_json::from_value::<Prefs>(v) {
            Ok(prefs) => session.prefs = prefs,
            Err(_) => dropped += 1,
        }
    }
    if !(0.6..=2.5).contains(&session.prefs.ui_scale) {
        session.prefs.ui_scale = 1.0;
    }
    for (key, list) in [("seen", &mut session.seen), ("recent", &mut session.recent)] {
        if let Some(v) = field(key) {
            match serde_json::from_value::<Vec<String>>(v) {
                Ok(items) => *list = items,
                Err(_) => dropped += 1,
            }
        }
    }
    if let Some(v) = field("bookmarks") {
        match v {
            Value::Array(items) => {
                for item in items {
                    match serde_json::from_value::<Bookmark>(item) {
                        Ok(b) => session.bookmarks.push(b),
                        Err(_) => dropped += 1,
                    }
                }
            }
            _ => dropped += 1,
        }
    }
    session.last_scene = field("last_scene").filter(|v| !v.is_null());
    (session, dropped)
}

/// Copy the bytes to a new file beside the original, never replacing an older copy.
fn keep_a_copy(path: &Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    for n in 0..1000 {
        let copy = path.with_file_name(format!("session.damaged-{stamp}-{n}.json"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&copy)
        {
            Ok(mut file) => {
                file.write_all(bytes)?;
                return Ok(copy);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::ErrorKind::AlreadyExists.into())
}

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn state_path() -> Option<PathBuf> {
    let base = env_dir("XDG_CONFIG_HOME")
        .or_else(|| env_dir("HOME").map(|h| h.join(".config")))
        .or_else(|| env_dir("APPDATA"))?;
    Some(base.join("revelatio-noetica").join("session.json"))
}

/// Where the window writes stills, frames, and scene files: a folder the user
/// can find regardless of where the app was launched from.
pub fn export_dir() -> PathBuf {
    if let Some(dir) = env_dir("REVELATIO_EXPORT_DIR") {
        return dir;
    }
    let pictures = env_dir("XDG_PICTURES_DIR")
        .or_else(|| env_dir("HOME").map(|h| h.join("Pictures")))
        .or_else(|| env_dir("USERPROFILE").map(|h| h.join("Pictures")));
    match pictures {
        Some(dir) => dir.join("Revelatio Noetica"),
        None => PathBuf::from("exports"),
    }
}

const LINK_PREFIX: &str = "revelatio:scene?";

/// A scene as one line of URL-safe text.
pub fn encode_link(scene: &Value) -> String {
    let json = scene.to_string();
    let mut out = String::with_capacity(LINK_PREFIX.len() + json.len() * 3 / 2);
    out.push_str(LINK_PREFIX);
    for b in json.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

pub fn is_link(text: &str) -> bool {
    text.trim_start().starts_with(LINK_PREFIX)
}

/// Parse a scene link. Input is untrusted; malformed text is an error.
pub fn decode_link(text: &str) -> Result<Value> {
    let Some(body) = text.trim().strip_prefix(LINK_PREFIX) else {
        bail!("not a Revelatio scene link");
    };
    let bytes = body.as_bytes();
    let mut raw = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .context("the link has a broken % escape")?;
            raw.push(hex);
            i += 3;
        } else {
            raw.push(bytes[i]);
            i += 1;
        }
    }
    serde_json::from_slice(&raw).context("the link does not contain a valid scene")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A private folder for one test, removed when the test ends.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("revelatio-test-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn copies_in(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().contains("session.damaged"))
            .collect()
    }

    #[test]
    fn one_malformed_bookmark_costs_only_that_bookmark() {
        let (session, dropped) = parse_tolerant(
            json!({
                "seen": ["a/b"],
                "bookmarks": [
                    { "name": "good", "chamber": "c", "scene": {} },
                    { "name": 7 },
                    { "name": "also good", "chamber": "c", "scene": {} },
                ],
            })
            .to_string()
            .as_bytes(),
        );
        assert_eq!(dropped, 1);
        assert_eq!(session.seen, vec!["a/b"]);
        let names: Vec<&str> = session.bookmarks.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, vec!["good", "also good"]);
    }

    #[test]
    fn a_damaged_file_is_copied_aside_before_any_save_replaces_it() {
        for original in [&b"{ this is not json"[..], &[0xFF, 0xFE, 0x00][..]] {
            let dir = TempDir::new(&format!("damaged-{}", original.len()));
            let path = dir.0.join("session.json");
            std::fs::write(&path, original).unwrap();
            let (mut session, message) = Session::load_from(&path);
            assert!(message.is_some(), "the user must be told");
            assert!(!session.hold_saves);
            session.save_to(&path).unwrap();
            let copies = copies_in(&dir.0);
            assert_eq!(copies.len(), 1);
            assert_eq!(std::fs::read(&copies[0]).unwrap(), original);
        }
    }

    #[test]
    fn a_good_file_makes_no_copy() {
        let dir = TempDir::new("good");
        let path = dir.0.join("session.json");
        Session::default().save_to(&path).unwrap();
        let (_, message) = Session::load_from(&path);
        assert!(message.is_none());
        assert!(copies_in(&dir.0).is_empty());
    }

    #[test]
    fn with_no_way_to_copy_the_file_is_never_replaced() {
        let dir = TempDir::new("nocopy");
        let path = dir.0.join("session.json");
        std::fs::write(&path, b"damaged").unwrap();
        let mut held = Session {
            hold_saves: true,
            ..Session::default()
        };
        held.save_to(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"damaged");
    }

    fn bookmark(name: &str) -> Bookmark {
        Bookmark {
            name: name.into(),
            chamber: "c".into(),
            scene: json!({ "t": 1.5 }),
        }
    }

    fn names_on_disk(path: &Path) -> Vec<String> {
        let (session, _) = Session::load_from(path);
        session.bookmarks.into_iter().map(|b| b.name).collect()
    }

    /// Two windows that have both read a file holding these bookmarks.
    fn two_windows(path: &Path, names: &[&str]) -> (Session, Session) {
        let mut first = Session {
            bookmarks: names.iter().map(|n| bookmark(n)).collect(),
            ..Session::default()
        };
        first.save_to(path).unwrap();
        (Session::load_from(path).0, Session::load_from(path).0)
    }

    #[test]
    fn a_bookmark_from_a_second_window_survives_the_first_window_closing() {
        let dir = TempDir::new("two-windows");
        let path = dir.0.join("session.json");
        let (mut first, mut second) = two_windows(&path, &["old"]);
        second.bookmarks.push(bookmark("from the link window"));
        second.save_to(&path).unwrap();
        first.save_to(&path).unwrap();
        assert_eq!(names_on_disk(&path), ["old", "from the link window"]);
        assert_eq!(first.bookmarks.len(), 2, "this window now shows it too");
    }

    #[test]
    fn a_bookmark_removed_in_either_window_stays_removed() {
        let dir = TempDir::new("removed");
        let path = dir.0.join("session.json");
        let (mut first, mut second) = two_windows(&path, &["a", "b"]);
        // Removed here, while the other window still holds it and saves first.
        first.bookmarks.retain(|b| b.name != "a");
        second.save_to(&path).unwrap();
        first.save_to(&path).unwrap();
        assert_eq!(names_on_disk(&path), ["b"]);
        // Removed elsewhere: this window, still holding it, must not bring it back.
        second.save_to(&path).unwrap();
        assert_eq!(names_on_disk(&path), ["b"]);
        assert_eq!(second.bookmarks.len(), 1);
    }

    #[test]
    fn both_windows_keep_what_they_added_and_saw() {
        let dir = TempDir::new("both");
        let path = dir.0.join("session.json");
        let (mut first, mut second) = two_windows(&path, &[]);
        first.bookmarks.push(bookmark("mine"));
        first.mark_seen("c", "here");
        second.bookmarks.push(bookmark("theirs"));
        second.mark_seen("c", "there");
        first.save_to(&path).unwrap();
        second.save_to(&path).unwrap();
        first.save_to(&path).unwrap();
        let (on_disk, _) = Session::load_from(&path);
        assert_eq!(on_disk.bookmarks.len(), 2);
        assert!(on_disk.has_seen("c", "here") && on_disk.has_seen("c", "there"));
    }

    #[test]
    fn saving_twice_does_not_duplicate_a_bookmark() {
        let dir = TempDir::new("twice");
        let path = dir.0.join("session.json");
        let mut session = Session::default();
        session.bookmarks.push(bookmark("once"));
        session.save_to(&path).unwrap();
        session.save_to(&path).unwrap();
        assert_eq!(names_on_disk(&path), ["once"]);
    }

    #[test]
    fn a_file_damaged_while_running_is_copied_and_costs_this_window_nothing() {
        let dir = TempDir::new("damaged-later");
        let path = dir.0.join("session.json");
        let (mut window, _) = two_windows(&path, &["keep"]);
        std::fs::write(&path, b"{ damaged").unwrap();
        window.save_to(&path).unwrap();
        assert_eq!(names_on_disk(&path), ["keep"]);
        let copies = copies_in(&dir.0);
        assert_eq!(copies.len(), 1);
        assert_eq!(std::fs::read(&copies[0]).unwrap(), b"{ damaged");
    }

    #[test]
    fn a_deleted_file_is_simply_written_again() {
        let dir = TempDir::new("deleted");
        let path = dir.0.join("session.json");
        let (mut window, _) = two_windows(&path, &["keep"]);
        std::fs::remove_file(&path).unwrap();
        window.save_to(&path).unwrap();
        assert_eq!(names_on_disk(&path), ["keep"]);
    }

    #[test]
    fn saved_numbers_come_back_exactly() {
        // Charges are f32, widened to f64 in the scene; sliders are f64. The saved-view
        // star compares these with ==, so one unit in the last place turns it off.
        let dir = TempDir::new("numbers");
        let path = dir.0.join("session.json");
        let mut state = 0x1234_5678u64;
        let mut uniform = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut session = Session::default();
        for _ in 0..1000 {
            let charges: Vec<[f32; 2]> = (0..4)
                .map(|_| {
                    [
                        (uniform() * 6.8 - 3.4) as f32,
                        (uniform() * 6.8 - 3.4) as f32,
                    ]
                })
                .collect();
            session.bookmarks.push(Bookmark {
                name: "b".into(),
                chamber: "em.fields".into(),
                scene: json!({
                    "placed": { "em.fields": charges },
                    "all_params": { "em.fields": { "k": uniform() * 7.0 + 2.0 } },
                }),
            });
        }
        session.save_to(&path).unwrap();
        let (back, _) = Session::load_from(&path);
        assert_eq!(back.bookmarks.len(), session.bookmarks.len());
        for (a, b) in session.bookmarks.iter().zip(&back.bookmarks) {
            assert_eq!(a.scene, b.scene);
        }
    }

    #[test]
    fn an_absurd_interface_scale_is_not_believed() {
        let (session, _) = parse_tolerant(br#"{ "prefs": { "ui_scale": -3.0 } }"#);
        assert_eq!(session.prefs.ui_scale, 1.0);
    }

    #[test]
    fn links_round_trip_and_stay_url_safe() {
        let scene = json!({
            "module": "quantum.schrodinger",
            "t": 3.25,
            "params": { "state": 2, "label": "Schrödinger & ψ" },
        });
        let link = encode_link(&scene);
        assert!(link.starts_with(LINK_PREFIX));
        assert!(is_link(&format!("  {link}")));
        assert!(link[LINK_PREFIX.len()..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'%' | b'-' | b'_' | b'.' | b'~')));
        assert_eq!(decode_link(&link).unwrap(), scene);
    }

    #[test]
    fn malformed_links_are_errors_not_panics() {
        for bad in [
            "",
            "https://example.com",
            "revelatio:scene?",
            "revelatio:scene?%",
            "revelatio:scene?%4",
            "revelatio:scene?%ZZ",
            "revelatio:scene?%FF%FE",
            "revelatio:scene?notjson",
            "revelatio:scene?%C3",
        ] {
            assert!(decode_link(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn recent_is_ordered_unique_and_bounded() {
        let mut s = Session::default();
        for i in 0..20 {
            s.mark_seen("c", &format!("v{i}"));
        }
        s.mark_seen("c", "v5");
        assert_eq!(s.recent.len(), MAX_RECENT);
        assert_eq!(s.recent[0], "c/v5");
        assert_eq!(s.recent.iter().filter(|k| *k == "c/v5").count(), 1);
        assert_eq!(s.seen.len(), 20);
        assert!(s.has_seen("c", "v0") && s.has_visited("c"));
        assert!(!s.has_seen("c", "nope") && !s.has_visited("other"));
    }

    #[test]
    fn older_or_partial_session_files_still_load() {
        let s: Session = serde_json::from_value(json!({ "seen": ["a/b"] })).unwrap();
        assert_eq!(s.seen, vec!["a/b"]);
        assert!(s.prefs.film_open, "missing prefs take their defaults");
        let p: Prefs = serde_json::from_value(json!({ "ui_scale": 1.25 })).unwrap();
        assert_eq!(p.ui_scale, 1.25);
        assert!(p.film_open);
    }
}
