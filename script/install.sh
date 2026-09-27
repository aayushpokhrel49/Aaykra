#!/usr/bin/env sh
set -eu

# Downloads an AAYKRA release from GitHub and unpacks it into ~/.local/.

main() {
    platform="$(uname -s)"
    arch="$(uname -m)"
    channel="${ZED_CHANNEL:-stable}"
    repo="aayushpokhrel49/Aaykra"
    ZED_VERSION="${ZED_VERSION:-latest}"
    if [ "$ZED_VERSION" = "latest" ]; then
        download_base="https://github.com/$repo/releases/latest/download"
    else
        download_base="https://github.com/$repo/releases/download/v$ZED_VERSION"
    fi
    # Use TMPDIR if available (for environments with non-standard temp directories)
    if [ -n "${TMPDIR:-}" ] && [ -d "${TMPDIR}" ]; then
        temp="$(mktemp -d "$TMPDIR/aaykra-XXXXXX")"
    else
        temp="$(mktemp -d "/tmp/aaykra-XXXXXX")"
    fi

    if [ "$platform" = "Darwin" ]; then
        platform="macos"
    elif [ "$platform" = "Linux" ]; then
        platform="linux"
    else
        echo "Unsupported platform $platform"
        exit 1
    fi

    case "$platform-$arch" in
        macos-arm64* | linux-arm64* | linux-aarch64)
            arch="aarch64"
            ;;
        macos-x86* | linux-x86*)
            arch="x86_64"
            ;;
        *)
            echo "Unsupported platform or architecture"
            exit 1
            ;;
    esac

    if command -v curl >/dev/null 2>&1; then
        curl () {
            command curl -fL "$@"
        }
    elif command -v wget >/dev/null 2>&1; then
        curl () {
            wget -O- "$@"
        }
    else
        echo "Could not find 'curl' or 'wget' in your path"
        exit 1
    fi

    "$platform" "$@"

    if [ "$(command -v aaykra)" = "$HOME/.local/bin/aaykra" ]; then
        echo "AAYKRA has been installed. Run with 'aaykra'"
    else
        echo "To run AAYKRA from your terminal, you must add ~/.local/bin to your PATH"
        echo "Run:"

        case "$SHELL" in
            *zsh)
                echo "   echo 'export PATH=\$HOME/.local/bin:\$PATH' >> ~/.zshrc"
                echo "   source ~/.zshrc"
                ;;
            *fish)
                echo "   fish_add_path -U $HOME/.local/bin"
                ;;
            *)
                echo "   echo 'export PATH=\$HOME/.local/bin:\$PATH' >> ~/.bashrc"
                echo "   source ~/.bashrc"
                ;;
        esac

        echo "To run AAYKRA now, '~/.local/bin/aaykra'"
    fi
}

linux() {
    if [ -n "${ZED_BUNDLE_PATH:-}" ]; then
        cp "$ZED_BUNDLE_PATH" "$temp/aaykra-linux-$arch.tar.gz"
    else
        echo "Downloading AAYKRA version: $ZED_VERSION"
        curl "$download_base/aaykra-linux-$arch.tar.gz" > "$temp/aaykra-linux-$arch.tar.gz"
    fi

    suffix=""
    if [ "$channel" != "stable" ]; then
        suffix="-$channel"
    fi

    appid=""
    case "$channel" in
      stable)
        appid="me.aayush.Aaykra"
        ;;
      dev)
        appid="me.aayush.Aaykra-Dev"
        ;;
      *)
        echo "Unknown release channel: ${channel}. Using stable app ID."
        appid="me.aayush.Aaykra"
        ;;
    esac

    # Unpack
    rm -rf "$HOME/.local/aaykra$suffix.app"
    mkdir -p "$HOME/.local/aaykra$suffix.app"
    tar -xzf "$temp/aaykra-linux-$arch.tar.gz" -C "$HOME/.local/"

    aaykra_editor="$HOME/.local/aaykra$suffix.app/libexec/aaykra-editor"
    if [ -f "$aaykra_editor" ] && command -v ldd >/dev/null 2>&1; then
        missing="$(ldd "$aaykra_editor" 2>/dev/null | sed -n 's/^[[:space:]]*\(.*\) => not found$/\1/p')"
        if [ -n "$missing" ]; then
            echo "Warning: your system is missing libraries that AAYKRA needs:"
            echo "$missing" | sed 's/^/    /'
            echo "Install them with your package manager, or AAYKRA will fail to start."
        fi
    fi

    # Setup ~/.local directories
    mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"

    # Link the binary
    ln -sf "$HOME/.local/aaykra$suffix.app/bin/aaykra" "$HOME/.local/bin/aaykra"

    # Install icons into the standard local icon theme paths. The desktop entry
    # references `Icon=aaykra`; a copy named after $appid is installed
    # too, because Wayland compositors look up the window/taskbar icon by the
    # window's app ID.
    icon_src_dir="$HOME/.local/aaykra$suffix.app/share/icons/hicolor"
    mkdir -p "$HOME/.local/share/icons/hicolor/512x512/apps" "$HOME/.local/share/icons/hicolor/1024x1024/apps"
    cp "$icon_src_dir/512x512/apps/aaykra.png" "$HOME/.local/share/icons/hicolor/512x512/apps/aaykra.png"
    cp "$icon_src_dir/512x512/apps/aaykra.png" "$HOME/.local/share/icons/hicolor/512x512/apps/${appid}.png"
    cp "$icon_src_dir/1024x1024/apps/aaykra.png" "$HOME/.local/share/icons/hicolor/1024x1024/apps/aaykra.png"
    cp "$icon_src_dir/1024x1024/apps/aaykra.png" "$HOME/.local/share/icons/hicolor/1024x1024/apps/${appid}.png"

    # Copy the .desktop file. The bundled entry uses the on-PATH `aaykra`
    # command and `aaykra` icon name; point both at the installed paths.
    desktop_file_path="$HOME/.local/share/applications/${appid}.desktop"
    src_dir="$HOME/.local/aaykra$suffix.app/share/applications"
    cp "$src_dir/${appid}.desktop" "${desktop_file_path}"
    sed -i "s|^Exec=aaykra|Exec=$HOME/.local/bin/aaykra|g" "${desktop_file_path}"
    sed -i "s|^Icon=aaykra|Icon=$HOME/.local/aaykra$suffix.app/share/icons/hicolor/512x512/apps/aaykra.png|g" "${desktop_file_path}"

    # Refresh the icon cache so the icon theme picks up the new entries.
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
    fi
}

macos() {
    echo "Downloading AAYKRA version: $ZED_VERSION"
    curl "$download_base/aaykra-$arch.dmg" > "$temp/aaykra-$arch.dmg"
    hdiutil attach -quiet "$temp/aaykra-$arch.dmg" -mountpoint "$temp/mount"
    app="$(cd "$temp/mount/"; echo *.app)"
    echo "Installing $app"
    if [ -d "/Applications/$app" ]; then
        echo "Removing existing $app"
        rm -rf "/Applications/$app"
    fi
    ditto "$temp/mount/$app" "/Applications/$app"
    hdiutil detach -quiet "$temp/mount"

    mkdir -p "$HOME/.local/bin"
    # Link the binary
    ln -sf "/Applications/$app/Contents/MacOS/cli" "$HOME/.local/bin/aaykra"
}

main "$@"
