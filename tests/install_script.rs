//! Runs scripts/install-desktop.sh for real, in a throwaway HOME, with a stand-in for
//! cargo so that no build happens. update-desktop-database and gtk-update-icon-cache
//! are the real ones when present; each test holds with or without them.
#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/install-desktop.sh");
const BUILT: &[u8] = b"#!/bin/sh\necho built by the stand-in\n";

/// Reports a binary in a folder of its own, the way Cargo does with CARGO_TARGET_DIR.
const STAND_IN_CARGO: &str = r#"#!/bin/sh
[ -z "$FAIL_THE_BUILD" ] || exit 101
printf '%s\n' '{"reason":"compiler-artifact","executable":null,"fresh":true}'
printf '{"reason":"compiler-artifact","executable":"%s","fresh":false}\n' "$BUILT_BINARY"
printf '%s\n' '{"reason":"build-finished","success":true}'
"#;

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("install-{name}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("tools")).unwrap();
        fs::create_dir_all(root.join("elsewhere/release")).unwrap();
        let sandbox = Self { root };
        sandbox.write_executable("tools/cargo", STAND_IN_CARGO.as_bytes());
        sandbox.write_executable("elsewhere/release/revelatio", BUILT);
        sandbox
    }

    fn write_executable(&self, path: &str, bytes: &[u8]) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.join(path);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn home(&self, path: &str) -> PathBuf {
        self.root.join("home").join(path)
    }

    fn apps(&self, path: &str) -> PathBuf {
        self.home(".local/share/applications").join(path)
    }

    fn icons(&self, path: &str) -> PathBuf {
        self.home(".local/share/icons/hicolor").join(path)
    }

    fn command(&self, args: &[&str]) -> Command {
        self.command_at(Path::new(SCRIPT), args)
    }

    fn command_at(&self, script: &Path, args: &[&str]) -> Command {
        let path = format!(
            "{}:{}",
            self.root.join("tools").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut c = Command::new("bash");
        c.arg(script)
            .args(args)
            .env("HOME", self.root.join("home"))
            .env("PATH", path)
            .env(
                "BUILT_BINARY",
                self.root.join("elsewhere/release/revelatio"),
            )
            .env_remove("XDG_DATA_HOME")
            .env_remove("FAIL_THE_BUILD");
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn run_ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Another app arrives after the install, and its entry reaches the shared cache
    /// whether or not the real tool is here to put it there.
    fn add_other_app(&self) {
        fs::write(
            self.apps("other-app.desktop"),
            "[Desktop Entry]\nType=Application\nName=Other\nExec=other %u\nMimeType=x-scheme-handler/other;\n",
        )
        .unwrap();
        let cache = self.apps("mimeinfo.cache");
        let mut text = fs::read_to_string(&cache).unwrap_or_else(|_| "[MIME Cache]\n".into());
        text.push_str("x-scheme-handler/other=other-app.desktop;\n");
        fs::write(cache, text).unwrap();
    }

    fn files_left(&self) -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, out);
                }
                out.push(path);
            }
        }
        let mut out = Vec::new();
        walk(&self.root.join("home"), &mut out);
        out
    }
}

#[test]
fn help_prints_the_whole_header_and_stops() {
    let help = Sandbox::new("help").run_ok(&["--help"]);
    assert!(help.starts_with("Install Revelatio Noetica"), "{help}");
    assert!(
        help.trim_end().ends_with("are left alone."),
        "stops mid-sentence: {help}"
    );
    assert!(!help.contains("set -e"), "ran past the header: {help}");
}

#[test]
fn installs_the_binary_cargo_built_wherever_it_went() {
    let s = Sandbox::new("built");
    s.run_ok(&[]);
    assert_eq!(fs::read(s.home(".local/bin/revelatio")).unwrap(), BUILT);
}

#[test]
fn a_failed_build_installs_nothing() {
    let s = Sandbox::new("failed");
    let out = s.command(&[]).env("FAIL_THE_BUILD", "1").output().unwrap();
    assert!(!out.status.success());
    assert!(!s.home(".local/bin/revelatio").exists());
}

#[test]
fn uninstall_leaves_nothing_behind_when_alone() {
    let s = Sandbox::new("alone");
    s.run_ok(&[]);
    assert!(s.home(".local/bin/revelatio").exists());
    s.run_ok(&["--uninstall"]);
    assert_eq!(s.files_left(), Vec::<PathBuf>::new());
}

