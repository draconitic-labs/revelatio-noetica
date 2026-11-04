#!/usr/bin/env bash
# Install Revelatio Noetica for this user: builds in release mode, then puts the
# binary, icon, and application-menu entry under ~/.local. Nothing needs root.
#
#   scripts/install-desktop.sh              build and install
#   scripts/install-desktop.sh --uninstall  remove exactly what install added
#
# Your session file (~/.config/revelatio-noetica) and exported images are yours;
# uninstalling leaves them alone.
set -euo pipefail

app_id="revelatio-noetica"
bin_dir="$HOME/.local/bin"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
apps_dir="$data_dir/applications"
icons_dir="$data_dir/icons/hicolor"

bin_path="$bin_dir/revelatio"
desktop_path="$apps_dir/$app_id.desktop"
icon_png="$icons_dir/256x256/apps/$app_id.png"
icon_svg="$icons_dir/scalable/apps/$app_id.svg"

refresh_caches() {
    if command -v update-desktop-database >/dev/null; then
        update-desktop-database "$apps_dir" >/dev/null 2>&1 || true
    fi
    if command -v gtk-update-icon-cache >/dev/null; then
        gtk-update-icon-cache -q -t "$icons_dir" >/dev/null 2>&1 || true
    fi
}

uninstall() {
    local removed=0
    for path in "$bin_path" "$desktop_path" "$icon_png" "$icon_svg"; do
        if [[ -e "$path" ]]; then
            rm -- "$path"
            echo "removed $path"
            removed=1
        fi
    done
    if ((removed == 0)); then
        echo "nothing to remove: Revelatio Noetica is not installed for this user"
        return
    fi
    refresh_caches
    echo "uninstalled; your session and exports were left in place"
}

install_app() {
    local repo
    repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    command -v cargo >/dev/null || {
        echo "install-desktop: cargo not found; install Rust from https://rustup.rs" >&2
        exit 1
    }

    echo "building release binary (this takes a few minutes the first time)"
    cargo build --release --manifest-path "$repo/Cargo.toml"

    mkdir -p "$bin_dir" "$apps_dir" "$(dirname "$icon_png")" "$(dirname "$icon_svg")"
    install -m 0755 "$repo/target/release/revelatio" "$bin_path"
    install -m 0644 "$repo/assets/icon-256.png" "$icon_png"
    install -m 0644 "$repo/assets/icon.svg" "$icon_svg"

    # Exec uses the absolute path because ~/.local/bin is not always on PATH for
    # the session that starts launchers. %u passes scene links through.
    cat >"$desktop_path" <<DESKTOP
[Desktop Entry]
Type=Application
Version=1.5
Name=Revelatio Noetica
GenericName=Atlas of mathematics and physics
Comment=Explore mathematics and physics as live GPU visualizations
Exec="$bin_path" %u
Icon=$app_id
Terminal=false
Categories=Science;Math;Physics;
Keywords=mathematics;physics;calculus;visualization;simulation;
MimeType=x-scheme-handler/revelatio;
StartupWMClass=$app_id
DESKTOP
    chmod 0644 "$desktop_path"

    refresh_caches
    echo "installed:"
    echo "  $bin_path"
    echo "  $desktop_path"
    echo "  $icon_png"
    echo "  $icon_svg"
    echo "Look for Revelatio Noetica in your application menu."
}

case "${1:-}" in
    "") install_app ;;
    --uninstall | -u) uninstall ;;
    --help | -h) sed -n '2,8p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' ;;
    *)
        echo "install-desktop: unknown option '$1' (use --uninstall or --help)" >&2
        exit 2
        ;;
esac
