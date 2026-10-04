//! Recipes backing the `task::RunCurrentFile` action.
//!
//! Each recipe is a shell snippet that is handed to the task system as-is, so it goes through
//! the same terminal, rerun history and output handling as any task defined in `tasks.json`.
//! The snippets refer to `$ZED_FILE`, `$ZED_DIRNAME` and `$ZED_STEM` instead of baking in paths,
//! which keeps a recipe's task id stable across reruns and lets `reevaluate_context` pick up the
//! file the cursor is in.

use crate::{SaveStrategy, TaskTemplate};

/// A task that builds and runs the current file, along with the language it was picked for.
pub struct RunCurrentFileTask {
    /// Name of the language the command was selected for, e.g. `"C++"`.
    ///
    /// This is the recipe's canonical name, so it is also available when the language was
    /// recognized from the file extension alone.
    pub language_name: String,
    pub template: TaskTemplate,
}

/// Human readable name of every language a run command exists for.
pub fn supported_languages() -> impl Iterator<Item = &'static str> {
    ["C", "C++", "Go", "Java", "JavaScript", "Python", "Rust"].into_iter()
}

/// Picks the run command for the active file.
///
/// The buffer's language is authoritative; the extension is only consulted when no language
/// matched, which keeps single files working for languages that are provided by an extension
/// (Java being the only one here that is not built in).
pub fn run_current_file_task(
    language_name: Option<&str>,
    file_extension: Option<&str>,
) -> Option<RunCurrentFileTask> {
    let key = language_name
        .and_then(key_for_language_name)
        .or_else(|| file_extension.and_then(key_for_extension))?;

    Some(RunCurrentFileTask {
        language_name: language_name_for_key(key).to_string(),
        template: TaskTemplate {
            label: "Run $ZED_FILENAME".to_string(),
            command: command_for_key(key),
            // The file has to be on disk before the command that reads it is started.
            save: SaveStrategy::Current,
            // Reuse this language's terminal and restart it, rather than queueing behind a
            // still-running program or opening yet another tab.
            allow_concurrent_runs: true,
            ..TaskTemplate::default()
        },
    })
}

fn key_for_language_name(language_name: &str) -> Option<&'static str> {
    match language_name {
        "C" => Some("c"),
        "C++" => Some("cpp"),
        "Go" => Some("go"),
        "Java" => Some("java"),
        "JavaScript" => Some("javascript"),
        "Python" => Some("python"),
        "Rust" => Some("rust"),
        _ => None,
    }
}

fn key_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "c" => Some("c"),
        "cc" | "cpp" | "cxx" | "c++" => Some("cpp"),
        "go" => Some("go"),
        "java" => Some("java"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "py" => Some("python"),
        "rs" => Some("rust"),
        _ => None,
    }
}

fn language_name_for_key(key: &str) -> &'static str {
    match key {
        "c" => "C",
        "cpp" => "C++",
        "go" => "Go",
        "java" => "Java",
        "javascript" => "JavaScript",
        "python" => "Python",
        "rust" => "Rust",
        other => panic!("unknown run recipe key: {other}"),
    }
}

fn command_for_key(key: &str) -> String {
    match key {
        "c" => compile_and_run("gcc"),
        "cpp" => compile_and_run("g++"),
        // A standalone `rustc` invocation matches the "run the file I am looking at" intent,
        // whereas `cargo run` would ignore the current file entirely.
        "rust" => compile_and_run("rustc"),
        // Java's entry point is the public class sharing the file's name, which `$ZED_STEM`
        // already holds. `-d` keeps the generated classes next to the source so that the
        // source directory doubles as the classpath.
        "java" => concat!(
            r#"javac -d "$ZED_DIRNAME" "$ZED_FILE" && "#,
            r#"java -cp "$ZED_DIRNAME" "$ZED_STEM""#
        )
        .to_string(),
        "go" => r#"go run "$ZED_FILE""#.to_string(),
        "javascript" => r#"node "$ZED_FILE""#.to_string(),
        "python" => {
            if cfg!(windows) {
                r#"python "$ZED_FILE""#.to_string()
            } else {
                r#"python3 "$ZED_FILE""#.to_string()
            }
        }
        other => panic!("unknown run recipe key: {other}"),
    }
}