#[test]
fn uninstall_keeps_a_shared_mime_cache_that_lists_another_app() {
    let s = Sandbox::new("mime");
    s.run_ok(&[]);
    s.add_other_app();
    let said = s.run_ok(&["--uninstall"]);
    let cache = fs::read_to_string(s.apps("mimeinfo.cache")).expect("the cache was deleted");
    assert!(cache.contains("other-app.desktop"), "{cache}");
    assert!(said.contains("lists other apps"), "{said}");
    assert!(s.apps("other-app.desktop").exists());
    assert!(!s.home(".local/bin/revelatio").exists());
    assert!(!s.apps("revelatio-noetica.desktop").exists());
}

#[test]
fn uninstall_keeps_an_icon_cache_that_other_icons_use() {
    let s = Sandbox::new("icons");
    s.run_ok(&[]);
    let other = s.icons("256x256/apps/other-app.png");
    fs::write(&other, b"other").unwrap();
    let cache = s.icons("icon-theme.cache");
    if !cache.exists() {
        fs::write(&cache, b"cache").unwrap();
    }
    s.run_ok(&["--uninstall"]);
    assert!(cache.exists(), "the cache was deleted");
    assert!(other.exists());
    assert!(!s.icons("256x256/apps/revelatio-noetica.png").exists());
}

#[test]
fn uninstall_leaves_a_binary_that_changed_since_install() {
    let s = Sandbox::new("changed");
    s.run_ok(&[]);
    fs::write(s.home(".local/bin/revelatio"), b"someone else's").unwrap();
    let said = s.run_ok(&["--uninstall"]);
    assert!(said.contains("changed since install"), "{said}");
    assert_eq!(
        fs::read(s.home(".local/bin/revelatio")).unwrap(),
        b"someone else's"
    );
}

#[test]
fn install_refuses_a_file_it_did_not_put_there_unless_forced() {
    let s = Sandbox::new("refuse");
    fs::create_dir_all(s.home(".local/bin")).unwrap();
    fs::write(s.home(".local/bin/revelatio"), b"someone else's").unwrap();
    assert!(!s.run(&[]).status.success());
    assert_eq!(
        fs::read(s.home(".local/bin/revelatio")).unwrap(),
        b"someone else's"
    );
    s.run_ok(&["--force"]);
    assert_eq!(fs::read(s.home(".local/bin/revelatio")).unwrap(), BUILT);
}

/// The Linux release archive ships the script beside a built binary, the icons, and
/// the menu entry. Run from there, it installs those and never calls cargo.
#[test]
fn installs_the_prebuilt_binary_from_a_release_archive() {
    let s = Sandbox::new("archive");
    let archive = s.root.join("revelatio-noetica-linux");
    fs::create_dir_all(&archive).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::copy(SCRIPT, archive.join("install-desktop.sh")).unwrap();
    for name in ["icon-256.png", "icon.svg", "revelatio-noetica.desktop"] {
        fs::copy(repo.join("assets").join(name), archive.join(name)).unwrap();
    }
    // Without its executable bit, as after a copy that drops modes: still the archive.
    const PREBUILT: &[u8] = b"#!/bin/sh\necho prebuilt\n";
    fs::write(archive.join("revelatio"), PREBUILT).unwrap();

    // The stand-in cargo fails if anything calls it.
    let out = s
        .command_at(&archive.join("install-desktop.sh"), &[])
        .env("FAIL_THE_BUILD", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read(s.home(".local/bin/revelatio")).unwrap(), PREBUILT);
    assert_eq!(
        fs::read(s.icons("256x256/apps/revelatio-noetica.png")).unwrap(),
        fs::read(repo.join("assets/icon-256.png")).unwrap()
    );
    let entry = fs::read_to_string(s.apps("revelatio-noetica.desktop")).unwrap();
    let exec = format!("Exec=\"{}\" %u", s.home(".local/bin/revelatio").display());
    assert!(entry.lines().any(|l| l == exec), "{entry}");
    assert!(entry.contains("Icon=revelatio-noetica\n"), "{entry}");

    s.run_ok(&["--uninstall"]);
    assert_eq!(s.files_left(), Vec::<PathBuf>::new());
}
