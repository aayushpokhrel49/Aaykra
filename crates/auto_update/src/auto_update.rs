use anyhow::{Context as _, Result};
use client::Client;
use db::kvp::KeyValueStore;
use futures_lite::StreamExt;
use gpui::{
    App, AppContext as _, AsyncApp, BackgroundExecutor, Context, Entity, Global, Task, TaskExt,
    Window, actions,
};
use http_client::{HttpClient, HttpClientWithUrl, HttpRequestExt};
use paths::remote_servers_dir;
use release_channel::ReleaseChannel;
use semver::Version;
use serde::{Deserialize, Serialize};
use settings::{RegisterSetting, Settings, SettingsStore};
use sha2::{Digest, Sha256};
use smol::fs::File;
use smol::{
    fs,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
};
use std::mem;
use std::{
    env::{
        self,
        consts::{ARCH, OS},
    },
    ffi::OsStr,
    ffi::OsString,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::Poll,
    time::{Duration, SystemTime},
};
use util::command::new_command;
use workspace::Workspace;

const SHOULD_SHOW_UPDATE_NOTIFICATION_KEY: &str = "auto-updater-should-show-updated-notification";

#[derive(Debug)]
struct MissingDependencyError(String);

impl std::fmt::Display for MissingDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for MissingDependencyError {}
const POLL_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
const REMOTE_SERVER_CACHE_LIMIT: usize = 5;
/// Retries for the update download, which frequently hits transient
/// connection failures on flaky networks. If a connect attempt times out but
/// the release exists, a later attempt succeeds.
const MAX_DOWNLOAD_ATTEMPTS: usize = 3;
const DOWNLOAD_RETRY_DELAYS: [Duration; MAX_DOWNLOAD_ATTEMPTS - 1] =
    [Duration::from_secs(2), Duration::from_secs(4)];
/// A connection that opens but then goes silent for this long has died on a
/// flaky path; abort the attempt so it can be retried instead of hanging.
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Total deadline for the release metadata request, so a stuck check can never
/// block the poll loop forever.
const RELEASE_FETCH_TIMEOUT: Duration = Duration::from_secs(15);
/// Retries for the release metadata request, which hits the same flaky network
/// path as the download itself.
const MAX_RELEASE_FETCH_ATTEMPTS: usize = 3;
const RELEASE_FETCH_RETRY_DELAYS: [Duration; MAX_RELEASE_FETCH_ATTEMPTS - 1] =
    [Duration::from_secs(2), Duration::from_secs(4)];
/// Cross-process guard so that two running instances (for example with
/// different `--user-data-dir`s) never download or install an update at the
/// same time. The OS releases the lock when the process exits, however it exits.
struct UpdateLock {
    _file: std::fs::File,
}

impl UpdateLock {
    fn path() -> PathBuf {
        #[cfg(test)]
        let file_name = format!("aaykra-auto-update-{}.lock", std::process::id());
        #[cfg(not(test))]
        let file_name = "aaykra-auto-update.lock".to_string();
        paths::temp_dir().join(file_name)
    }

    fn try_acquire() -> Result<Self> {
        let path = Self::path();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("opening update lock at {path:?}"))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(std::fs::TryLockError::WouldBlock) => {
                anyhow::bail!("another AAYKRA instance is already checking for updates")
            }
            Err(std::fs::TryLockError::Error(error)) => {
                Err(error).with_context(|| format!("locking update lock at {path:?}"))
            }
        }
    }
}

fn verify_digest(hasher: Sha256, expected_digest: Option<&str>, url: &str) -> Result<()> {
    let Some(expected_digest) = expected_digest else {
        log::warn!("no digest published for {url}, skipping download verification");
        return Ok(());
    };
    let actual_digest = format!("{:x}", hasher.finalize());
    anyhow::ensure!(
        actual_digest.eq_ignore_ascii_case(expected_digest),
        "download from {url} failed SHA-256 verification: expected {expected_digest}, got {actual_digest}"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_rsync_install_hint() -> &'static str {
    let os_release = match std::fs::read_to_string("/etc/os-release") {
        Ok(os_release) => os_release,
        Err(_) => return "Please install rsync using your package manager",
    };

    let mut distribution_ids = Vec::new();
    for line in os_release.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("ID=") {
            distribution_ids.push(value.trim_matches('"').to_ascii_lowercase());
        } else if let Some(value) = trimmed.strip_prefix("ID_LIKE=") {
            for id in value.trim_matches('"').split_whitespace() {
                distribution_ids.push(id.to_ascii_lowercase());
            }
        }
    }

    let package_manager_hint = if distribution_ids
        .iter()
        .any(|distribution_id| distribution_id == "arch")
    {
        Some("Install it with: sudo pacman -S rsync")
    } else if distribution_ids
        .iter()
        .any(|distribution_id| distribution_id == "debian" || distribution_id == "ubuntu")
    {
        Some("Install it with: sudo apt install rsync")
    } else if distribution_ids.iter().any(|distribution_id| {
        distribution_id == "fedora"
            || distribution_id == "rhel"
            || distribution_id == "centos"
            || distribution_id == "rocky"
            || distribution_id == "almalinux"
    }) {
        Some("Install it with: sudo dnf install rsync")
    } else if distribution_ids
        .iter()
        .any(|distribution_id| distribution_id == "nixos")
    {
        Some("Install pkgs.rsync from nixpkgs")
    } else {
        None
    };

    package_manager_hint.unwrap_or("Please install rsync using your package manager")
}

actions!(
    auto_update,
    [
        /// Checks for available updates.
        Check,
        /// Dismisses the update error message.
        DismissMessage,
        /// Opens the release notes for the current version in a browser.
        ViewReleaseNotes,
    ]
);

#[derive(Clone, Debug)]
pub enum AutoUpdateStatus {
    Idle,
    Checking,
    Downloading {
        version: Version,
        /// Download progress as a fraction in the range `0.0..=1.0`, or `None`
        /// when the total download size is not yet known.
        progress: Option<f32>,
    },
    Installing {
        version: Version,
    },
    Updated {
        version: Version,
    },
    Errored {
        error: Arc<anyhow::Error>,
    },
}

impl PartialEq for AutoUpdateStatus {
    // `progress` is deliberately not compared: two `Downloading` statuses for
    // the same version are equal regardless of how far the download is.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AutoUpdateStatus::Idle, AutoUpdateStatus::Idle) => true,
            (AutoUpdateStatus::Checking, AutoUpdateStatus::Checking) => true,
            (
                AutoUpdateStatus::Downloading { version: v1, .. },
                AutoUpdateStatus::Downloading { version: v2, .. },
            ) => v1 == v2,
            (
                AutoUpdateStatus::Installing { version: v1 },
                AutoUpdateStatus::Installing { version: v2 },
            ) => v1 == v2,
            (
                AutoUpdateStatus::Updated { version: v1 },
                AutoUpdateStatus::Updated { version: v2 },
            ) => v1 == v2,
            (AutoUpdateStatus::Errored { error: e1 }, AutoUpdateStatus::Errored { error: e2 }) => {
                e1.to_string() == e2.to_string()
            }
            _ => false,
        }
    }
}

impl AutoUpdateStatus {
    pub fn is_updated(&self) -> bool {
        matches!(self, Self::Updated { .. })
    }
}

pub struct AutoUpdater {
    status: AutoUpdateStatus,
    current_version: Version,
    client: Arc<Client>,
    pending_poll: Option<Task<Option<()>>>,
    quit_subscription: Option<gpui::Subscription>,
    update_check_type: UpdateCheckType,
    _wake_subscription: gpui::Subscription,
    dismissed_status: Option<AutoUpdateStatus>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ReleaseAsset {
    pub version: String,
    pub url: String,
    /// Hex SHA-256 of the asset, when the release publishes one.
    pub digest: Option<String>,
}

const GITHUB_RELEASES_API_URL: &str =
    "https://api.github.com/repos/aayushpokhrel49/Aaykra/releases";

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Clone, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

/// All release asset names the updater recognizes for `asset` on `os`/`arch`,
/// most preferred first.
///
/// The primary name follows the current `aaykra-*` scheme. Legacy names from
/// before the rebrand are kept as fallbacks: releases still publish copies under
/// the old names so already-installed binaries built before the rebrand (which
/// only know the legacy names) can self-update, and matching them first lets a
/// future release drop the legacy copies without breaking us.
fn github_asset_names(asset: &str, os: &str, arch: &str) -> Result<Vec<String>> {
    let mut names = vec![match (asset, os) {
        ("zed", "macos") => format!("aaykra-{arch}.dmg"),
        ("zed", "linux") => format!("aaykra-linux-{arch}.tar.gz"),
        ("zed", "windows") => format!("aaykra-{arch}.exe"),
        ("aaykra-remote-server", _) => format!("aaykra-remote-server-{os}-{arch}.gz"),
        _ => anyhow::bail!("no release asset for {asset} on {os}"),
    }];
    match (asset, os) {
        ("zed", "macos") => names.push(format!("Aaykra-{arch}.dmg")),
        ("zed", "linux") => names.push(format!("aaykra-linux-{arch}.tar.gz")),
        ("zed", "windows") => names.push(format!("Aaykra-{arch}.exe")),
        ("aaykra-remote-server", _) => {
            names.push(format!("aaykra-remote-server-{os}-{arch}.gz"));
        }
        _ => {}
    }
    Ok(names)
}

struct MacOsUnmounter<'a> {
    mount_path: PathBuf,
    background_executor: &'a BackgroundExecutor,
}