/// Compiles `$ZED_FILE` into a sibling of the source file and runs it.
///
/// The artifact is named after the source and placed next to it, the way Dev-C++ does it, so
/// that `$ZED_DIRNAME` and `$ZED_STEM` are enough to address it. `&&` makes the shell skip the
/// run when compilation fails, leaving the compiler's diagnostics as the last thing on screen.
fn compile_and_run(compiler: &str) -> String {
    format!(r#"out="$ZED_DIRNAME/$ZED_STEM"; {compiler} "$ZED_FILE" -o "$out" && "$out""#)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TaskContext, TaskVariables, VariableName};

    fn resolved_command(language_name: Option<&str>, extension: Option<&str>) -> Option<String> {
        let task = run_current_file_task(language_name, extension)?;
        Some(task.template.command)
    }

    #[test]
    fn test_language_names_map_to_commands() {
        assert_eq!(
            resolved_command(Some("C"), None).unwrap(),
            r#"out="$ZED_DIRNAME/$ZED_STEM"; gcc "$ZED_FILE" -o "$out" && "$out""#
        );
        assert_eq!(
            resolved_command(Some("C++"), None).unwrap(),
            r#"out="$ZED_DIRNAME/$ZED_STEM"; g++ "$ZED_FILE" -o "$out" && "$out""#
        );
        assert_eq!(
            resolved_command(Some("Rust"), None).unwrap(),
            r#"out="$ZED_DIRNAME/$ZED_STEM"; rustc "$ZED_FILE" -o "$out" && "$out""#
        );
        assert_eq!(
            resolved_command(Some("JavaScript"), None).unwrap(),
            r#"node "$ZED_FILE""#
        );
        assert_eq!(
            resolved_command(Some("Go"), None).unwrap(),
            r#"go run "$ZED_FILE""#
        );
        assert_eq!(
            resolved_command(Some("Java"), None).unwrap(),
            r#"javac -d "$ZED_DIRNAME" "$ZED_FILE" && java -cp "$ZED_DIRNAME" "$ZED_STEM""#
        );
        assert!(
            resolved_command(Some("Python"), None)
                .unwrap()
                .ends_with(r#"python3 "$ZED_FILE""#)
                || resolved_command(Some("Python"), None)
                    .unwrap()
                    .ends_with(r#"python "$ZED_FILE""#)
        );
    }

    #[test]
    fn test_extension_is_used_when_no_language_matched() {
        assert_eq!(
            resolved_command(Some("Plain Text"), Some("rs")).unwrap(),
            r#"out="$ZED_DIRNAME/$ZED_STEM"; rustc "$ZED_FILE" -o "$out" && "$out""#
        );
        assert_eq!(
            resolved_command(Some("Plain Text"), Some("java")).unwrap(),
            r#"javac -d "$ZED_DIRNAME" "$ZED_FILE" && java -cp "$ZED_DIRNAME" "$ZED_STEM""#
        );
        assert_eq!(
            resolved_command(None, Some("cpp")).unwrap(),
            r#"out="$ZED_DIRNAME/$ZED_STEM"; g++ "$ZED_FILE" -o "$out" && "$out""#
        );
    }

    #[test]
    fn test_language_wins_over_extension() {
        assert_eq!(
            resolved_command(Some("C"), Some("cpp")).unwrap(),
            r#"out="$ZED_DIRNAME/$ZED_STEM"; gcc "$ZED_FILE" -o "$out" && "$out""#
        );
    }

    #[test]
    fn test_unsupported_languages_are_rejected() {
        assert!(resolved_command(Some("Markdown"), None).is_none());
        assert!(resolved_command(Some("Ruby"), Some("rb")).is_none());
        assert!(resolved_command(None, None).is_none());
        assert!(resolved_command(None, Some("txt")).is_none());
    }

    #[test]
    fn test_task_saves_current_buffer_and_reuses_terminal() {
        let task = run_current_file_task(Some("Go"), None).unwrap();
        assert_eq!(task.language_name, "Go");
        assert_eq!(task.template.label, "Run $ZED_FILENAME");
        assert_eq!(task.template.save, SaveStrategy::Current);
        assert!(task.template.allow_concurrent_runs);
        assert!(!task.template.use_new_terminal);
    }

    #[test]
    fn test_recipes_resolve_against_a_real_task_context() {
        let context = TaskContext {
            cwd: Some("/dir".into()),
            task_variables: TaskVariables::from_iter([
                (VariableName::File, "/dir/main.c".into()),
                (VariableName::Dirname, "/dir".into()),
                (VariableName::Filename, "main.c".into()),
                (VariableName::Stem, "main".into()),
            ]),
            project_env: Default::default(),
        };

        for language_name in supported_languages() {
            let task = run_current_file_task(Some(language_name), None)
                .unwrap_or_else(|| panic!("no recipe for {language_name}"));
            let resolved = task
                .template
                .resolve_task("test", &context)
                .unwrap_or_else(|| panic!("{language_name} recipe did not resolve"));

            assert_eq!(resolved.display_label(), "Run main.c");
            assert!(!resolved.resolved.command_label.contains("$ZED_"));
        }
    }

    #[test]
    fn test_recipes_reject_templates_with_missing_variables() {
        // `resolve_task` drops templates referencing `$ZED_*` variables that the context does
        // not define, so recipes must only use variables a file-backed buffer provides.
        let template = run_current_file_task(Some("C"), None).unwrap().template;
        assert!(
            template
                .resolve_task("test", &TaskContext::default())
                .is_none()
        );
    }
}
