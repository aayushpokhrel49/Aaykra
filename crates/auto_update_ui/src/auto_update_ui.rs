use std::sync::Arc;

use auto_update::{AutoUpdateStatus, AutoUpdater, release_notes_url};
use gpui::{
    App, DismissEvent, EventEmitter, FocusHandle, Focusable, TaskExt, Window, actions,
    prelude::*,
};
use release_channel::ReleaseChannel;
use semver::Version;
use ui::{AnnouncementToast, ListBulletItem, prelude::*};
use workspace::{
    Workspace,
    notifications::{
        Notification, NotificationId, SuppressEvent, show_app_notification,
        simple_message_notification::MessageNotification,
    },
};
use aaykra_actions::ShowUpdateNotification;

actions!(
    auto_update,
    [
        /// Opens the release notes for the current version in a new tab.
        ViewReleaseNotesLocally
    ]
);

pub fn init(cx: &mut App) {
    notify_if_app_was_updated(cx);
    observe_update_availability(cx);
    cx.observe_new(|workspace: &mut Workspace, _window, cx| {
        workspace.register_action(|workspace, _: &ViewReleaseNotesLocally, window, cx| {
            view_release_notes_locally(workspace, window, cx);
        });

        if matches!(ReleaseChannel::global(cx), ReleaseChannel::Dev) {
            workspace.register_action(|_workspace, _: &ShowUpdateNotification, _window, cx| {
                show_update_notification(cx);
            });
        }
    })
    .detach();
}

fn view_release_notes_locally(
    _workspace: &mut Workspace,
    _window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    if let Some(url) = release_notes_url(cx) {
        cx.open_url(&url);
    }
}

#[derive(Clone)]
struct AnnouncementContent {
    heading: SharedString,
    description: SharedString,
    bullet_items: Vec<SharedString>,
    primary_action_label: SharedString,
    secondary_action_label: SharedString,
    primary_action_url: Option<SharedString>,
    primary_action_callback: Option<Arc<dyn Fn(&mut Window, &mut App) + Send + Sync>>,
    secondary_action_url: Option<SharedString>,
    on_dismiss: Option<Arc<dyn Fn(&mut App) + Send + Sync>>,
}

fn announcement_for_version(_version: &Version, _cx: &App) -> Option<AnnouncementContent> {
    None
}

struct AnnouncementToastNotification {
    focus_handle: FocusHandle,
    content: AnnouncementContent,
}

impl AnnouncementToastNotification {
    fn new(content: AnnouncementContent, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content,
        }
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
        if let Some(on_dismiss) = &self.content.on_dismiss {
            on_dismiss(cx);
        }
    }
}

impl Focusable for AnnouncementToastNotification {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for AnnouncementToastNotification {}
impl EventEmitter<SuppressEvent> for AnnouncementToastNotification {}
impl Notification for AnnouncementToastNotification {}

impl Render for AnnouncementToastNotification {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        AnnouncementToast::new()
            .heading(self.content.heading.clone())
            .description(self.content.description.clone())
            .bullet_items(
                self.content
                    .bullet_items
                    .iter()
                    .map(|item| ListBulletItem::new(item.clone())),
            )
            .primary_action_label(self.content.primary_action_label.clone())
            .secondary_action_label(self.content.secondary_action_label.clone())
            .primary_on_click(cx.listener({
                let url = self.content.primary_action_url.clone();
                let callback = self.content.primary_action_callback.clone();
                move |this, _, window, cx| {
                    if let Some(callback) = &callback {
                        callback(window, cx);
                    }
                    if let Some(url) = &url {
                        cx.open_url(url);
                    }
                    this.dismiss(cx);
                }
            }))
            .secondary_on_click(cx.listener({
                let url = self.content.secondary_action_url.clone();
                move |_, _, _window, cx| {
                    if let Some(url) = &url {
                        cx.open_url(url);
                    }
                }
            }))
            .dismiss_on_click(cx.listener(|this, _, _window, cx| {
                this.dismiss(cx);
            }))
    }
}