impl MacOsUnmounter<'_> {
    /// Unmounts the disk image and waits for completion. This must happen
    /// before the `InstallerDir` is dropped: deleting the temp dir while the
    /// image is still mounted inside it fails silently and leaks the
    /// directory (and the downloaded DMG) in the system temp dir.
    async fn unmount(mut self) {
        let mount_path = mem::take(&mut self.mount_path);
        unmount_disk_image(&mount_path).await;
    }
}

impl Drop for MacOsUnmounter<'_> {
    fn drop(&mut self) {
        let mount_path = mem::take(&mut self.mount_path);
        // Safety net for early exits and cancellation; the happy path calls
        // `unmount`, which leaves the path empty.
        if mount_path.as_os_str().is_empty() {
            return;
        }
        self.background_executor
            .spawn(async move { unmount_disk_image(&mount_path).await })
            .detach();
    }
}

async fn unmount_disk_image(mount_path: &Path) {
    let unmount_output = new_command("hdiutil")
        .args(["detach", "-force"])
        .arg(mount_path)
        .output()
        .await;
    match unmount_output {
        Ok(output) if output.status.success() => {
            log::info!("Successfully unmounted the disk image");
        }
        Ok(output) => {
            log::error!(
                "Failed to unmount disk image: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(error) => {
            log::error!("Error while trying to unmount disk image: {:?}", error);
        }
    }
}

#[derive(Clone, Copy, Debug, RegisterSetting)]
struct AutoUpdateSetting(bool);

/// Whether or not to automatically check for updates.
///
/// Default: true
impl Settings for AutoUpdateSetting {
    fn from_settings(content: &settings::SettingsContent) -> Self {
        Self(content.auto_update.unwrap())
    }
}

#[derive(Default)]
struct GlobalAutoUpdate(Option<Entity<AutoUpdater>>);

impl Global for GlobalAutoUpdate {}

pub fn init(client: Arc<Client>, cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _window, _cx| {
        workspace.register_action(|_, action, window, cx| check(action, window, cx));

        workspace.register_action(|_, action, _, cx| {
            view_release_notes(action, cx);
        });
    })
    .detach();

    let version = release_channel::AppVersion::global(cx);
    let auto_updater = cx.new(|cx| {
        let updater = AutoUpdater::new(version, client, cx);

        let poll_for_updates = ReleaseChannel::try_global(cx)
            .map(|channel| channel.poll_for_updates())
            .unwrap_or(false);

        if option_env!("ZED_UPDATE_EXPLANATION").is_none()
            && env::var("ZED_UPDATE_EXPLANATION").is_err()
            && poll_for_updates
        {
            let mut update_subscription = AutoUpdateSetting::get_global(cx)
                .0
                .then(|| updater.start_polling(cx));

            cx.observe_global::<SettingsStore>(move |updater: &mut AutoUpdater, cx| {
                if AutoUpdateSetting::get_global(cx).0 {
                    if update_subscription.is_none() {
                        update_subscription = Some(updater.start_polling(cx))
                    }
                } else {
                    update_subscription.take();
                }
            })
            .detach();
        }

        updater
    });
    cx.set_global(GlobalAutoUpdate(Some(auto_updater)));
}

pub fn check(_: &Check, window: &mut Window, cx: &mut App) {
    if let Some(message) = option_env!("ZED_UPDATE_EXPLANATION")
        .map(ToOwned::to_owned)
        .or_else(|| env::var("ZED_UPDATE_EXPLANATION").ok())
    {
        drop(window.prompt(
            gpui::PromptLevel::Info,
            "AAYKRA was installed via a package manager.",
            Some(&message),
            &["OK"],
            cx,
        ));
        return;
    }

    if !ReleaseChannel::try_global(cx)
        .map(|channel| channel.poll_for_updates())
        .unwrap_or(false)
    {
        return;
    }

    if let Some(updater) = AutoUpdater::get(cx) {
        updater.update(cx, |updater, cx| updater.poll(UpdateCheckType::Manual, cx));
    } else {
        drop(window.prompt(
            gpui::PromptLevel::Info,
            "Could not check for updates",
            Some("Auto-updates disabled for non-bundled app."),
            &["OK"],
            cx,
        ));
    }
}

pub fn release_notes_url(cx: &mut App) -> Option<String> {
    let release_channel = ReleaseChannel::try_global(cx)?;
    let url = match release_channel {
        ReleaseChannel::Stable => {
            let auto_updater = AutoUpdater::get(cx)?;
            let auto_updater = auto_updater.read(cx);
            let mut current_version = auto_updater.current_version.clone();
            current_version.pre = semver::Prerelease::EMPTY;
            current_version.build = semver::BuildMetadata::EMPTY;
            format!("https://github.com/aayushpokhrel49/Aaykra/releases/tag/v{current_version}")
        }
        ReleaseChannel::Dev => {
            "https://github.com/aayushpokhrel49/Aaykra/commits/main/".to_string()
        }
    };
    Some(url)
}

pub fn view_release_notes(_: &ViewReleaseNotes, cx: &mut App) -> Option<()> {
    let url = release_notes_url(cx)?;
    cx.open_url(&url);
    None
}

#[cfg(not(target_os = "windows"))]
const INSTALLER_DIR_PREFIX: &str = "aaykra-auto-update";

#[cfg(not(target_os = "windows"))]
struct InstallerDir(tempfile::TempDir);

