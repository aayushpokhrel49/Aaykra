use aaykra_actions::{Quit, debug_panel, dev, git_panel, project_panel};
use gpui::{App, Menu, MenuItem, OsAction};
use project::DisableAiSettings;
use release_channel::ReleaseChannel;
use settings::Settings;
use terminal_view::terminal_panel;

pub fn app_menus(cx: &mut App) -> Vec<Menu> {
    let mut view_items = vec![
        MenuItem::action(
            "Zoom In",
            aaykra_actions::IncreaseBufferFontSize { persist: false },
        ),
        MenuItem::action(
            "Zoom Out",
            aaykra_actions::DecreaseBufferFontSize { persist: false },
        ),
        MenuItem::action(
            "Reset Zoom",
            aaykra_actions::ResetBufferFontSize { persist: false },
        ),
        MenuItem::action(
            "Reset All Zoom",
            aaykra_actions::ResetAllZoom { persist: false },
        ),
        MenuItem::separator(),
        MenuItem::action("Toggle Left Dock", workspace::ToggleLeftDock),
        MenuItem::action("Toggle Right Dock", workspace::ToggleRightDock),
        MenuItem::action("Toggle Bottom Dock", workspace::ToggleBottomDock),
        MenuItem::action("Toggle All Docks", workspace::ToggleAllDocks),
        MenuItem::submenu(Menu {
            name: "Editor Layout".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Split Up", workspace::SplitUp::default()),
                MenuItem::action("Split Down", workspace::SplitDown::default()),
                MenuItem::action("Split Left", workspace::SplitLeft::default()),
                MenuItem::action("Split Right", workspace::SplitRight::default()),
            ],
        }),
        MenuItem::separator(),
        MenuItem::action("Project Panel", project_panel::ToggleFocus),
        MenuItem::action("Outline Panel", outline_panel::ToggleFocus),
        MenuItem::action("Terminal Panel", terminal_panel::Toggle),
        MenuItem::action("Debugger Panel", debug_panel::ToggleFocus),
<<<<<<< 5a0780efc8eaf2484f8abfa16efc6194cfe72888:crates/aaykra/src/aaykra/app_menus.rs
=======
    ];

    if !DisableAiSettings::get_global(cx).disable_ai {
        view_items.push(MenuItem::action("Agent Panel", assistant::ToggleFocus));
    }

    view_items.extend([
>>>>>>> 2abf58ae93479ba0ebd02d357f3332c9d40baf50:crates/zed/src/zed/app_menus.rs
        MenuItem::action("Git Panel", git_panel::ToggleFocus),
        MenuItem::separator(),
        MenuItem::action("Diagnostics", diagnostics::Deploy),
        MenuItem::separator(),
    ]);

    if ReleaseChannel::try_global(cx) == Some(ReleaseChannel::Dev) {
        view_items.push(MenuItem::action(
            "Toggle GPUI Inspector",
            dev::ToggleInspector,
        ));
        view_items.push(MenuItem::separator());
    }

    vec![
        Menu {
            name: "AAYKRA".into(),
            disabled: false,
            items: vec![
                MenuItem::action("About AAYKRA", aaykra_actions::About),
                MenuItem::action("Check for Updates", auto_update::Check),
                MenuItem::separator(),
                MenuItem::submenu(Menu::new("Settings").items([
                    MenuItem::action("Open Settings", aaykra_actions::OpenSettings),
                    MenuItem::action("Open Settings File", super::OpenSettingsFile),
                    MenuItem::action("Open Project Settings", aaykra_actions::OpenProjectSettings),
                    MenuItem::action("Open Project Settings File", super::OpenProjectSettingsFile),
                    MenuItem::action("Open Default Settings", super::OpenDefaultSettings),
                    MenuItem::separator(),
                    MenuItem::action("Open Keymap", aaykra_actions::OpenKeymap),
                    MenuItem::action("Open Keymap File", aaykra_actions::OpenKeymapFile),
                    MenuItem::action(
                        "Open Default Key Bindings",
                        aaykra_actions::OpenDefaultKeymap,
                    ),
                    MenuItem::separator(),
                    MenuItem::action(
                        "Select Theme...",
                        aaykra_actions::theme_selector::Toggle::default(),
                    ),
                    MenuItem::action(
                        "Select Icon Theme...",
                        aaykra_actions::icon_theme_selector::Toggle::default(),
                    ),
                ])),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::os_submenu("Services", gpui::SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Extensions", aaykra_actions::Extensions::default()),
                #[cfg(not(target_os = "windows"))]
                MenuItem::action("Install CLI", install_cli::InstallCliBinary),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::action("Hide AAYKRA", super::Hide),
                #[cfg(target_os = "macos")]
                MenuItem::action("Hide Others", super::HideOthers),
                #[cfg(target_os = "macos")]
                MenuItem::action("Show All", super::ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit AAYKRA", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            disabled: false,
            items: vec![
                MenuItem::action("New", workspace::NewFile),
                MenuItem::action("New Window", workspace::NewWindow),
                MenuItem::separator(),
                #[cfg(not(target_os = "macos"))]
                MenuItem::action("Open File...", workspace::OpenFiles),
                MenuItem::action(
                    if cfg!(not(target_os = "macos")) {
                        "Open Folder..."
                    } else {
                        "Open…"
                    },
                    workspace::Open::default(),
                ),
                MenuItem::action("Open Recent…", aaykra_actions::OpenRecent::default()),
                MenuItem::action("Open Remote…", aaykra_actions::OpenRemote::default()),
                MenuItem::separator(),
                MenuItem::action("Add Folder to Project…", workspace::AddFolderToProject),
                MenuItem::separator(),
                MenuItem::action("Save", workspace::Save { save_intent: None }),
                MenuItem::action("Save As…", workspace::SaveAs),
                MenuItem::action("Save All", workspace::SaveAll { save_intent: None }),
                MenuItem::separator(),
                MenuItem::action(
                    "Close Editor",
                    workspace::CloseActiveItem {
                        save_intent: None,
                        close_pinned: true,
                    },
                ),
                MenuItem::action("Close Project", workspace::CloseProject),
                MenuItem::action("Close Window", workspace::CloseWindow),
            ],
        },
        Menu {
            name: "Edit".into(),
            disabled: false,
            items: vec![
                MenuItem::os_action("Undo", editor::actions::Undo, OsAction::Undo),
                MenuItem::os_action("Redo", editor::actions::Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", editor::actions::Cut, OsAction::Cut),
                MenuItem::os_action("Copy", editor::actions::Copy, OsAction::Copy),
                MenuItem::action("Copy and Trim", editor::actions::CopyAndTrim),
                MenuItem::os_action("Paste", editor::actions::Paste, OsAction::Paste),
                MenuItem::separator(),
                MenuItem::action("Find", search::buffer_search::Deploy::find()),
                MenuItem::action("Find in Project", workspace::DeploySearch::default()),
                MenuItem::separator(),
                MenuItem::action(
                    "Toggle Line Comment",
                    editor::actions::ToggleComments::default(),
                ),
            ],
        },
        Menu {
            name: "Selection".into(),
            disabled: false,
            items: vec![
                MenuItem::os_action(
                    "Select All",
                    editor::actions::SelectAll,
                    OsAction::SelectAll,
                ),
                MenuItem::action("Expand Selection", editor::actions::SelectLargerSyntaxNode),
                MenuItem::action("Shrink Selection", editor::actions::SelectSmallerSyntaxNode),
                MenuItem::action("Select Next Sibling", editor::actions::SelectNextSyntaxNode),
                MenuItem::action(
                    "Select Previous Sibling",
                    editor::actions::SelectPreviousSyntaxNode,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    "Add Cursor Above",
                    editor::actions::AddSelectionAbove {
                        skip_soft_wrap: true,
                    },
                ),
                MenuItem::action(
                    "Add Cursor Below",
                    editor::actions::AddSelectionBelow {
                        skip_soft_wrap: true,
                    },
                ),
                MenuItem::action(
                    "Select Next Occurrence",
                    editor::actions::SelectNext {
                        replace_newest: false,
                    },
                ),
                MenuItem::action(
                    "Select Previous Occurrence",
                    editor::actions::SelectPrevious {
                        replace_newest: false,
                    },
                ),
                MenuItem::action("Select All Occurrences", editor::actions::SelectAllMatches),
                MenuItem::separator(),
                MenuItem::action("Move Line Up", editor::actions::MoveLineUp),
                MenuItem::action("Move Line Down", editor::actions::MoveLineDown),
                MenuItem::action("Duplicate Selection", editor::actions::DuplicateLineDown),
            ],
        },
        Menu {
            name: "View".into(),
            disabled: false,
            items: view_items,
        },
        Menu {
            name: "Go".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Back", workspace::GoBack),
                MenuItem::action("Forward", workspace::GoForward),
                MenuItem::separator(),
                MenuItem::action(
                    "Command Palette...",
                    aaykra_actions::command_palette::Toggle,
                ),
                MenuItem::separator(),
                MenuItem::action("Go to File...", workspace::ToggleFileFinder::default()),
                // MenuItem::action("Go to Symbol in Project", project_symbols::Toggle),
                MenuItem::action(
                    "Go to Symbol in Editor...",
                    aaykra_actions::outline::ToggleOutline,
                ),
                MenuItem::action("Go to Line/Column...", editor::actions::ToggleGoToLine),
                MenuItem::separator(),
                MenuItem::action(
                    "Go to Definition",
                    editor::actions::GoToDefinition::default(),
                ),
                MenuItem::action(
                    "Go to Declaration",
                    editor::actions::GoToDeclaration::default(),
                ),
                MenuItem::action(
                    "Go to Type Definition",
                    editor::actions::GoToTypeDefinition::default(),
                ),
                MenuItem::action(
                    "Find All References",
                    editor::actions::FindAllReferences::default(),
                ),
                MenuItem::action("Show Incoming Calls", call_hierarchy::ShowIncomingCalls),
                MenuItem::action("Show Outgoing Calls", call_hierarchy::ShowOutgoingCalls),
                MenuItem::separator(),
                MenuItem::action("Next Problem", editor::actions::GoToDiagnostic::default()),
                MenuItem::action(
                    "Previous Problem",
                    editor::actions::GoToPreviousDiagnostic::default(),
                ),
            ],
        },
        Menu {
            name: "Run".into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    "Spawn Task",
                    aaykra_actions::Spawn::ViaModal {
                        reveal_target: None,
                    },
                ),
                MenuItem::action("Start Debugger", debugger_ui::Start),
                MenuItem::separator(),
                MenuItem::action("Edit tasks.json…", aaykra_actions::OpenProjectTasks),
                MenuItem::action("Edit debug.json…", aaykra_actions::OpenProjectDebugTasks),
                MenuItem::separator(),
                MenuItem::action("Continue", debugger_ui::Continue),
                MenuItem::action("Step Over", debugger_ui::StepOver),
                MenuItem::action("Step Into", debugger_ui::StepInto),
                MenuItem::action("Step Out", debugger_ui::StepOut),
                MenuItem::separator(),
                MenuItem::action("Toggle Breakpoint", editor::actions::ToggleBreakpoint),
                MenuItem::action("Edit Breakpoint", editor::actions::EditLogBreakpoint),
                MenuItem::action("Clear All Breakpoints", debugger_ui::ClearAllBreakpoints),
            ],
        },
        Menu {
            name: "Terminal".into(),
            disabled: false,
            items: vec![
                MenuItem::submenu(Menu::new("New").items([
                    MenuItem::action("Terminal", workspace::NewTerminal::default()),
                    MenuItem::action("Center Terminal", workspace::NewCenterTerminal::default()),
                    MenuItem::action("Local Terminal", workspace::NewTerminal { local: true }),
                ])),
                MenuItem::separator(),
                MenuItem::action("Show Terminal Panel", terminal_panel::Toggle),
                MenuItem::action("Focus Terminal Panel", terminal_panel::ToggleFocus),
                MenuItem::separator(),
                MenuItem::submenu(Menu::new("Tasks").items([
                    MenuItem::action(
                        "Run Task…",
                        aaykra_actions::Spawn::ViaModal {
                            reveal_target: None,
                        },
                    ),
                    MenuItem::action("Rerun Last Task", terminal_view::RerunTask),
                    MenuItem::separator(),
                    MenuItem::action("Configure Tasks…", aaykra_actions::OpenProjectTasks),
                    MenuItem::action("Open Global Tasks File", super::OpenTasks),
                ])),
                MenuItem::separator(),
                MenuItem::action("Rename Terminal", terminal_view::RenameTerminal),
                MenuItem::separator(),
                MenuItem::action("Clear", terminal::Clear),
                MenuItem::action("Copy", terminal::Copy),
                MenuItem::action("Paste", terminal::Paste),
                MenuItem::action("Select All", terminal::SelectAll),
            ],
        },
        Menu {
            name: "Window".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Minimize", super::Minimize),
                MenuItem::action("Zoom", super::Zoom),
                MenuItem::separator(),
            ],
        },
        Menu {
            name: "Help".into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    "View Release Notes Locally",
                    auto_update_ui::ViewReleaseNotesLocally,
                ),
                MenuItem::action("View Dependency Licenses", aaykra_actions::OpenLicenses),
                MenuItem::action("Show Welcome", onboarding::ShowWelcome),
                MenuItem::separator(),
                MenuItem::action(
                    "Documentation",
                    super::OpenBrowser {
                        url: "https://aayushhpokhrel.com.np".into(),
                    },
                ),
                MenuItem::action(
                    "AAYKRA Repository",
                    super::OpenBrowser {
                        url: "https://github.com/aayushpokhrel49/Aaykra".into(),
                    },
                ),
            ],
        },
    ]
}