struct UpdateNotification;

/// Prompts the user to apply a staged update. It can be dismissed within a
/// session, but it is re-shown on the next poll and on every app launch until
/// the update is actually applied, so it keeps appearing until the user
/// updates.
struct UpdateAvailableNotification;

fn show_update_available_notification(version: Version, cx: &mut App) {
    let app_name = ReleaseChannel::global(cx).display_name();
    show_app_notification(
        NotificationId::unique::<UpdateAvailableNotification>(),
        cx,
        move |cx| {
            let version = version.clone();
            cx.new(|cx| {
                MessageNotification::new(format!("{app_name} {version} is ready to update"), cx)
                    .primary_message("Update Now")
                    .primary_on_click(move |_window, cx| {
                        workspace::reload(cx);
                        cx.emit(DismissEvent);
                    })
            })
        },
    );
}

/// Returns the version of an update that has been staged but not yet applied,
/// if one is pending. `None` when there is nothing waiting to be updated.
fn pending_update(auto_updater: &AutoUpdater) -> Option<Version> {
    let current_version = auto_updater.current_version();
    let AutoUpdateStatus::Updated { version } = auto_updater.status() else {
        return None;
    };
    (version > current_version).then_some(version)
}

fn observe_update_availability(cx: &mut App) {
    let Some(auto_updater) = AutoUpdater::get(cx) else {
        return;
    };

    // If a previous session staged an update that has not been applied yet,
    // remind the user on launch rather than waiting for the next poll.
    if let Some(version) = pending_update(&auto_updater.read(cx)) {
        show_update_available_notification(version, cx);
    }

    cx.observe(&auto_updater, |auto_updater, cx| {
        if let Some(version) = pending_update(&auto_updater.read(cx)) {
            show_update_available_notification(version, cx);
        }
    })
    .detach();
}

fn show_update_notification(cx: &mut App) {
    let Some(updater) = AutoUpdater::get(cx) else {
        return;
    };

    let mut version = updater.read(cx).current_version();
    version.pre = semver::Prerelease::EMPTY;
    version.build = semver::BuildMetadata::EMPTY;
    let app_name = ReleaseChannel::global(cx).display_name();

    if let Some(content) = announcement_for_version(&version, cx) {
        show_app_notification(
            NotificationId::unique::<UpdateNotification>(),
            cx,
            move |cx| cx.new(|cx| AnnouncementToastNotification::new(content.clone(), cx)),
        );
    } else {
        show_app_notification(
            NotificationId::unique::<UpdateNotification>(),
            cx,
            move |cx| {
                let workspace_handle = cx.entity().downgrade();
                cx.new(|cx| {
                    MessageNotification::new(format!("Updated to {app_name} {}", version), cx)
                        .primary_message("View Release Notes")
                        .primary_on_click(move |window, cx| {
                            if let Some(workspace) = workspace_handle.upgrade() {
                                workspace.update(cx, |workspace, cx| {
                                    crate::view_release_notes_locally(workspace, window, cx);
                                })
                            }
                            cx.emit(DismissEvent);
                        })
                        .show_suppress_button(false)
                })
            },
        );
    }
}

/// Shows a notification across all workspaces if an update was previously automatically installed
/// and this notification had not yet been shown.
pub fn notify_if_app_was_updated(cx: &mut App) {
    let Some(updater) = AutoUpdater::get(cx) else {
        return;
    };

    let should_show_notification = updater.read(cx).should_show_update_notification(cx);

    cx.spawn(async move |cx| {
        let should_show_notification = should_show_notification.await?;

        if should_show_notification {
            cx.update(|cx| {
                show_update_notification(cx);
                updater.update(cx, |updater, cx| {
                    updater
                        .set_should_show_update_notification(false, cx)
                        .detach_and_log_err(cx);
                });
            });
        }
        anyhow::Ok(())
    })
    .detach();
}