#[cfg(not(target_os = "windows"))]
impl InstallerDir {
    async fn new() -> Result<Self> {
        Ok(Self(
            tempfile::Builder::new()
                .prefix(INSTALLER_DIR_PREFIX)
                .tempdir()?,
        ))
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

#[cfg(target_os = "windows")]
struct InstallerDir(PathBuf);

#[cfg(target_os = "windows")]
impl InstallerDir {
    async fn new() -> Result<Self> {
        let installer_dir = std::env::current_exe()?
            .parent()
            .context("No parent dir for Aaykra.exe")?
            .join("updates");
        if smol::fs::metadata(&installer_dir).await.is_ok() {
            smol::fs::remove_dir_all(&installer_dir).await?;
        }
        smol::fs::create_dir(&installer_dir).await?;
        Ok(Self(installer_dir))
    }

    fn path(&self) -> &Path {
        self.0.as_path()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpdateCheckType {
    Automatic,
    Manual,
}

impl UpdateCheckType {
    pub fn is_manual(self) -> bool {
        self == Self::Manual
    }
}

impl AutoUpdater {
    pub fn get(cx: &mut App) -> Option<Entity<Self>> {
        cx.default_global::<GlobalAutoUpdate>().0.clone()
    }

    fn new(current_version: Version, client: Arc<Client>, cx: &mut Context<Self>) -> Self {
        // On windows, executable files cannot be overwritten while they are
        // running, so we must wait to overwrite the application until quitting
        // or restarting. When quitting the app, we spawn the auto update helper
        // to finish the auto update process after Zed exits. When restarting
        // the app after an update, we use `set_restart_path` to run the auto
        // update helper instead of the app, so that it can overwrite the app
        // and then spawn the new binary.
        #[cfg(target_os = "windows")]
        let quit_subscription = Some(cx.on_app_quit(|_, _| finalize_auto_update_on_quit()));
        #[cfg(not(target_os = "windows"))]
        let quit_subscription = None;

        cx.on_app_restart(|this, _| {
            this.quit_subscription.take();
        })
        .detach();

        // A download or check that was in flight when the machine went to sleep
        // is almost certainly riding a TCP connection that silently died during
        // suspend, so it would otherwise appear to stall indefinitely.
        let wake_subscription = cx.on_system_wake({
            let this = cx.entity().downgrade();
            move |cx| {
                this.update(cx, |this, cx| this.restart_after_wake(cx)).ok();
            }
        });

        Self {
            status: AutoUpdateStatus::Idle,
            current_version,
            client,
            pending_poll: None,
            quit_subscription,
            update_check_type: UpdateCheckType::Automatic,
            _wake_subscription: wake_subscription,
            dismissed_status: None,
        }
    }

    fn restart_after_wake(&mut self, cx: &mut Context<Self>) {
        // Only network phases can be safely restarted. `Installing` is a local
        // operation (mounting a dmg, rsync, etc.) that must not be interrupted.
        if !matches!(
            self.status,
            AutoUpdateStatus::Checking | AutoUpdateStatus::Downloading { .. }
        ) {
            return;
        }

        let check_type = self.update_check_type;
        self.pending_poll.take();
        self.status = AutoUpdateStatus::Idle;
        self.poll(check_type, cx);
    }

    pub fn start_polling(&self, cx: &mut Context<Self>) -> Task<Result<()>> {
        let poll_interval = POLL_INTERVAL;

        cx.spawn(async move |this, cx| {
            if cfg!(target_os = "windows") {
                use util::ResultExt;

                cleanup_windows()
                    .await
                    .context("failed to cleanup old directories")
                    .log_err();
            }

            #[cfg(all(not(target_os = "windows"), not(test)))]
            cx.background_spawn(cleanup_stale_installer_dirs()).detach();

            loop {
                this.update(cx, |this, cx| this.poll(UpdateCheckType::Automatic, cx))?;
                cx.background_executor().timer(poll_interval).await;
            }
        })
    }

    pub fn update_check_type(&self) -> UpdateCheckType {
        self.update_check_type
    }

    pub fn poll(&mut self, check_type: UpdateCheckType, cx: &mut Context<Self>) {
        if check_type.is_manual() {
            self.dismissed_status = None;
        }
        if self.pending_poll.is_some() {
            if self.update_check_type == UpdateCheckType::Automatic {
                self.update_check_type = check_type;
                cx.notify();
            }
            return;
        }
        self.update_check_type = check_type;

        cx.notify();

        self.pending_poll = Some(cx.spawn(async move |this, cx| {
            let result = Self::update(this.upgrade()?, cx).await;
            this.update(cx, |this, cx| {
                this.pending_poll = None;
                if let Err(error) = result {
                    let is_missing_dependency =
                        error.downcast_ref::<MissingDependencyError>().is_some();
                    this.status = match check_type {
                        UpdateCheckType::Automatic if is_missing_dependency => {
                            log::warn!("auto-update: {}", error);
                            AutoUpdateStatus::Errored {
                                error: Arc::new(error),
                            }
                        }
                        // Be quiet if the check was automated (e.g. when offline)
                        UpdateCheckType::Automatic => {
                            log::info!("auto-update check failed: error:{:?}", error);
                            AutoUpdateStatus::Idle
                        }
                        UpdateCheckType::Manual => {
                            log::error!("auto-update failed: error:{:?}", error);
                            AutoUpdateStatus::Errored {
                                error: Arc::new(error),
                            }
                        }
                    };

                    cx.notify();
                }
            })
            .ok()
        }));
    }

    pub fn current_version(&self) -> Version {
        self.current_version.clone()
    }

    pub fn status(&self) -> AutoUpdateStatus {
        self.status.clone()
    }

    pub fn dismissed_status(&self) -> Option<AutoUpdateStatus> {
        self.dismissed_status.clone()
    }

    pub fn dismiss_status(&mut self, status: AutoUpdateStatus, cx: &mut Context<Self>) {
        self.dismissed_status = Some(status);
        cx.notify();
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) -> bool {
        if let AutoUpdateStatus::Idle = self.status {
            return false;
        }
        self.status = AutoUpdateStatus::Idle;
        cx.notify();
        true
    }

    // If you are packaging Zed and need to override the place it downloads SSH remotes from,
    // you can override this function. You should also update get_remote_server_release_url to return
    // Ok(None).
    pub async fn download_remote_server_release(
        release_channel: ReleaseChannel,
        version: Option<Version>,
        os: &str,
        arch: &str,
        set_status: impl Fn(&str, &mut AsyncApp) + Send + 'static,
        cx: &mut AsyncApp,
    ) -> Result<PathBuf> {
        let this = cx.update(|cx| {
            cx.default_global::<GlobalAutoUpdate>()
                .0
                .clone()
                .context("auto-update not initialized")
        })?;

        set_status("Fetching remote server release", cx);
        let release = Self::get_release_asset(
            &this,
            release_channel,
            version,
            "aaykra-remote-server",
            os,
            arch,
            cx,
        )
        .await?;

        let servers_dir = paths::remote_servers_dir();
        let channel_dir = servers_dir.join(release_channel.dev_name());
        let platform_dir = channel_dir.join(format!("{}-{}", os, arch));
        let version_path = platform_dir.join(format!("{}.gz", release.version));
        smol::fs::create_dir_all(&platform_dir).await.ok();

        let client = this.read_with(cx, |this, _| this.client.http_client());

        if smol::fs::metadata(&version_path).await.is_err() {
            log::info!(
                "downloading aaykra-remote-server {os} {arch} version {}",
                release.version
            );
            set_status("Downloading remote server", cx);
            download_remote_server_binary(&version_path, release, client).await?;
        }

        if let Err(error) =
            cleanup_remote_server_cache(&platform_dir, &version_path, REMOTE_SERVER_CACHE_LIMIT)
                .await
        {
            log::warn!(
                "Failed to clean up remote server cache in {:?}: {error:#}",
                platform_dir
            );
        }

        Ok(version_path)
    }

    pub async fn get_remote_server_release_url(
        channel: ReleaseChannel,
        version: Option<Version>,
        os: &str,
        arch: &str,
        cx: &mut AsyncApp,
    ) -> Result<Option<String>> {
        let this = cx.update(|cx| {
            cx.default_global::<GlobalAutoUpdate>()
                .0
                .clone()
                .context("auto-update not initialized")
        })?;

        let release = Self::get_release_asset(
            &this,
            channel,
            version,
            "aaykra-remote-server",
            os,
            arch,
            cx,
        )
        .await?;

        Ok(Some(release.url))
    }

    async fn fetch_release_once(
        this: &Entity<Self>,
        version: Option<Version>,
        cx: &mut AsyncApp,
    ) -> Result<(Version, Vec<GitHubReleaseAsset>)> {
        let client = this.read_with(cx, |this, _| this.client.clone());
        let http_client = client.http_client();

        let url = match version {
            Some(mut version) => {
                version.pre = semver::Prerelease::EMPTY;
                version.build = semver::BuildMetadata::EMPTY;
                format!("{GITHUB_RELEASES_API_URL}/tags/v{version}")
            }
            None => format!("{GITHUB_RELEASES_API_URL}/latest"),
        };

        let request = http_client::http::request::Builder::new()
            .uri(url.as_str())
            .follow_redirects(http_client::RedirectPolicy::FollowAll)
            .timeout(RELEASE_FETCH_TIMEOUT)
            .body(http_client::AsyncBody::default())
            .context("failed to build release fetch request")?;
        let mut response = http_client.send(request).await?;
        let mut body = Vec::new();
        response.body_mut().read_to_end(&mut body).await?;

        anyhow::ensure!(
            response.status().is_success(),
            "failed to fetch release: {:?}",
            String::from_utf8_lossy(&body),
        );

        let release: GitHubRelease =
            serde_json::from_slice(body.as_slice()).with_context(|| {
                format!(
                    "error deserializing release {:?}",
                    String::from_utf8_lossy(&body),
                )
            })?;

        let version = release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name)
            .parse::<Version>()?;

        let assets = release
            .assets
            .into_iter()
            .map(|mut asset| {
                if let Some(digest) = &asset.digest
                    && let Some(stripped) = digest.strip_prefix("sha256:")
                {
                    asset.digest = Some(stripped.to_owned());
                }
                asset
            })
            .collect::<Vec<_>>();

        Ok((version, assets))
    }

    /// Fetches the latest release metadata, retrying transient failures with a
    /// short backoff so a flaky network path (the same one the download uses)
    /// doesn't fail a single, unrecoverable request.
    async fn fetch_release(
        this: &Entity<Self>,
        version: Option<Version>,
        cx: &mut AsyncApp,
    ) -> Result<(Version, Vec<GitHubReleaseAsset>)> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match Self::fetch_release_once(this, version.clone(), cx).await {
                Ok(release) => return Ok(release),
                Err(error) if attempt < MAX_RELEASE_FETCH_ATTEMPTS => {
                    let retry_delay = RELEASE_FETCH_RETRY_DELAYS[attempt - 1];
                    log::warn!(
                        "auto-update release fetch attempt {attempt} failed; retrying in {:?}: error:{error:#}",
                        retry_delay
                    );
                    cx.background_executor().timer(retry_delay).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn get_release_asset(
        this: &Entity<Self>,
        _release_channel: ReleaseChannel,
        version: Option<Version>,
        asset: &str,
        os: &str,
        arch: &str,
        cx: &mut AsyncApp,
    ) -> Result<ReleaseAsset> {
        let (version, assets) = Self::fetch_release(this, version, cx).await?;
        let asset_names = github_asset_names(asset, os, arch)?;
        let asset = asset_names
            .iter()
            .find_map(|asset_name| {
                assets
                    .iter()
                    .find(|release_asset| &release_asset.name == asset_name)
                    .cloned()
            })
            .with_context(|| format!("release v{version} has no asset named {asset_names:?}"))?;

        Ok(ReleaseAsset {
            version: version.to_string(),
            url: asset.browser_download_url,
            digest: asset.digest,
        })
    }

    async fn update(this: Entity<Self>, cx: &mut AsyncApp) -> Result<()> {
        let (client, installed_version, previous_status) = this.read_with(cx, |this, _| {
            (
                this.client.http_client(),
                this.current_version.clone(),
                this.status.clone(),
            )
        });

        Self::check_dependencies()?;

        let _update_lock = UpdateLock::try_acquire()?;

        this.update(cx, |this, cx| {
            this.status = AutoUpdateStatus::Checking;
            log::info!("Auto Update: checking for updates");
            cx.notify();
        });

        let (fetched_version, release_assets) = Self::fetch_release(&this, None, cx).await?;
        let newer_version = Self::check_if_fetched_version_is_newer(
            installed_version,
            fetched_version.to_string(),
            previous_status.clone(),
        )?;

        let Some(newer_version) = newer_version else {
            this.update(cx, |this, cx| {
                let status = match previous_status {
                    AutoUpdateStatus::Updated { .. } => previous_status,
                    _ => AutoUpdateStatus::Idle,
                };
                this.status = status;
                cx.notify();
            });
            return Ok(());
        };

        // Only resolve the platform asset once we know there is an update to
        // install, so that a release which is missing the asset for this
        // platform does not break the "already up to date" check.
        let asset_names = github_asset_names("zed", OS, ARCH)?;
        let asset = asset_names
            .iter()
            .find_map(|asset_name| {
                release_assets
                    .iter()
                    .find(|release_asset| &release_asset.name == asset_name)
                    .cloned()
            })
            .with_context(|| {
                format!("release v{fetched_version} has no asset named {asset_names:?}")
            })?;
        let fetched_release_data = ReleaseAsset {
            version: fetched_version.to_string(),
            url: asset.browser_download_url,
            digest: asset.digest,
        };

        this.update(cx, |this, cx| {
            this.status = AutoUpdateStatus::Downloading {
                version: newer_version.clone(),
                progress: None,
            };
            cx.notify();
        });

        let installer_dir = InstallerDir::new()
            .await
            .context("Failed to create installer dir")?;
        let target_path = Self::target_path(&installer_dir).await?;
        let progress_entity = this.clone();
        let mut progress_cx = cx.clone();
        download_release(
            &target_path,
            fetched_release_data,
            client,
            cx.background_executor().clone(),
            move |progress| {
                progress_entity.update(&mut progress_cx, |this, cx| {
                    if let AutoUpdateStatus::Downloading {
                        progress: current_progress,
                        ..
                    } = &mut this.status
                    {
                        *current_progress = progress;
                        cx.notify();
                    }
                });
            },
        )
        .await
        .with_context(|| format!("Failed to download update to {}", target_path.display()))?;

        this.update(cx, |this, cx| {
            this.status = AutoUpdateStatus::Installing {
                version: newer_version.clone(),
            };
            cx.notify();
        });

        #[cfg(test)]
        let install_result = match cx
            .try_read_global::<tests::InstallOverride, _>(|g, _| g.0.clone())
            .map(|test_install| test_install(&target_path, cx))
        {
            Some(result) => result,
            None => return Ok(()),
        };

        #[cfg(not(test))]
        let install_result = {
            let running_app_path = cx.update(|cx| cx.app_path())?;
            let background_executor = cx.background_executor().clone();
            let channel = cx.update(|cx| ReleaseChannel::global(cx).dev_name());
            cx.background_spawn(Self::install_release(
                installer_dir,
                target_path.clone(),
                running_app_path,
                channel,
                background_executor,
            ))
            .await
        };
        let new_binary_path = install_result
            .with_context(|| format!("Failed to install update at: {}", target_path.display()))?;
        if let Some(new_binary_path) = new_binary_path {
            cx.update(|cx| cx.set_restart_path(new_binary_path));
        }

        this.update(cx, |this, cx| {
            this.record_installed_update(&newer_version, cx)
                .detach_and_log_err(cx);
            this.status = AutoUpdateStatus::Updated {
                version: newer_version,
            };
            cx.notify();
        });
        Ok(())
    }

    fn check_if_fetched_version_is_newer(
        installed_version: Version,
        fetched_version: String,
        status: AutoUpdateStatus,
    ) -> Result<Option<Version>> {
        let fetched_version = fetched_version.parse::<Version>()?;
        let current_version = if let AutoUpdateStatus::Updated { version } = status {
            version
        } else {
            installed_version
        };
        Ok(Self::check_if_fetched_version_is_newer_non_nightly(
            current_version,
            fetched_version,
        ))
    }

    fn check_dependencies() -> Result<()> {
        #[cfg(target_os = "linux")]
        if which::which("rsync").is_err() {
            let install_hint = linux_rsync_install_hint();
            return Err(MissingDependencyError(format!(
                "rsync is required for auto-updates but is not installed. {install_hint}"
            ))
            .into());
        }

        #[cfg(target_os = "macos")]
        anyhow::ensure!(
            which::which("rsync").is_ok(),
            "Could not auto-update because the required rsync utility was not found."
        );

        Ok(())
    }

    async fn target_path(installer_dir: &InstallerDir) -> Result<PathBuf> {
        let filename = match OS {
            "macos" => anyhow::Ok("aaykra.dmg"),
            "linux" => Ok("aaykra.tar.gz"),
            "windows" => Ok("aaykra.exe"),
            unsupported_os => anyhow::bail!("not supported: {unsupported_os}"),
        }?;
        Ok(installer_dir.path().join(filename))
    }

    #[cfg_attr(test, allow(dead_code))]
    async fn install_release(
        installer_dir: InstallerDir,
        target_path: PathBuf,
        running_app_path: PathBuf,
        channel: &str,
        background_executor: BackgroundExecutor,
    ) -> Result<Option<PathBuf>> {
        match OS {
            "macos" => {
                install_release_macos(
                    &installer_dir,
                    &target_path,
                    running_app_path,
                    &background_executor,
                )
                .await
            }
            "linux" => {
                install_release_linux(&installer_dir, &target_path, channel, running_app_path).await
            }
            "windows" => install_release_windows(&target_path).await,
            unsupported_os => anyhow::bail!("not supported: {unsupported_os}"),
        }
    }

    fn check_if_fetched_version_is_newer_non_nightly(
        mut installed_version: Version,
        fetched_version: Version,
    ) -> Option<Version> {
        // For non-nightly releases, ignore build and pre-release fields as they're not provided by our endpoints right now.
        installed_version.pre = semver::Prerelease::EMPTY;
        installed_version.build = semver::BuildMetadata::EMPTY;
        (fetched_version > installed_version).then_some(fetched_version)
    }

    pub fn set_should_show_update_notification(
        &self,
        should_show: bool,
        cx: &App,
    ) -> Task<Result<()>> {
        let kvp = KeyValueStore::global(cx);
        cx.background_spawn(async move {
            if should_show {
                kvp.write_kvp(
                    SHOULD_SHOW_UPDATE_NOTIFICATION_KEY.to_string(),
                    "".to_string(),
                )
                .await?;
            } else {
                kvp.delete_kvp(SHOULD_SHOW_UPDATE_NOTIFICATION_KEY.to_string())
                    .await?;
            }
            Ok(())
        })
    }

    /// Records that an update to `version` was staged, so the "updated"
    /// notification can be shown once that version is actually running.
    fn record_installed_update(&self, version: &Version, cx: &App) -> Task<Result<()>> {
        let kvp = KeyValueStore::global(cx);
        let version = version.to_string();
        cx.background_spawn(async move {
            kvp.write_kvp(SHOULD_SHOW_UPDATE_NOTIFICATION_KEY.to_string(), version)
                .await
        })
    }

    pub fn should_show_update_notification(&self, cx: &App) -> Task<Result<bool>> {
        let kvp = KeyValueStore::global(cx);
        let mut running_version = self.current_version.clone();
        running_version.pre = semver::Prerelease::EMPTY;
        running_version.build = semver::BuildMetadata::EMPTY;
        cx.background_spawn(async move {
            let Some(recorded_version) = kvp.read_kvp(SHOULD_SHOW_UPDATE_NOTIFICATION_KEY)? else {
                return Ok(false);
            };
            // An empty value was written by older builds that did not record
            // the version. A mismatch means the staged update was rolled back
            // or never applied, so the notification would be wrong.
            if recorded_version.is_empty() || recorded_version == running_version.to_string() {
                return Ok(true);
            }
            log::info!(
                "Auto Update: recorded update to {recorded_version} but {running_version} is running, not showing update notification"
            );
            kvp.delete_kvp(SHOULD_SHOW_UPDATE_NOTIFICATION_KEY.to_string())
                .await?;
            Ok(false)
        })
    }
}

async fn download_remote_server_binary(
    target_path: &PathBuf,
    release: ReleaseAsset,
    client: Arc<HttpClientWithUrl>,
) -> Result<()> {
    let temp = tempfile::Builder::new().tempfile_in(remote_servers_dir())?;
    let mut temp_file = File::create(&temp).await?;

    let mut response = client.get(&release.url, Default::default(), true).await?;
    anyhow::ensure!(
        response.status().is_success(),
        "failed to download remote server release: {:?}",
        response.status()
    );
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    let body = response.body_mut();
    loop {
        let bytes_read = body.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        temp_file.write_all(&buffer[..bytes_read]).await?;
        hasher.update(&buffer[..bytes_read]);
    }
    temp_file.flush().await?;
    verify_digest(hasher, release.digest.as_deref(), &release.url)?;
    smol::fs::rename(&temp, &target_path).await?;

    Ok(())
}

async fn cleanup_remote_server_cache(
    platform_dir: &Path,
    keep_path: &Path,
    limit: usize,
) -> Result<()> {
    if limit == 0 {
        return Ok(());
    }

    let mut entries = smol::fs::read_dir(platform_dir).await?;
    let now = SystemTime::now();
    let mut candidates = Vec::new();

    while let Some(entry) = entries.next().await {
        let entry = entry?;
        let path = entry.path();
        if path.extension() != Some(OsStr::new("gz")) {
            continue;
        }

        let mtime = if path == keep_path {
            now
        } else {
            smol::fs::metadata(&path)
                .await
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        };

        candidates.push((path, mtime));
    }

    if candidates.len() <= limit {
        return Ok(());
    }

    candidates.sort_by(|(path_a, time_a), (path_b, time_b)| {
        time_b.cmp(time_a).then_with(|| path_a.cmp(path_b))
    });

    for (index, (path, _)) in candidates.into_iter().enumerate() {
        if index < limit || path == keep_path {
            continue;
        }

        if let Err(error) = smol::fs::remove_file(&path).await {
            log::warn!(
                "Failed to remove old remote server archive {:?}: {}",
                path,
                error
            );
        }
    }

    Ok(())
}

async fn download_release(
    target_path: &Path,
    release: ReleaseAsset,
    client: Arc<HttpClientWithUrl>,
    executor: BackgroundExecutor,
    mut on_progress: impl FnMut(Option<f32>),
) -> Result<()> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match download_release_once(target_path, &release, &client, &executor, &mut on_progress)
            .await
        {
            Ok(()) => return Ok(()),
            Err(error) if attempt < MAX_DOWNLOAD_ATTEMPTS => {
                let retry_delay = DOWNLOAD_RETRY_DELAYS[attempt - 1];
                log::warn!(
                    "auto-update download attempt {attempt} failed; retrying in {:?}: error:{error:#}",
                    retry_delay
                );
                executor.timer(retry_delay).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn download_release_once(
    target_path: &Path,
    release: &ReleaseAsset,
    client: &HttpClientWithUrl,
    executor: &BackgroundExecutor,
    on_progress: &mut impl FnMut(Option<f32>),
) -> Result<()> {
    let mut target_file = File::create(&target_path).await?;

    let mut response = client.get(&release.url, Default::default(), true).await?;
    anyhow::ensure!(
        response.status().is_success(),
        "failed to download update: {:?}",
        response.status()
    );

    let total_bytes = response
        .headers()
        .get(http_client::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|total_bytes| *total_bytes > 0);

    let mut downloaded_bytes: u64 = 0;
    let mut last_reported_percent: Option<u8> = None;
    let mut buffer = [0u8; 8192];
    let mut hasher = Sha256::new();
    let body = response.body_mut();
    loop {
        let bytes_read = read_body_chunk(body, &mut buffer, executor).await?;
        if bytes_read == 0 {
            break;
        }
        target_file.write_all(&buffer[..bytes_read]).await?;
        hasher.update(&buffer[..bytes_read]);
        downloaded_bytes += bytes_read as u64;

        if let Some(total_bytes) = total_bytes {
            let fraction = (downloaded_bytes as f32 / total_bytes as f32).clamp(0.0, 1.0);
            // Only report when the whole-number percentage changes to avoid notifying the UI on every chunk.
            let percent = (fraction * 100.0) as u8;
            if last_reported_percent != Some(percent) {
                last_reported_percent = Some(percent);
                on_progress(Some(fraction));
            }
        }
    }
    target_file.flush().await?;
    verify_digest(hasher, release.digest.as_deref(), &release.url)?;
    if total_bytes.is_some() && last_reported_percent != Some(100) {
        on_progress(Some(1.0));
    }
    log::info!("downloaded update. path:{:?}", target_path);

    Ok(())
}

/// Reads the next chunk of the response body, aborting the attempt if no data
/// arrives within [`DOWNLOAD_READ_TIMEOUT`]. A connection that opens but then
/// goes silent (a common failure on flaky networks) used to make the download
/// hang forever instead of triggering the retry loop above.
async fn read_body_chunk(
    body: &mut (impl AsyncRead + Unpin),
    buffer: &mut [u8],
    executor: &BackgroundExecutor,
) -> Result<usize> {
    let mut read = body.read(buffer);
    let mut idle = executor.timer(DOWNLOAD_READ_TIMEOUT);
    let result = smol::future::poll_fn(|cx| {
        if let Poll::Ready(result) = Pin::new(&mut read).poll(cx) {
            return Poll::Ready(result.map_err(Into::into));
        }
        if let Poll::Ready(_) = Pin::new(&mut idle).poll(cx) {
            return Poll::Ready(Err(anyhow::anyhow!(
                "download stalled: no data received within the idle timeout"
            )));
        }
        Poll::Pending
    })
    .await;
    result
}

async fn install_release_linux(
    temp_dir: &InstallerDir,
    downloaded_tar_gz: &Path,
    channel: &str,
    running_app_path: PathBuf,
) -> Result<Option<PathBuf>> {
    let home_dir = PathBuf::from(env::var("HOME").context("no HOME env var set")?);

    let extracted = temp_dir.path().join("aaykra");
    fs::create_dir_all(&extracted)
        .await
        .context("failed to create directory into which to extract update")?;

    let mut cmd = new_command("tar");
    cmd.arg("-xzf")
        .arg(&downloaded_tar_gz)
        .arg("-C")
        .arg(&extracted);
    let output = cmd
        .output()
        .await
        .with_context(|| format!("failed to extract: {cmd:?}"))?;

    anyhow::ensure!(
        output.status.success(),
        "failed to extract {:?} to {:?}: {:?}",
        downloaded_tar_gz,
        extracted,
        String::from_utf8_lossy(&output.stderr)
    );

    let suffix = if channel != "stable" {
        format!("-{}", channel)
    } else {
        String::default()
    };
    let app_folder_name = format!("aaykra{}.app", suffix);

    let from = extracted.join(&app_folder_name);
    let mut to = home_dir.join(".local");

    let expected_suffix = format!("{}/libexec/aaykra-editor", app_folder_name);

    let running_app_prefix = running_app_path
        .to_str()
        .and_then(|str| str.strip_suffix(&expected_suffix));

    if let Some(prefix) = running_app_prefix {
        to = PathBuf::from(prefix);
    }

    let mut cmd = new_command("rsync");
    cmd.args(["-av", "--delete"]).arg(&from).arg(&to);
    let output = cmd
        .output()
        .await
        .with_context(|| format!("failed to rsync: {cmd:?}"))?;

    anyhow::ensure!(
        output.status.success(),
        "failed to copy AAYKRA update from {:?} to {:?}: {:?}",
        from,
        to,
        String::from_utf8_lossy(&output.stderr)
    );

    // Only Linux has launchers that can point somewhere other than the
    // directory the update was just written to, so the per-user repointing
    // step does not exist on the other platforms.
    #[cfg(target_os = "linux")]
    if running_app_prefix.is_none() {
        install_per_user_launchers(&home_dir, &app_folder_name, channel).await?;
    }

    Ok(Some(to.join(expected_suffix)))
}

/// Points the user's launchers at the per-user copy under `~/.local` that
/// [`install_release_linux`] just wrote.
///
/// A package-managed install (`.deb`/`.pkg.tar.zst`) runs from a root-owned
/// `/opt/aaykra` that the updater cannot write to, so the update is staged in
/// `~/.local` instead. Without repointing the launchers, `/usr/bin/aaykra` and
/// the system desktop entry keep starting `/opt/aaykra`, so every fresh launch
/// runs the old build, rediscovers the newer release, and asks to restart
/// again, forever. Repointing them makes `~/.local` the copy that is actually
/// launched, after which updates are applied in place.
#[cfg(target_os = "linux")]
async fn install_per_user_launchers(
    home_dir: &Path,
    app_folder_name: &str,
    channel: &str,
) -> Result<()> {
    let app_id = match channel {
        "stable" => "me.aayush.Aaykra",
        "dev" => "me.aayush.Aaykra-Dev",
        unsupported_channel => anyhow::bail!("unknown release channel: {unsupported_channel}"),
    };

    let app_dir = home_dir.join(".local").join(app_folder_name);
    let cli = app_dir.join("bin").join("aaykra");
    anyhow::ensure!(
        cli.is_file(),
        "expected the update to contain a CLI at {}",
        cli.display()
    );

    let bin_dir = home_dir.join(".local").join("bin");
    fs::create_dir_all(&bin_dir)
        .await
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;

    let cli_on_path = bin_dir.join("aaykra");
    replace_symlink(&cli, &cli_on_path)
        .await
        .with_context(|| format!("failed to link {}", cli_on_path.display()))?;

    // The package layouts expose the CLI under this name as well; the tar.gz
    // layout only ships `aaykra`, but a symlink costs nothing either way.
    let cli_alias = bin_dir.join("aaykra-cli");
    replace_symlink(&cli, &cli_alias)
        .await
        .with_context(|| format!("failed to link {}", cli_alias.display()))?;

    // XDG searches `~/.local/share/applications` before `/usr/share/applications`,
    // so a user-level entry with the same app id shadows the package manager's
    // and the app launcher starts the updated copy.
    let bundled_desktop_entry = app_dir
        .join("share")
        .join("applications")
        .join(format!("{app_id}.desktop"));

    let desktop_entry = fs::read_to_string(&bundled_desktop_entry)
        .await
        .with_context(|| format!("failed to read {}", bundled_desktop_entry.display()))?;

    // Desktop entry values are quoted strings, so a home directory containing a
    // space still yields a single value. `TryExec` is repointed too: it decides
    // whether a launcher shows the app as available, and the bare `aaykra` name
    // is not resolvable from the desktop environment's `PATH`.
    //
    // `Icon` is left as the bundled icon name: the spec does not allow quoting
    // it, so a path containing a space cannot be expressed. The icons are
    // installed into the user's icon theme below instead, which is how a
    // tar.gz install resolves that name.
    let desktop_entry = desktop_entry
        .lines()
        .map(|line| {
            let repointed = ["Exec=", "TryExec="].into_iter().find_map(|prefix| {
                line.strip_prefix(prefix)
                    .and_then(|arguments| arguments.strip_prefix("aaykra"))
                    .map(|arguments| format!("{prefix}\"{}\"{}", cli_on_path.display(), arguments))
            });
            repointed.unwrap_or_else(|| line.to_owned())
        })
        .collect::<Vec<_>>()
        .join("\n");

    let applications_dir = home_dir.join(".local").join("share").join("applications");
    fs::create_dir_all(&applications_dir)
        .await
        .with_context(|| format!("failed to create {}", applications_dir.display()))?;

    let desktop_entry_path = applications_dir.join(format!("{app_id}.desktop"));
    fs::write(&desktop_entry_path, desktop_entry.as_bytes())
        .await
        .with_context(|| format!("failed to write {}", desktop_entry_path.display()))?;

    install_user_icons(home_dir, &app_dir, app_id).await?;

    log::info!(
        "Auto Update: installed the update to {app_dir:?} and pointed {} and {} at it",
        cli_on_path.display(),
        desktop_entry_path.display()
    );

    Ok(())
}

/// Installs the bundled icons into the user's icon theme, mirroring
/// `script/install.sh`, so the per-user copy is self-sufficient even when no
/// package manager installed the app system-wide.
#[cfg(target_os = "linux")]
async fn install_user_icons(home_dir: &Path, app_dir: &Path, app_id: &str) -> Result<()> {
    let bundled_icons = app_dir.join("share").join("icons").join("hicolor");
    let user_icons = home_dir
        .join(".local")
        .join("share")
        .join("icons")
        .join("hicolor");

    for size in ["512x512", "1024x1024"] {
        let source_dir = bundled_icons.join(size).join("apps");
        let target_dir = user_icons.join(size).join("apps");
        fs::create_dir_all(&target_dir)
            .await
            .with_context(|| format!("failed to create {}", target_dir.display()))?;

        // The app-id-named copy is what Wayland compositors look up by the
        // window's app id, so it is installed under that name as well.
        for name in ["aaykra.png", &format!("{app_id}.png")] {
            let source = source_dir.join(name);
            anyhow::ensure!(
                source.is_file(),
                "expected the update to contain an icon at {}",
                source.display()
            );
            fs::copy(&source, target_dir.join(name))
                .await
                .with_context(|| {
                    format!(
                        "failed to copy {} to {}",
                        source.display(),
                        target_dir.join(name).display()
                    )
                })?;
        }
    }

    // Only a cache: a failure here leaves the icons to be picked up lazily, and
    // not having the tool installed is the common case.
    if which::which("gtk-update-icon-cache").is_ok()
        && let Ok(output) = new_command("gtk-update-icon-cache")
            .arg("-f")
            .arg("-t")
            .arg(&user_icons)
            .output()
            .await
        && !output.status.success()
    {
        log::warn!(
            "Auto Update: gtk-update-icon-cache failed for {}: {}",
            user_icons.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Points `link` at `target`, replacing whatever `link` referred to before.
///
/// `symlink` fails when the destination already exists, which is the common
/// case here: a previous update already created the link, or the package
/// manager shipped a real file under the same name.
#[cfg(target_os = "linux")]
async fn replace_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(link).await {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(link).await?,
        Ok(_) => fs::remove_file(link).await?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    smol::fs::unix::symlink(target, link).await
}

async fn install_release_macos(
    temp_dir: &InstallerDir,
    downloaded_dmg: &Path,
    running_app_path: PathBuf,
    background_executor: &BackgroundExecutor,
) -> Result<Option<PathBuf>> {
    let running_app_filename = running_app_path
        .file_name()
        .with_context(|| format!("invalid running app path {running_app_path:?}"))?;

    let mount_path = temp_dir.path().join("AAYKRA");
    let mut mounted_app_path: OsString = mount_path.join(running_app_filename).into();

    mounted_app_path.push("/");
    let mut cmd = new_command("hdiutil");
    cmd.args(["attach", "-nobrowse"])
        .arg(&downloaded_dmg)
        .arg("-mountroot")
        .arg(temp_dir.path());
    let output = cmd
        .output()
        .await
        .with_context(|| format!("failed to mount: {cmd:?}"))?;

    anyhow::ensure!(
        output.status.success(),
        "failed to mount: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    let unmounter = MacOsUnmounter {
        mount_path: mount_path.clone(),
        background_executor,
    };

    let mut cmd = new_command("rsync");
    cmd.args(["-av", "--delete", "--exclude", "Icon?"])
        .arg(&mounted_app_path)
        .arg(&running_app_path);
    let rsync_output = cmd.output().await;

    // Await the unmount (even if rsync failed) so that the installer temp dir
    // can be deleted once this function returns.
    unmounter.unmount().await;

    let output = rsync_output.with_context(|| format!("failed to rsync: {cmd:?}"))?;

    anyhow::ensure!(
        output.status.success(),
        "failed to copy app: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(None)
}

/// Removes stale installer dirs from the system temp dir. Older Zed versions
/// leaked one per update by deleting the dir while the downloaded disk image
/// was still mounted inside it, which made the deletion fail silently.
#[cfg(any(rust_analyzer, all(not(target_os = "windows"), not(test))))]
async fn cleanup_stale_installer_dirs() {
    const STALE_INSTALLER_DIR_AGE: Duration = Duration::from_secs(24 * 60 * 60);

    let temp_dir = std::env::temp_dir();
    let Ok(mut entries) = fs::read_dir(&temp_dir).await else {
        log::warn!("failed to read temp dir {temp_dir:?} while cleaning up installer dirs");
        return;
    };
    while let Some(entry) = entries.next().await {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(INSTALLER_DIR_PREFIX)
        {
            continue;
        }
        // Leave recent dirs alone, as they may belong to an update currently
        // in progress in another Zed instance.
        let is_stale = entry.metadata().await.ok().is_some_and(|metadata| {
            metadata.is_dir()
                && metadata.modified().ok().is_some_and(|modified| {
                    SystemTime::now()
                        .duration_since(modified)
                        .is_ok_and(|age| age > STALE_INSTALLER_DIR_AGE)
                })
        });
        if is_stale {
            if let Err(error) = fs::remove_dir_all(entry.path()).await {
                log::warn!(
                    "failed to remove stale installer dir {:?}: {error}",
                    entry.path()
                );
            } else {
                log::info!("removed stale installer dir {:?}", entry.path());
            }
        }
    }
}

async fn cleanup_windows() -> Result<()> {
    let parent = std::env::current_exe()?
        .parent()
        .context("No parent dir for Aaykra.exe")?
        .to_owned();

    // keep in sync with crates/auto_update_helper/src/updater.rs. `updates` and
    // `install` hold a staged update until the helper applies it, so only their
    // empty leftovers are removed; `old` is safe to clear entirely.
    for (directory_name, recursive) in [("updates", false), ("install", false), ("old", true)] {
        let directory = parent.join(directory_name);
        if smol::fs::metadata(&directory).await.is_err() {
            continue;
        }
        let result = if recursive {
            smol::fs::remove_dir_all(&directory).await
        } else {
            smol::fs::remove_dir(&directory).await
        };
        match result {
            Ok(()) => log::info!("removed leftover update directory {directory:?}"),
            Err(error) => {
                log::info!("left update directory {directory:?} in place: {error}")
            }
        }
    }

    Ok(())
}

async fn install_release_windows(downloaded_installer: &Path) -> Result<Option<PathBuf>> {
    let mut cmd = new_command(downloaded_installer);
    cmd.arg("/verysilent")
        .arg("/update=true")
        .arg("/MERGETASKS=!desktopicon");
    let output = cmd.output().await?;
    anyhow::ensure!(
        output.status.success(),
        "failed to start installer: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    // We return the path to the update helper program, because it will
    // perform the final steps of the update process, copying the new binary,
    // deleting the old one, and launching the new binary.
    let helper_path = std::env::current_exe()?
        .parent()
        .context("No parent dir for Aaykra.exe")?
        .join("tools")
        .join("auto_update_helper.exe");
    Ok(Some(helper_path))
}

pub async fn finalize_auto_update_on_quit() {
    let Some(installer_path) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("updates")))
    else {
        return;
    };

    // The installer will create a flag file after it finishes updating
    let flag_file = installer_path.join("versions.txt");
    if flag_file.exists()
        && let Some(helper) = installer_path
            .parent()
            .map(|p| p.join("tools").join("auto_update_helper.exe"))
    {
        let mut command = util::command::new_command(helper);
        command.arg("--launch");
        command.arg("false");
        if let Ok(mut cmd) = command.spawn() {
            _ = cmd.status().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use client::Client;
    use futures::channel::oneshot;
    use gpui::TestAppContext;
    use http_client::{FakeHttpClient, Response};
    use settings::default_settings;
    use std::{
        rc::Rc,
        sync::{
            Arc,
            atomic::{self, AtomicBool},
        },
    };
    use tempfile::tempdir;

    #[ctor::ctor(unsafe)]
    fn init_logger() {
        zlog::init_test();
    }

    use super::*;

    pub(super) struct InstallOverride(pub Rc<dyn Fn(&Path, &AsyncApp) -> Result<Option<PathBuf>>>);
    impl Global for InstallOverride {}

    #[gpui::test]
    fn test_auto_update_defaults_to_true(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let mut store = SettingsStore::new(cx, &settings::default_settings());
            store
                .set_default_settings(&default_settings(), cx)
                .expect("Unable to set default settings");
            store
                .set_user_settings("{}", cx)
                .expect("Unable to set user settings");
            cx.set_global(store);
            assert!(AutoUpdateSetting::get_global(cx).0);
        });
    }

    #[test]
    fn test_github_asset_names_include_legacy_names() {
        assert_eq!(
            github_asset_names("zed", "linux", "x86_64").unwrap(),
            ["aaykra-linux-x86_64.tar.gz", "aaykra-linux-x86_64.tar.gz",]
        );
        assert_eq!(
            github_asset_names("zed", "macos", "aarch64").unwrap(),
            ["aaykra-aarch64.dmg", "Aaykra-aarch64.dmg"]
        );
        assert_eq!(
            github_asset_names("zed", "windows", "x86_64").unwrap(),
            ["aaykra-x86_64.exe", "Aaykra-x86_64.exe"]
        );
        assert_eq!(
            github_asset_names("aaykra-remote-server", "linux", "x86_64").unwrap(),
            [
                "aaykra-remote-server-linux-x86_64.gz",
                "aaykra-remote-server-linux-x86_64.gz",
            ]
        );
    }

    #[gpui::test]
    async fn test_auto_update_downloads(cx: &mut TestAppContext) {
        cx.background_executor.allow_parking();
        zlog::init_test();
        let release_available = Arc::new(AtomicBool::new(false));

        let (dmg_tx, dmg_rx) = oneshot::channel::<String>();

        cx.update(|cx| {
            settings::init(cx);

            let current_version = semver::Version::new(0, 100, 0);
            release_channel::init_test(current_version, ReleaseChannel::Stable, cx);

            let release_available = Arc::clone(&release_available);
            let dmg_rx = Arc::new(parking_lot::Mutex::new(Some(dmg_rx)));
            let fake_client_http = FakeHttpClient::create(move |req| {
                let release_available = release_available.load(atomic::Ordering::Relaxed);
                let dmg_rx = dmg_rx.clone();
                async move {
                if req.uri().path() == "/repos/aayushpokhrel49/Aaykra/releases/latest" {
                    let tag = if release_available { "v0.100.1" } else { "v0.100.0" };
                    let asset_name = github_asset_names("zed", OS, ARCH)
                        .unwrap()
                        .remove(0);
                    return Ok(Response::builder().status(200).body(
                        format!(
                            r#"{{"tag_name":"{tag}","assets":[{{"name":"{asset_name}","browser_download_url":"http://test.example/new-download","digest":null}}]}}"#
                        )
                        .into(),
                    )
                    .unwrap());
                } else if req.uri().path() == "/new-download" {
                    return Ok(Response::builder().status(200).body({
                        let dmg_rx = dmg_rx.lock().take().unwrap();
                        dmg_rx.await.unwrap().into()
                    }).unwrap());
                }
                Ok(Response::builder().status(404).body("".into()).unwrap())
                }
            });
            let client = Client::new(fake_client_http);
            crate::init(client, cx);
        });

        let auto_updater = cx.update(|cx| AutoUpdater::get(cx).expect("auto updater should exist"));

        cx.background_executor.run_until_parked();

        auto_updater.read_with(cx, |updater, _| {
            assert_eq!(updater.status(), AutoUpdateStatus::Idle);
            assert_eq!(updater.current_version(), semver::Version::new(0, 100, 0));
        });

        release_available.store(true, atomic::Ordering::SeqCst);
        cx.background_executor.advance_clock(POLL_INTERVAL);
        cx.background_executor.run_until_parked();

        loop {
            cx.background_executor.timer(Duration::from_millis(0)).await;
            cx.run_until_parked();
            let status = auto_updater.read_with(cx, |updater, _| updater.status());
            if !matches!(status, AutoUpdateStatus::Idle) {
                break;
            }
        }
        let status = auto_updater.read_with(cx, |updater, _| updater.status());
        assert_eq!(
            status,
            AutoUpdateStatus::Downloading {
                version: semver::Version::new(0, 100, 1),
                progress: None,
            }
        );

        dmg_tx.send("<fake-zed-update>".to_owned()).unwrap();

        let tmp_dir = Arc::new(tempdir().unwrap());

        cx.update(|cx| {
            let tmp_dir = tmp_dir.clone();
            cx.set_global(InstallOverride(Rc::new(move |target_path, _cx| {
                let tmp_dir = tmp_dir.clone();
                let dest_path = tmp_dir.path().join("zed");
                std::fs::copy(&target_path, &dest_path)?;
                Ok(Some(dest_path))
            })));
        });

        loop {
            cx.background_executor.timer(Duration::from_millis(0)).await;
            cx.run_until_parked();
            let status = auto_updater.read_with(cx, |updater, _| updater.status());
            if !matches!(status, AutoUpdateStatus::Downloading { .. }) {
                break;
            }
        }
        let status = auto_updater.read_with(cx, |updater, _| updater.status());
        assert_eq!(
            status,
            AutoUpdateStatus::Updated {
                version: semver::Version::new(0, 100, 1)
            }
        );
        let will_restart = cx.expect_restart();
        cx.update(|cx| cx.restart()).expect("restart was rejected");
        let (path, arguments) = will_restart.await.unwrap();
        assert!(arguments.is_empty());
        let path = path.unwrap();
        assert_eq!(path, tmp_dir.path().join("zed"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "<fake-zed-update>");
    }

    #[gpui::test]
    async fn test_auto_update_does_not_error_when_latest_release_misses_platform_asset(
        cx: &mut TestAppContext,
    ) {
        cx.background_executor.allow_parking();
        zlog::init_test();

        cx.update(|cx| {
            let mut store = SettingsStore::new(cx, &settings::default_settings());
            store
                .set_default_settings(&settings::default_settings(), cx)
                .expect("Unable to set default settings");
            store
                .set_user_settings(r#"{"auto_update": false}"#, cx)
                .expect("Unable to set user settings");
            cx.set_global(store);

            let current_version = semver::Version::new(1, 0, 0);
            release_channel::init_test(current_version, ReleaseChannel::Stable, cx);

            // The latest release matches the installed version but carries no
            // asset for this platform. The check must settle quietly on Idle
            // instead of surfacing the missing-asset error for a release we
            // are already running.
            let fake_client_http = FakeHttpClient::create(|_req| async move {
                Ok(Response::builder()
                    .status(200)
                    .body(
                        r#"{"tag_name":"v1.0.0","assets":[{"name":"aaykra-x86_64.exe","browser_download_url":"http://test.example/win.exe","digest":null}]}"#
                            .into(),
                    )
                    .unwrap())
            });
            let client = Client::new(fake_client_http);
            crate::init(client, cx);
        });

        let auto_updater = cx.update(|cx| AutoUpdater::get(cx).expect("auto updater should exist"));

        auto_updater.update(cx, |updater, cx| {
            updater.poll(UpdateCheckType::Manual, cx);
        });
        wait_until_poll_finished(cx, &auto_updater).await;

        let status = auto_updater.read_with(cx, |updater, _| updater.status());
        assert_eq!(status, AutoUpdateStatus::Idle);
    }

    async fn wait_until_poll_finished(cx: &mut TestAppContext, auto_updater: &Entity<AutoUpdater>) {
        loop {
            cx.background_executor.timer(Duration::from_millis(0)).await;
            cx.run_until_parked();
            let poll_pending =
                auto_updater.read_with(cx, |updater, _| updater.pending_poll.is_some());
            if !poll_pending {
                return;
            }
        }
    }

    #[gpui::test]
    async fn test_download_release_reports_progress(cx: &mut TestAppContext) {
        cx.background_executor.allow_parking();

        let body = vec![0u8; 20_000];
        let content_length = body.len();

        let client = FakeHttpClient::create(move |_req| {
            let body = body.clone();
            async move {
                Ok(Response::builder()
                    .status(200)
                    .header(
                        http_client::http::header::CONTENT_LENGTH,
                        body.len().to_string(),
                    )
                    .body(body.into())
                    .unwrap())
            }
        });

        let temp_dir = tempdir().unwrap();
        let target_path = temp_dir.path().join("zed-download");
        let release = ReleaseAsset {
            version: "1.0.0".to_string(),
            url: "https://test.example/download".to_string(),
            digest: None,
        };

        let reported = Rc::new(std::cell::RefCell::new(Vec::<f32>::new()));
        download_release(
            &target_path,
            release,
            client,
            cx.background_executor.clone(),
            {
                let reported = reported.clone();
                move |fraction| {
                    if let Some(fraction) = fraction {
                        reported.borrow_mut().push(fraction);
                    }
                }
            },
        )
        .await
        .unwrap();

        let reported = reported.borrow();
        assert!(
            reported.len() >= 2,
            "expected progress to be reported across multiple reads, got {reported:?}"
        );
        assert_eq!(
            reported.last().copied(),
            Some(1.0),
            "download should finish at 100%"
        );
        for fraction in reported.iter() {
            assert!(
                (0.0..=1.0).contains(fraction),
                "progress {fraction} out of range"
            );
        }
        for pair in reported.windows(2) {
            assert!(
                pair[0] <= pair[1],
                "progress must not decrease: {reported:?}"
            );
        }

        let downloaded_len = std::fs::metadata(&target_path).unwrap().len();
        assert_eq!(downloaded_len, content_length as u64);
    }

    #[gpui::test]
    async fn test_download_release_without_content_length_reports_no_progress(
        cx: &mut TestAppContext,
    ) {
        cx.background_executor.allow_parking();

        let body = vec![0u8; 20_000];
        let content_length = body.len();

        let client = FakeHttpClient::create(move |_req| {
            let body = body.clone();
            async move { Ok(Response::builder().status(200).body(body.into()).unwrap()) }
        });

        let temp_dir = tempdir().unwrap();
        let target_path = temp_dir.path().join("zed-download");
        let release = ReleaseAsset {
            version: "1.0.0".to_string(),
            url: "https://test.example/download".to_string(),
            digest: None,
        };

        let reported = Rc::new(std::cell::RefCell::new(Vec::<Option<f32>>::new()));
        download_release(
            &target_path,
            release,
            client,
            cx.background_executor.clone(),
            {
                let reported = reported.clone();
                move |fraction| {
                    reported.borrow_mut().push(fraction);
                }
            },
        )
        .await
        .unwrap();

        assert!(
            reported.borrow().is_empty(),
            "progress should not be reported when the total size is unknown, got {:?}",
            reported.borrow()
        );

        let downloaded_len = std::fs::metadata(&target_path).unwrap().len();
        assert_eq!(downloaded_len, content_length as u64);
    }

    #[test]
    fn test_stable_does_not_update_when_fetched_version_is_not_higher() {
        let installed_version = semver::Version::new(1, 0, 0);
        let status = AutoUpdateStatus::Idle;
        let fetched_version = semver::Version::new(1, 0, 0);

        let newer_version = AutoUpdater::check_if_fetched_version_is_newer(
            installed_version,
            fetched_version.to_string(),
            status,
        );

        assert_eq!(newer_version.unwrap(), None);
    }

    #[test]
    fn test_stable_does_update_when_fetched_version_is_higher() {
        let installed_version = semver::Version::new(1, 0, 0);
        let status = AutoUpdateStatus::Idle;
        let fetched_version = semver::Version::new(1, 0, 1);

        let newer_version = AutoUpdater::check_if_fetched_version_is_newer(
            installed_version,
            fetched_version.to_string(),
            status,
        );

        assert_eq!(newer_version.unwrap(), Some(fetched_version));
    }

    #[test]
    fn test_stable_does_not_update_when_fetched_version_is_not_higher_than_cached() {
        let installed_version = semver::Version::new(1, 0, 0);
        let status = AutoUpdateStatus::Updated {
            version: semver::Version::new(1, 0, 1),
        };
        let fetched_version = semver::Version::new(1, 0, 1);

        let newer_version = AutoUpdater::check_if_fetched_version_is_newer(
            installed_version,
            fetched_version.to_string(),
            status,
        );

        assert_eq!(newer_version.unwrap(), None);
    }

    #[test]
    fn test_stable_does_update_when_fetched_version_is_higher_than_cached() {
        let installed_version = semver::Version::new(1, 0, 0);
        let status = AutoUpdateStatus::Updated {
            version: semver::Version::new(1, 0, 1),
        };
        let fetched_version = semver::Version::new(1, 0, 2);

        let newer_version = AutoUpdater::check_if_fetched_version_is_newer(
            installed_version,
            fetched_version.to_string(),
            status,
        );

        assert_eq!(newer_version.unwrap(), Some(fetched_version));
    }

    /// A package-managed install runs from a root-owned `/opt/aaykra`, so the
    /// update lands in `~/.local`. The launchers must follow it there, or every
    /// launch restarts the old build and the update loop never ends.
    #[cfg(target_os = "linux")]
    #[gpui::test]
    async fn test_install_per_user_launchers_repoints_launchers(cx: &mut TestAppContext) {
        cx.background_executor.allow_parking();

        const APP_ID: &str = "me.aayush.Aaykra";

        let home = tempdir().unwrap();
        let app_dir = home.path().join(".local").join("aaykra.app");

        let bundled_desktop_entry = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=AAYKRA\n\
             TryExec=aaykra\n\
             Exec=aaykra %U\n\
             Icon=aaykra\n\
             Actions=NewWorkspace;\n\
             \n\
             [Desktop Action NewWorkspace]\n\
             Exec=aaykra --new %U\n\
             Name=Open a new workspace\n"
        );

        let write_bundle_file = |path: PathBuf, contents: &[u8]| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        };
        write_bundle_file(app_dir.join("bin").join("aaykra"), b"cli");
        write_bundle_file(app_dir.join("libexec").join("aaykra-editor"), b"editor");
        write_bundle_file(
            app_dir
                .join("share")
                .join("applications")
                .join(format!("{APP_ID}.desktop")),
            bundled_desktop_entry.as_bytes(),
        );
        for size in ["512x512", "1024x1024"] {
            for name in ["aaykra.png", &format!("{APP_ID}.png")] {
                write_bundle_file(
                    app_dir
                        .join("share")
                        .join("icons")
                        .join("hicolor")
                        .join(size)
                        .join("apps")
                        .join(name),
                    b"icon",
                );
            }
        }

        // A previous update may already have left a link behind, and the package
        // manager may have shipped a real binary under the same name.
        let cli_on_path = home.path().join(".local").join("bin").join("aaykra");
        std::fs::create_dir_all(cli_on_path.parent().unwrap()).unwrap();
        std::fs::write(&cli_on_path, b"stale").unwrap();

        install_per_user_launchers(home.path(), "aaykra.app", "stable")
            .await
            .unwrap();

        let cli = app_dir.join("bin").join("aaykra");
        for link in [
            cli_on_path.clone(),
            home.path().join(".local/bin/aaykra-cli"),
        ] {
            assert_eq!(std::fs::read_link(&link).unwrap(), cli);
        }

        let desktop_entry = std::fs::read_to_string(
            home.path()
                .join(".local")
                .join("share")
                .join("applications")
                .join(format!("{APP_ID}.desktop")),
        )
        .unwrap();
        let exec = format!("\"{}\"", cli_on_path.display());
        assert!(desktop_entry.contains(&format!("Exec={exec} %U\n")));
        assert!(desktop_entry.contains(&format!("Exec={exec} --new %U\n")));
        assert!(desktop_entry.contains(&format!("TryExec={exec}\n")));
        // Quoting keeps a home directory containing a space a single value.
        assert!(!desktop_entry.contains("Exec=aaykra"));

        for size in ["512x512", "1024x1024"] {
            for name in ["aaykra.png", &format!("{APP_ID}.png")] {
                let icon = home
                    .path()
                    .join(".local")
                    .join("share")
                    .join("icons")
                    .join("hicolor")
                    .join(size)
                    .join("apps")
                    .join(name);
                assert_eq!(std::fs::read(icon).unwrap(), b"icon");
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[gpui::test]
    async fn test_install_per_user_launchers_rejects_unknown_channel(_cx: &mut TestAppContext) {
        let home = tempdir().unwrap();
        let error = install_per_user_launchers(home.path(), "aaykra.app", "nightly")
            .await
            .expect_err("expected an unknown channel to be rejected");
        assert!(
            error.to_string().contains("unknown release channel"),
            "unexpected error: {error:?}"
        );
    }
}
