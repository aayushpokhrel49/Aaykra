## Install

All releases are available [here](https://github.com/aayushpokhrel49/Aaykra/releases/latest).

### macOS (Apple Silicon and Intel)

1. Download the installer for your Mac, open it, and drag Aaykra into your Applications folder.
    - [**Apple Silicon**](https://github.com/aayushpokhrel49/Aaykra/releases/latest/download/Aaykra-aarch64.dmg) (M1 and later)
    - [**Intel**](https://github.com/aayushpokhrel49/Aaykra/releases/latest/download/Aaykra-x86_64.dmg)

   Aaykra is not signed with an Apple Developer certificate yet, so macOS will block it the first time you open it.
2. Open Terminal and run:

   ```sh
   xattr -d com.apple.quarantine /Applications/Aaykra.app
   ```

> If you'd rather not use Terminal: open Aaykra once (you'll see a "Aaykra can't be opened" or "Apple could not verify" message), then go to **System Settings → Privacy & Security**, scroll down, and click **Open Anyway** next to Aaykra. Confirm with your password.

3. Open Aaykra normally.

#### Command line

To open files and folders from Terminal with `aaykra`, choose **Aaykra → Install CLI** from the menu bar. It adds `aaykra` to `/usr/local/bin` and may ask for your password.

If that fails, for example because your account isn't an admin, add it yourself without a password:

```sh
mkdir -p ~/.local/bin
ln -sf /Applications/Aaykra.app/Contents/MacOS/cli ~/.local/bin/aaykra
```

If `aaykra` still isn't found, add `~/.local/bin` to your `PATH` by adding this line to `~/.zshrc`, then open a new Terminal window:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

### Linux (x86-64 and AArch64)

Run this in a terminal:

```sh
curl -fsSL https://code.aayushpokhrel.com.np/install.sh | sh
```

It installs Aaykra into `~/.local`, adds the `aaykra` command, and adds Aaykra to your app menu with its icon. Run it again any time to reinstall. Aaykra keeps itself up to date after that.

#### Manual install

1. Download the tarball for your platform.
    - [**Linux x86-64**](https://github.com/aayushpokhrel49/Aaykra/releases/latest/download/aaykra-linux-x86_64.tar.gz)
    - [**Linux AArch64**](https://github.com/aayushpokhrel49/Aaykra/releases/latest/download/aaykra-linux-aarch64.tar.gz)

2. Unpack its contents into `~/.local`:

```sh
tar -xzf aaykra-linux-$(uname -m).tar.gz -C ~/.local
ln -sf ~/.local/aaykra.app/bin/aaykra ~/.local/bin/aaykra
```

Make sure `~/.local/bin` is on your `PATH`, then run `aaykra`. This doesn't add Aaykra to your app menu; the install command above does.

#### WSL2

On WSL2, Aaykra draws with the CPU by default because the WSL GPU driver isn't fully standards-compliant. To try the GPU instead, start Aaykra with:

```sh
WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER=1 aaykra
```

### Windows (x86-64)

Download the [installer](https://github.com/aayushpokhrel49/Aaykra/releases/latest/download/Aaykra-x86_64.exe) and click to run. The installer isn't code-signed, so Windows SmartScreen may warn you. Click **More info**, then **Run anyway**.
