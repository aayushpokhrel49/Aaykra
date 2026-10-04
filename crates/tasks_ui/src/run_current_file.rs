use gpui::{Context, Window};
use project::{TaskContexts, TaskSourceKind};
use task::{VariableName, run};
use workspace::Workspace;

use crate::task_contexts;

/// Handles `task::RunCurrentFile`: builds and runs the active file in the terminal panel.
pub fn run_current_file(
    workspace: &mut Workspace,
    _action: &aaykra_actions::RunCurrentFile,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let task_contexts = task_contexts(workspace, window, cx);
    cx.spawn_in(window, async move |workspace, cx| {
        let task_contexts = task_contexts.await;
        workspace
            .update_in(cx, |workspace, window, cx| {
                schedule_run_current_file(workspace, task_contexts, window, cx);
            })
            .ok();
    })
    .detach();
}

fn schedule_run_current_file(
    workspace: &mut Workspace,
    task_contexts: TaskContexts,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(location) = task_contexts.location() else {
        workspace.show_error("Open a file to run it.", cx);
        return;
    };
    let Some(task_context) = task_contexts.active_context() else {
        workspace.show_error("Open a file to run it.", cx);
        return;
    };

    // `$ZED_FILE`, `$ZED_DIRNAME` and `$ZED_STEM` are all resolved from the buffer's file, and
    // a recipe referencing a variable the context lacks resolves to nothing at all, silently.
    if task_context
        .task_variables
        .get(&VariableName::File)
        .is_none_or(str::is_empty)
    {
        workspace.show_error("Save this file before running it.", cx);
        return;
    }

    let (language_name, file_extension) = {
        let buffer = location.buffer.read(cx);
        (
            buffer
                .language()
                .map(|language| language.name().to_string()),
            buffer
                .file()
                .and_then(|file| file.path().extension())
                .map(str::to_string),
        )
    };

    let Some(task) =
        run::run_current_file_task(language_name.as_deref(), file_extension.as_deref())
    else {
        workspace.show_error(unsupported_language_message(language_name.as_deref()), cx);
        return;
    };

    workspace.schedule_task(
        TaskSourceKind::Language {
            name: task.language_name.into(),
        },
        &task.template,
        task_context,
        false,
        window,
        cx,
    );
}

