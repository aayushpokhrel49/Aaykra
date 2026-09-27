#!/usr/bin/env sh
set -eu

# Uninstalls AAYKRA that was installed using the install.sh script

check_remaining_installations() {
    platform="$(uname -s)"
    if [ "$platform" = "Darwin" ]; then
        # Check for any AAYKRA variants in /Applications
        remaining=$(ls -d "/Applications/AAYKRA"*.app 2>/dev/null | wc -l)
        [ "$remaining" -eq 0 ]
    else
        # Check for any AAYKRA variants in ~/.local
        remaining=$(ls -d "$HOME/.local/aaykra"*.app 2>/dev/null | wc -l)
        [ "$remaining" -eq 0 ]
    fi
}

prompt_remove_preferences() {
    printf "Do you want to keep your Aaykra preferences? [Y/n] "
    read -r response
    case "$response" in
        [nN]|[nN][oO])
            rm -rf "$HOME/.config/aaykra"
            # Pre-rename config directory, still in use by installs that predate the rename.
            rm -rf "$HOME/.config/aayushi code"
            echo "Preferences removed."
            ;;
        *)
            echo "Preferences kept."
            ;;
    esac
}

main() {
    platform="$(uname -s)"
    channel="${ZED_CHANNEL:-stable}"

    if [ "$platform" = "Darwin" ]; then
        platform="macos"
    elif [ "$platform" = "Linux" ]; then
        platform="linux"
    else
        echo "Unsupported platform $platform"
        exit 1
    fi

    "$platform"

    echo "AAYKRA has been uninstalled"
}

linux() {
    suffix=""
    if [ "$channel" != "stable" ]; then
        suffix="-$channel"
    fi

    appid=""
    db_suffix="stable"
    case "$channel" in
      stable)
        appid="me.aayush.Aaykra"
        db_suffix="stable"
        ;;
      dev)
        appid="me.aayush.Aaykra-Dev"
        db_suffix="dev"
        ;;
      *)
        echo "Unknown release channel: ${channel}. Using stable app ID."
        appid="me.aayush.Aaykra"
        db_suffix="stable"
        ;;
    esac

    # Remove the app directory
    rm -rf "$HOME/.local/aaykra$suffix.app"

    # Remove the binary symlink
    rm -f "$HOME/.local/bin/aaykra"

    # Remove the .desktop file
    rm -f "$HOME/.local/share/applications/${appid}.desktop"

    # Remove the database directory for this channel
    rm -rf "$HOME/.local/share/aaykra/db/0-$db_suffix"
    # Same, for installs still using the pre-rename data directory.
    rm -rf "$HOME/.local/share/aayushi code/db/0-$db_suffix"

    # Remove socket file
    rm -f "$HOME/.local/share/aaykra/aaykra-$db_suffix.sock"
    rm -f "$HOME/.local/share/aayushi code/aaykra-$db_suffix.sock"
    rm -f "$HOME/.local/share/aayushi code/aayushicode-$db_suffix.sock"

    # Remove the entire Aaykra data directory if no installations remain
    if check_remaining_installations; then
        rm -rf "$HOME/.local/share/aaykra"
        # Pre-rename data directory, still in use by installs that predate the rename.
        rm -rf "$HOME/.local/share/aayushi code"
        prompt_remove_preferences
    fi

    rm -rf "$HOME/.aaykra_server"
}

macos() {
    app="AAYKRA.app"
    db_suffix="stable"
    app_id="me.aayush.Aaykra"
    case "$channel" in
      dev)
        app="AAYKRA Dev.app"
        db_suffix="dev"
        app_id="me.aayush.Aaykra-Dev"
        ;;
    esac

    # Remove the app bundle
    if [ -d "/Applications/$app" ]; then
        rm -rf "/Applications/$app"
    fi

    # Remove the binary symlink
    rm -f "$HOME/.local/bin/aaykra"

    # Remove the database directory for this channel
    rm -rf "$HOME/Library/Application Support/Aaykra/db/0-$db_suffix"
    # Same, for installs still using the pre-rename data directory.
    rm -rf "$HOME/Library/Application Support/Aayushi Code/db/0-$db_suffix"

    # Remove app-specific files and directories
    rm -rf "$HOME/Library/Application Support/com.apple.sharedfilelist/com.apple.LSSharedFileList.ApplicationRecentDocuments/$app_id.sfl"*
    rm -rf "$HOME/Library/Caches/$app_id"
    rm -rf "$HOME/Library/HTTPStorages/$app_id"
    rm -rf "$HOME/Library/Preferences/$app_id.plist"
    rm -rf "$HOME/Library/Saved Application State/$app_id.savedState"

    # Remove the entire Aaykra directory if no installations remain
    if check_remaining_installations; then
        rm -rf "$HOME/Library/Application Support/Aaykra"
        rm -rf "$HOME/Library/Logs/Aaykra"
        # Pre-rename directories, still in use by installs that predate the rename.
        rm -rf "$HOME/Library/Application Support/Aayushi Code"
        rm -rf "$HOME/Library/Logs/Aayushi Code"

        prompt_remove_preferences
    fi

    rm -rf "$HOME/.aaykra_server"
}

main "$@"