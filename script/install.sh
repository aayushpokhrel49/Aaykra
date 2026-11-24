#!/usr/bin/env sh
set -eu

# Downloads a Aaykra release from GitHub and unpacks it into ~/.local/.
# Optional: AAYKRA_VERSION=1.2.3 picks a release, AAYKRA_BUNDLE_PATH installs a local tarball.
# The ZED_ names of these variables still work for older instructions.

main() {
    platform="$(uname -s)"
    arch="$(uname -m)"
    channel="${AAYKRA_CHANNEL:-${ZED_CHANNEL:-stable}}"
    repo="aayushpokhrel49/aaykra"
    version="${AAYKRA_VERSION:-${ZED_VERSION:-latest}}"
    bundle_path="${AAYKRA_BUNDLE_PATH:-${ZED_BUNDLE_PATH:-}}"
    if [ "$version" = "latest" ]; then
        download_base="https://github.com/$repo/releases/latest/download"
    else
        download_base="https://github.com/$repo/releases/download/v$version"
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
        echo "Aaykra has been installed. Run with 'aaykra'"
    else
        echo "To run Aaykra from your terminal, you must add ~/.local/bin to your PATH"
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

        echo "To run Aaykra now, '~/.local/bin/aaykra'"
    fi
}

linux() {
    if [ -n "$bundle_path" ]; then
        cp "$bundle_path" "$temp/aaykra-linux-$arch.tar.gz"
    else
        echo "Downloading Aaykra version: $version"
        curl "$download_base/aaykra-linux-$arch.tar.gz" > "$temp/aaykra-linux-$arch.tar.gz"
    fi

    suffix=""
    if [ "$channel" != "stable" ]; then
        suffix="-$channel"
    fi

    appid=""
    case "$channel" in
      stable)
        appid="com.aayushpokhrel.aaykra"
        ;;
      dev)
        appid="com.aayushpokhrel.aaykra-dev"
        ;;
      *)
        echo "Unknown release channel: ${channel}. Using stable app ID."
        appid="com.aayushpokhrel.aaykra"
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
            echo "Warning: your system is missing libraries that Aaykra needs:"
            echo "$missing" | sed 's/^/    /'
            echo "Install them with your package manager, or Aaykra will fail to start."
        fi
    fi

    # Setup ~/.local directories
    mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"

    # Link the binary
    ln -sf "$HOME/.local/aaykra$suffix.app/bin/aaykra" "$HOME/.local/bin/aaykra"

    # Copy .desktop file
    desktop_file_path="$HOME/.local/share/applications/${appid}.desktop"
    src_dir="$HOME/.local/aaykra$suffix.app/share/applications"
    cp "$src_dir/${appid}.desktop" "${desktop_file_path}"
    sed -i "s|Icon=aaykra|Icon=$HOME/.local/aaykra$suffix.app/share/icons/hicolor/512x512/apps/aaykra.png|g" "${desktop_file_path}"
    sed -i "s|Exec=aaykra|Exec=$HOME/.local/aaykra$suffix.app/bin/aaykra|g" "${desktop_file_path}"
}

macos() {
    echo "Downloading Aaykra version: $version"
    curl "$download_base/Aaykra-$arch.dmg" > "$temp/Aaykra-$arch.dmg"
    hdiutil attach -quiet "$temp/Aaykra-$arch.dmg" -mountpoint "$temp/mount"
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