fn unsupported_language_message(language_name: Option<&str>) -> String {
    let supported = run::supported_languages().collect::<Vec<_>>().join(", ");
    match language_name {
        Some(language_name) => {
            format!("Cannot run {language_name} files. Supported languages: {supported}.")
        }
        None => format!(
            "Cannot run this file, as its language could not be detected. Supported languages: {supported}."
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::process::ExitStatus;
    use std::sync::{Arc, Mutex};

    use anyhow::Result as AnyResult;
    use editor::Editor;
    use gpui::{App, Entity, Task, TestAppContext};
    use language::{Language, LanguageConfig};
    use project::{FakeFs, Project};
    use serde_json::json;
    use task::{SaveStrategy, SpawnInTerminal};
    use ui::VisualContext;
    use util::{path, rel_path::rel_path};
    use workspace::{MultiWorkspace, TerminalProvider};

    use crate::tests::init_test;

    use super::*;

    struct RecordingTerminalProvider(Arc<Mutex<Vec<SpawnInTerminal>>>);

    impl TerminalProvider for RecordingTerminalProvider {
        fn spawn(
            &self,
            task: SpawnInTerminal,
            _window: &mut ui::Window,
            _cx: &mut App,
        ) -> Task<Option<AnyResult<ExitStatus>>> {
            self.0.lock().unwrap().push(task);
            Task::ready(Some(Ok(ExitStatus::default())))
        }
    }

    /// Opens `file_name` in an editor focused in `workspace`, using `language` for its language.
    async fn open_file(
        cx: &mut gpui::VisualTestContext,
        project: &Entity<Project>,
        workspace: &Entity<Workspace>,
        file_name: &str,
        language: Option<Language>,
    ) -> anyhow::Result<()> {
        let worktree_id = project.update(cx, |project, cx| {
            project.worktrees(cx).next().unwrap().read(cx).id()
        });
        let buffer = project
            .update(cx, |project, cx| {
                project.open_buffer((worktree_id, rel_path(file_name)), cx)
            })
            .await?;
        if let Some(language) = language {
            buffer.update(cx, |buffer, cx| {
                buffer.set_language(Some(Arc::new(language)), cx)
            });
        }
        let editor = cx.new_window_entity(|window, cx| {
            Editor::for_buffer(buffer, Some(project.clone()), window, cx)
        });
        workspace.update_in(cx, |workspace, window, cx| {
            workspace.add_item_to_center(Box::new(editor), window, cx);
        });
        Ok(())
    }

    #[gpui::test]
    async fn test_run_current_file_schedules_a_rustc_build(cx: &mut TestAppContext) {
        init_test(cx);
        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(path!("/dir"), json!({ "main.rs": "fn main() {}" }))
            .await;
        let project = Project::test(fs, [path!("/dir").as_ref()], cx).await;
        let (multi_workspace, cx) =
            cx.add_window_view(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = multi_workspace.read_with(cx, |mw, _| mw.workspace().clone());

        open_file(
            cx,
            &project,
            &workspace,
            "main.rs",
            Some(Language::new(
                LanguageConfig {
                    name: "Rust".into(),
                    ..Default::default()
                },
                Some(tree_sitter_rust::LANGUAGE.into()),
            )),
        )
        .await
        .expect("failed to open the test file");

        let spawned = Arc::new(Mutex::new(Vec::new()));
        workspace.update(cx, |workspace, _| {
            workspace.set_terminal_provider(RecordingTerminalProvider(spawned.clone()));
        });

        cx.dispatch_action(aaykra_actions::RunCurrentFile);
        cx.run_until_parked();

        let spawned = cx.read(|_| spawned.lock().unwrap().clone());
        assert_eq!(spawned.len(), 1, "expected exactly one task to be spawned");
        let spawned = &spawned[0];
        assert_eq!(spawned.label, "Run main.rs");
        assert_eq!(
            spawned.command.as_deref(),
            Some(r#"out="/dir/main"; rustc "/dir/main.rs" -o "$out" && "$out""#),
            "the recipe's `$ZED_*` variables should be substituted"
        );
        assert_eq!(spawned.cwd, Some(path!("/dir").into()));
        assert_eq!(spawned.save, SaveStrategy::Current);
        assert!(spawned.show_rerun, "the task must be rerunnable");
    }

    #[gpui::test]
    async fn test_run_current_file_falls_back_to_the_extension(cx: &mut TestAppContext) {
        init_test(cx);
        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(path!("/dir"), json!({ "Main.java": "class Main {}" }))
            .await;
        let project = Project::test(fs, [path!("/dir").as_ref()], cx).await;
        let (multi_workspace, cx) =
            cx.add_window_view(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = multi_workspace.read_with(cx, |mw, _| mw.workspace().clone());

        // Java is not a built-in language here, so the buffer has none and only the extension
        // identifies it.
        open_file(cx, &project, &workspace, "Main.java", None)
            .await
            .expect("failed to open the test file");

        let spawned = Arc::new(Mutex::new(Vec::new()));
        workspace.update(cx, |workspace, _| {
            workspace.set_terminal_provider(RecordingTerminalProvider(spawned.clone()));
        });

        cx.dispatch_action(aaykra_actions::RunCurrentFile);
        cx.run_until_parked();

        let spawned = cx.read(|_| spawned.lock().unwrap().clone());
        assert_eq!(spawned.len(), 1);
        assert_eq!(
            spawned[0].command.as_deref(),
            Some(r#"javac -d "/dir" "/dir/Main.java" && java -cp "/dir" "Main""#)
        );
    }

    #[gpui::test]
    async fn test_run_current_file_does_nothing_for_unsupported_languages(cx: &mut TestAppContext) {
        init_test(cx);
        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(path!("/dir"), json!({ "notes.md": "# hi" }))
            .await;
        let project = Project::test(fs, [path!("/dir").as_ref()], cx).await;
        let (multi_workspace, cx) =
            cx.add_window_view(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = multi_workspace.read_with(cx, |mw, _| mw.workspace().clone());

        open_file(cx, &project, &workspace, "notes.md", None)
            .await
            .expect("failed to open the test file");

        let spawned = Arc::new(Mutex::new(Vec::new()));
        workspace.update(cx, |workspace, _| {
            workspace.set_terminal_provider(RecordingTerminalProvider(spawned.clone()));
        });

        cx.dispatch_action(aaykra_actions::RunCurrentFile);
        cx.run_until_parked();

        assert!(
            spawned.lock().unwrap().is_empty(),
            "an unsupported language must not spawn a task"
        );
    }
}
