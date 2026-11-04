#!/usr/bin/env bash
# Install Revelatio Noetica for this user: builds in release mode, then puts the
# binary, icon, and application-menu entry under ~/.local. Nothing needs root.
#
#   scripts/install-desktop.sh              build and install
#   scripts/install-desktop.sh --force      also replace files it did not install
#   scripts/install-desktop.sh --uninstall  remove what the install recorded
#
# Uninstall removes a file only if it still matches what was installed, so a
# different ~/.local/bin/revelatio is safe. Your session file
# (~/.config/revelatio-noetica) and exported images are yours and are left alone.
set -euo pipefail

app_id="revelatio-noetica"
bin_dir="$HOME/.local/bin"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
apps_dir="$data_dir/applications"
icons_dir="$data_dir/icons/hicolor"
state_dir="$data_dir/$app_id"
manifest="$state_dir/install-manifest"

bin_path="$bin_dir/revelatio"
desktop_path="$apps_dir/$app_id.desktop"
icon_png="$icons_dir/256x256/apps/$app_id.png"
icon_svg="$icons_dir/scalable/apps/$app_id.svg"
mime_cache="$apps_dir/mimeinfo.cache"
icon_cache="$icons_dir/icon-theme.cache"

refresh_caches() {
    if command -v update-desktop-database >/dev/null && [[ -d "$apps_dir" ]]; then
        update-desktop-database "$apps_dir" >/dev/null 2>&1 || true
    fi
    if command -v gtk-update-icon-cache >/dev/null && [[ -d "$icons_dir" ]]; then
        gtk-update-icon-cache -q -t "$icons_dir" >/dev/null 2>&1 || true
    fi
}

hash_of() {
    sha256sum -- "$1" | cut -d' ' -f1
}

# The hash recorded for a path by the last install, if any.
recorded_hash() {
    [[ -f "$manifest" ]] || return 0
    awk -F'\t' -v path="$1" '$1 == "file" && $3 == path { print $2 }' "$manifest"
}

# Refuse to replace a file that this script did not put there.
check_replaceable() {
    local path="$1" recorded
    [[ -e "$path" ]] || return 0
    recorded="$(recorded_hash "$path")"
    if [[ -n "$recorded" && "$recorded" == "$(hash_of "$path")" ]]; then
        return 0
    fi
    echo "install-desktop: $path exists and was not installed by this script." >&2
    echo "Move it away, or rerun with --force to replace it." >&2
    exit 1
}

# Create a folder, recording every level that did not exist.
make_dir() {
    local dir="$1" missing=()
    while [[ ! -d "$dir" ]]; do
        missing+=("$dir")
        dir="$(dirname -- "$dir")"
    done
    if ((${#missing[@]} > 0)); then
        mkdir -p -- "$1"
        printf 'dir\t-\t%s\n' "${missing[@]}" >>"$new_manifest"
    fi
}

uninstall() {
    if [[ ! -f "$manifest" ]]; then
        echo "nothing to remove: no install by this script is recorded"
        return
    fi
    local kind hash path dirs=() caches=()
    while IFS=$'\t' read -r kind hash path; do
        case "$kind" in
            file)
                if [[ ! -e "$path" ]]; then
                    continue
                elif [[ "$(hash_of "$path")" == "$hash" ]]; then
                    rm -- "$path"
                    echo "removed $path"
                else
                    echo "left in place, changed since install: $path"
                fi
                ;;
            dir) dirs+=("$path") ;;
            cache) caches+=("$path") ;;
        esac
    done <"$manifest"

    # Rebuild shared caches so they no longer list this app, then drop the ones
    # that did not exist before the install.
    refresh_caches
    local item
    for item in ${caches[@]+"${caches[@]}"}; do
        if [[ -e "$item" ]]; then
            rm -- "$item"
            echo "removed $item"
        fi
    done
    rm -- "$manifest"
    # Longest path first, so children go before parents, and only if empty: other
    # apps may have moved in.
    if ((${#dirs[@]} > 0)); then
        while IFS= read -r item; do
            rmdir -- "$item" 2>/dev/null || true
        done < <(printf '%s\n' "${dirs[@]}" | awk '{ print length($0) "\t" $0 }' | sort -rn | cut -f2-)
    fi
    echo "uninstalled; your session and exports were left in place"
}

install_app() {
    local force="$1" repo
    repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    command -v cargo >/dev/null || {
        echo "install-desktop: cargo not found; install Rust from https://rustup.rs" >&2
        exit 1
    }
    if [[ "$force" != yes ]]; then
        for path in "$bin_path" "$desktop_path" "$icon_png" "$icon_svg"; do
            check_replaceable "$path"
        done
    fi

    echo "building release binary (this takes a few minutes the first time)"
    cargo build --release --manifest-path "$repo/Cargo.toml"

    new_manifest="$(mktemp)"
    # Keep what the first install recorded as created by it; a reinstall must not
    # forget that, or a later uninstall would leave those behind.
    if [[ -f "$manifest" ]]; then
        awk -F'\t' '$1 == "dir" || $1 == "cache"' "$manifest" >>"$new_manifest"
    fi

    make_dir "$state_dir"
    make_dir "$bin_dir"
    make_dir "$apps_dir"
    make_dir "$(dirname -- "$icon_png")"
    make_dir "$(dirname -- "$icon_svg")"
    for cache in "$mime_cache" "$icon_cache"; do
        if [[ ! -e "$cache" ]]; then
            printf 'cache\t-\t%s\n' "$cache" >>"$new_manifest"
        fi
    done

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

    for path in "$bin_path" "$desktop_path" "$icon_png" "$icon_svg"; do
        printf 'file\t%s\t%s\n' "$(hash_of "$path")" "$path" >>"$new_manifest"
    done
    mv -- "$new_manifest" "$manifest"

    refresh_caches
    echo "installed:"
    echo "  $bin_path"
    echo "  $desktop_path"
    echo "  $icon_png"
    echo "  $icon_svg"
    echo "Look for Revelatio Noetica in your application menu."
}

case "${1:-}" in
    "") install_app no ;;
    --force) install_app yes ;;
    --uninstall | -u) uninstall ;;
    --help | -h) sed -n '2,10p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' ;;
    *)
        echo "install-desktop: unknown option '$1' (use --uninstall, --force, or --help)" >&2
        exit 2
        ;;
esac
