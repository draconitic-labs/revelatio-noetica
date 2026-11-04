//! What the atlas remembers between runs, and shareable scene links.
//!
//! Everything lives in one JSON file under the user's config directory. A bad
//! or missing file means a fresh start, never a crash.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Interface choices. `None` means "pick from the window size".
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Prefs {
    pub atlas_open: Option<bool>,
    pub panel_open: Option<bool>,
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
            film_open: true,
            ui_scale: 1.0,
            reduce_motion: false,
            closed_sections: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

    /// Load from disk. The error string, if any, is shown to the user.
    pub fn load() -> (Self, Option<String>) {
        let Some(path) = state_path() else {
            return (Self::default(), None);
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return (Self::default(), None);
        };
        match serde_json::from_str(&text) {
            Ok(session) => (session, None),
            Err(_) => (
                Self::default(),
                Some(format!(
                    "Could not read {}; starting fresh. Saving will replace it.",
                    path.display()
                )),
            ),
        }
    }

    /// Write atomically: a crash mid-save must not lose the old file.
    pub fn save(&self) -> Result<()> {
        let path = state_path().context("no config directory is known for this user")?;
        let dir = path.parent().context("state path has no parent")?;
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating folder {}", dir.display()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }
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
