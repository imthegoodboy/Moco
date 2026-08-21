use crate::inference::{RuntimeManager, emit_generation, emit_phase};
use crate::models::{AppSettings, GenerationEvent};
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::process::Command;

const MAX_AGENT_STEPS: usize = 8;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOOL_OUTPUT: usize = 48_000;

pub async fn run_agent(
    runtime: &Arc<RuntimeManager>,
    app: &AppHandle,
    generation_id: &str,
    conversation_id: &str,
    mut messages: Vec<Value>,
    settings: &AppSettings,
    workspace: &Path,
) -> Result<String> {
    let workspace = workspace
        .canonicalize()
        .context("Moco could not access the current user's files")?;
    if !workspace.is_dir() {
        bail!("Moco could not access the current user's files.");
    }
    let request = messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let change_requested = [
        "build",
        "create",
        "add",
        "fix",
        "change",
        "update",
        "edit",
        "implement",
        "refactor",
        "remove",
        "write",
    ]
    .iter()
    .any(|keyword| {
        request
            .split(|character: char| !character.is_alphanumeric())
            .any(|word| word == *keyword)
    });
    let inventory = list_files(&workspace, "", 2)
        .unwrap_or_else(|error| format!("Desktop file map unavailable: {error}"));
    messages[0]["content"] = Value::String(format!(
        "{}\n\nDesktop file root: {}\nAnswer normal questions directly. Use tools only when the request needs desktop facts or changes. Tool paths are relative to the current user's profile directory; for example Desktop/notes.txt. Never claim an action unless a tool result confirms it. If native function calling is unavailable, output only JSON in this form: {{\"tool\":\"read_file\",\"arguments\":{{\"path\":\"Desktop/notes.txt\"}}}}. Valid tool names: list_files, read_file, search_files, replace_text, create_file, run_check. When no tool is needed or the work is complete, answer normally.",
        messages[0]["content"].as_str().unwrap_or_default(),
        workspace.display()
    ));
    messages.insert(1, json!({
        "role": "system",
        "content": format!("Initial desktop file map (automatically gathered):\n{}", truncate(&inventory, 14_000))
    }));
    let tools = tool_definitions();
    let mut compatibility_attempts = 0usize;
    let mut tool_completed = false;
    let mut mutation_completed = false;

    for _ in 0..MAX_AGENT_STEPS {
        emit_phase(
            app,
            generation_id,
            conversation_id,
            "Agent · Planning the next action",
            &[],
        );
        let assistant = runtime
            .complete_with_tools(&messages, settings, &tools)
            .await?;
        let calls = assistant
            .get("tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let content = assistant
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        messages.push(assistant);

        if calls.is_empty() {
            if let Some((name, arguments)) = compatibility_action(&content) {
                let arguments = normalize_tool_arguments(&name, arguments, &request);
                let label = tool_label(&name, &arguments);
                emit_phase(
                    app,
                    generation_id,
                    conversation_id,
                    &format!("Agent · {label}"),
                    &[],
                );
                let result = match execute_tool(&name, &arguments, &workspace).await {
                    Ok(output) => {
                        tool_completed = true;
                        if matches!(name.as_str(), "replace_text" | "create_file") {
                            mutation_completed = true;
                        }
                        output
                    }
                    Err(error) => format!("Tool error: {error}"),
                };
                messages.push(json!({
                    "role": "user",
                    "content": format!("Tool result for {name}:\n{}\nChoose the next tool or provide the final answer.", truncate(&result, MAX_TOOL_OUTPUT))
                }));
                continue;
            }
            if content.is_empty() {
                bail!(
                    "The model did not return a tool action or a final response. Try a stronger coding model from Models."
                );
            }
            if !tool_completed {
                if compatibility_attempts < 2 {
                    compatibility_attempts += 1;
                    messages.push(json!({
                        "role": "user",
                        "content": "This request requires a real desktop tool result. Choose exactly one next action. Return JSON only: {\"tool\":\"list_files|read_file|search_files|replace_text|create_file|run_check\",\"arguments\":{...}}"
                    }));
                    continue;
                }
                bail!(
                    "The selected model could not choose a desktop tool. Try a stronger model from Models."
                );
            }
            if change_requested && !mutation_completed && compatibility_attempts < 2 {
                compatibility_attempts += 1;
                messages.push(json!({
                    "role": "user",
                    "content": "You have not performed the requested desktop change. Choose exactly one next action. Return JSON only: {\"tool\":\"list_files|read_file|search_files|replace_text|create_file|run_check\",\"arguments\":{...}}"
                }));
                continue;
            }
            emit_complete(app, generation_id, conversation_id, &content);
            return Ok(content);
        }

        for call in calls {
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("moco-tool")
                .to_string();
            let name = call
                .pointer("/function/name")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("The model returned a tool call without a name."))?;
            let raw_arguments = call
                .pointer("/function/arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let arguments = if let Some(raw) = raw_arguments.as_str() {
                serde_json::from_str(raw).context("The model returned invalid tool arguments")?
            } else {
                raw_arguments
            };
            let arguments = normalize_tool_arguments(name, arguments, &request);
            let label = tool_label(name, &arguments);
            emit_phase(
                app,
                generation_id,
                conversation_id,
                &format!("Agent · {label}"),
                &[],
            );
            let result = match execute_tool(name, &arguments, &workspace).await {
                Ok(output) => {
                    tool_completed = true;
                    if matches!(name, "replace_text" | "create_file") {
                        mutation_completed = true;
                    }
                    output
                }
                Err(error) => format!("Tool error: {error}"),
            };
            messages.push(json!({
                "role": "tool",
                "tool_call_id": id,
                "content": truncate(&result, MAX_TOOL_OUTPUT),
            }));
        }
    }

    messages.push(json!({
        "role": "user",
        "content": "Stop using tools now. Summarize the completed work, validation, and any remaining issue concisely."
    }));
    let assistant = runtime
        .complete_with_tools(&messages, settings, &[])
        .await?;
    let content = assistant
        .get("content")
        .and_then(Value::as_str)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| anyhow!("The model did not produce a final response."))?
        .to_string();
    emit_complete(app, generation_id, conversation_id, &content);
    Ok(content)
}

fn emit_complete(app: &AppHandle, generation_id: &str, conversation_id: &str, content: &str) {
    emit_generation(
        app,
        GenerationEvent {
            generation_id: generation_id.into(),
            conversation_id: conversation_id.into(),
            delta: content.into(),
            content: content.into(),
            phase: "complete".into(),
            done: true,
            error: None,
            sources: vec![],
            tokens_per_second: None,
        },
    );
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "list_files",
            "List files and folders available under the current user's profile. Use an empty path for the profile root.",
            json!({"type":"object","properties":{"path":{"type":"string"},"depth":{"type":"integer","minimum":1,"maximum":6}},"required":[]}),
        ),
        tool(
            "read_file",
            "Read a UTF-8 text file with line numbers. Read focused ranges when possible.",
            json!({"type":"object","properties":{"path":{"type":"string"},"start_line":{"type":"integer","minimum":1},"end_line":{"type":"integer","minimum":1}},"required":["path"]}),
        ),
        tool(
            "search_files",
            "Search UTF-8 files for literal text and return matching lines.",
            json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string"},"extension":{"type":"string"}},"required":["query"]}),
        ),
        tool(
            "replace_text",
            "Replace one exact, unique text block in an existing file. Fails if the old text is absent or appears more than once.",
            json!({"type":"object","properties":{"path":{"type":"string"},"old_text":{"type":"string"},"new_text":{"type":"string"}},"required":["path","old_text","new_text"]}),
        ),
        tool(
            "create_file",
            "Create a new UTF-8 file. Refuses to overwrite an existing file.",
            json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}),
        ),
        tool(
            "run_check",
            "Run a safe validation command in a chosen folder under the current user's profile. Supported families: cargo check/test/fmt/clippy, npm/pnpm/yarn test or run, git status/diff, dotnet build/test, pytest, and go test.",
            json!({"type":"object","properties":{"command":{"type":"string"},"path":{"type":"string","description":"Folder relative to the user profile, such as Desktop/MyProject"}},"required":["command"]}),
        ),
    ]
}

fn tool(name: &str, description: &str, parameters: Value) -> Value {
    json!({
        "type": "function",
        "function": { "name": name, "description": description, "parameters": parameters }
    })
}

fn compatibility_action(content: &str) -> Option<(String, Value)> {
    let mut candidate = content.trim();
    if candidate.starts_with("```") {
        candidate = candidate
            .strip_prefix("```json")
            .or_else(|| candidate.strip_prefix("```"))?
            .trim();
        candidate = candidate.strip_suffix("```").unwrap_or(candidate).trim();
    }
    let value: Value = serde_json::from_str(candidate).ok()?;
    let raw_name = value
        .get("tool")
        .or_else(|| value.get("action"))
        .and_then(Value::as_str)?
        .to_ascii_lowercase();
    let name = match raw_name.as_str() {
        "list_files" | "list" | "inspect" | "scan" => "list_files",
        "read_file" | "read" | "open" => "read_file",
        "search_files" | "search" | "find" => "search_files",
        "replace_text" | "replace" | "edit" | "update" => "replace_text",
        "create_file" | "create" | "write" => "create_file",
        "run_check" | "run" | "check" | "test" => "run_check",
        _ => return None,
    };
    let arguments = value
        .get("arguments")
        .or_else(|| value.get("args"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    Some((name.into(), arguments))
}

fn normalize_tool_arguments(name: &str, mut arguments: Value, request: &str) -> Value {
    if name == "list_files"
        && request
            .split(|character: char| !character.is_alphanumeric())
            .any(|word| word == "desktop")
    {
        let path = arguments
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if path.is_empty() || matches!(path, "." | "./" | ".\\") {
            arguments["path"] = Value::String("Desktop".into());
        }
    }
    arguments
}

fn tool_label(name: &str, arguments: &Value) -> String {
    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match name {
        "list_files" => format!(
            "Listing {}",
            if path.is_empty() { "your files" } else { path }
        ),
        "read_file" => format!("Reading {path}"),
        "search_files" => format!(
            "Searching for “{}”",
            arguments
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        "replace_text" => format!("Editing {path}"),
        "create_file" => format!("Creating {path}"),
        "run_check" => format!(
            "Running {}",
            arguments
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        _ => format!("Using {name}"),
    }
}

async fn execute_tool(name: &str, arguments: &Value, workspace: &Path) -> Result<String> {
    match name {
        "list_files" => list_files(
            workspace,
            string_arg(arguments, "path").unwrap_or_default(),
            arguments
                .get("depth")
                .and_then(Value::as_u64)
                .unwrap_or(3)
                .clamp(1, 6) as usize,
        ),
        "read_file" => read_file(
            workspace,
            required_string(arguments, "path")?,
            arguments
                .get("start_line")
                .and_then(Value::as_u64)
                .unwrap_or(1) as usize,
            arguments
                .get("end_line")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
        ),
        "search_files" => search_files(
            workspace,
            required_string(arguments, "query")?,
            string_arg(arguments, "path").unwrap_or_default(),
            string_arg(arguments, "extension"),
        ),
        "replace_text" => replace_text(
            workspace,
            required_string(arguments, "path")?,
            required_string(arguments, "old_text")?,
            required_string(arguments, "new_text")?,
        ),
        "create_file" => create_file(
            workspace,
            required_string(arguments, "path")?,
            required_string(arguments, "content")?,
        ),
        "run_check" => {
            let working_directory =
                resolve_existing(workspace, string_arg(arguments, "path").unwrap_or_default())?;
            if !working_directory.is_dir() {
                bail!("The validation path must be a folder.");
            }
            run_check(&working_directory, required_string(arguments, "command")?).await
        }
        _ => bail!("Unknown tool: {name}"),
    }
}

fn list_files(workspace: &Path, relative: &str, depth: usize) -> Result<String> {
    let root = resolve_existing(workspace, relative)?;
    if !root.is_dir() {
        bail!("The requested path is not a folder.");
    }
    let mut files = Vec::new();
    collect_files(&root, workspace, depth, &mut files)?;
    files.sort();
    files.truncate(1200);
    Ok(if files.is_empty() {
        "No files found.".into()
    } else {
        files.join("\n")
    })
}

fn collect_files(
    directory: &Path,
    workspace: &Path,
    depth: usize,
    output: &mut Vec<String>,
) -> Result<()> {
    if depth == 0 || output.len() >= 1200 {
        return Ok(());
    }
    let mut entries = std::fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let kind = entry.file_type()?;
        if kind.is_symlink() || (kind.is_dir() && ignored_directory(&name)) {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(workspace)
            .unwrap_or(&path)
            .to_string_lossy();
        output.push(format!(
            "{}{}",
            relative,
            if kind.is_dir() { "/" } else { "" }
        ));
        if kind.is_dir() {
            collect_files(&path, workspace, depth - 1, output)?;
        }
    }
    Ok(())
}

fn read_file(
    workspace: &Path,
    relative: &str,
    start_line: usize,
    end_line: Option<usize>,
) -> Result<String> {
    let path = resolve_existing(workspace, relative)?;
    let metadata = path.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        bail!("Only text files up to 2 MB can be read.");
    }
    let text = std::fs::read_to_string(&path).context("The file is not valid UTF-8 text")?;
    let start = start_line.max(1);
    let end = end_line.unwrap_or(start + 399).max(start).min(start + 799);
    let selected = text
        .lines()
        .enumerate()
        .filter(|(index, _)| index + 1 >= start && *index < end)
        .map(|(index, line)| format!("{:>5} | {line}", index + 1))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        bail!("The requested line range is outside the file.");
    }
    Ok(selected.join("\n"))
}

fn search_files(
    workspace: &Path,
    query: &str,
    relative: &str,
    extension: Option<&str>,
) -> Result<String> {
    if query.trim().is_empty() {
        bail!("Search text cannot be empty.");
    }
    let root = resolve_existing(workspace, relative)?;
    let mut paths = Vec::new();
    if root.is_file() {
        paths.push(root);
    } else {
        collect_search_paths(&root, 8, &mut paths)?;
    }
    let needle = query.to_lowercase();
    let normalized_extension = extension.map(|value| value.trim_start_matches('.').to_lowercase());
    let mut matches = Vec::new();
    for path in paths.into_iter().take(4000) {
        if let Some(expected) = &normalized_extension
            && path
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_lowercase)
                .as_deref()
                != Some(expected)
        {
            continue;
        }
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        if metadata.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&needle) {
                let relative = path.strip_prefix(workspace).unwrap_or(&path).display();
                matches.push(format!("{relative}:{}: {}", index + 1, line.trim()));
                if matches.len() >= 240 {
                    break;
                }
            }
        }
        if matches.len() >= 240 {
            break;
        }
    }
    Ok(if matches.is_empty() {
        "No matches found.".into()
    } else {
        matches.join("\n")
    })
}

fn collect_search_paths(directory: &Path, depth: usize, output: &mut Vec<PathBuf>) -> Result<()> {
    if depth == 0 || output.len() >= 4000 {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name().to_string_lossy().to_string();
        if kind.is_symlink() || (kind.is_dir() && ignored_directory(&name)) {
            continue;
        }
        if kind.is_dir() {
            collect_search_paths(&entry.path(), depth - 1, output)?;
        } else if kind.is_file() {
            output.push(entry.path());
        }
    }
    Ok(())
}

fn replace_text(
    workspace: &Path,
    relative: &str,
    old_text: &str,
    new_text: &str,
) -> Result<String> {
    if old_text.is_empty() {
        bail!("old_text cannot be empty.");
    }
    let path = resolve_existing(workspace, relative)?;
    let metadata = path.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        bail!("Only text files up to 2 MB can be edited.");
    }
    let text = std::fs::read_to_string(&path).context("The file is not valid UTF-8 text")?;
    let count = text.matches(old_text).count();
    if count != 1 {
        bail!("Expected old_text exactly once, but found {count} matches.");
    }
    std::fs::write(&path, text.replacen(old_text, new_text, 1))?;
    Ok(format!("Updated {relative}."))
}

fn create_file(workspace: &Path, relative: &str, content: &str) -> Result<String> {
    let path = resolve_new(workspace, relative)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    use std::io::Write;
    let mut file = options
        .write(true)
        .create_new(true)
        .open(&path)
        .context("The file already exists")?;
    file.write_all(content.as_bytes())?;
    Ok(format!("Created {relative}."))
}

async fn run_check(workspace: &Path, raw: &str) -> Result<String> {
    let parts = raw.split_whitespace().collect::<Vec<_>>();
    if parts.is_empty() {
        bail!("Command cannot be empty.");
    }
    validate_check_command(&parts)?;
    let program = parts[0].to_ascii_lowercase();
    let executable = match program.as_str() {
        "npm" => "npm.cmd",
        "pnpm" => "pnpm.cmd",
        "yarn" => "yarn.cmd",
        other => other,
    };
    let mut command = Command::new(executable);
    command
        .args(&parts[1..])
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let output = tokio::time::timeout(Duration::from_secs(120), command.output())
        .await
        .map_err(|_| anyhow!("Validation timed out after 120 seconds."))??;
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.stderr.is_empty() {
        combined.push('\n');
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    let status = if output.status.success() {
        "passed"
    } else {
        "failed"
    };
    Ok(format!(
        "Command {status} ({}).\n{}",
        output.status,
        truncate(&combined, 30_000)
    ))
}

fn validate_check_command(parts: &[&str]) -> Result<()> {
    let first = parts[0].to_ascii_lowercase();
    let second = parts
        .get(1)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let allowed = match first.as_str() {
        "cargo" => matches!(second.as_str(), "check" | "test" | "fmt" | "clippy"),
        "npm" | "pnpm" | "yarn" => second == "test" || (second == "run" && parts.len() >= 3),
        "git" => matches!(second.as_str(), "status" | "diff"),
        "dotnet" => matches!(second.as_str(), "build" | "test"),
        "pytest" => true,
        "python" => {
            second == "-m"
                && parts
                    .get(2)
                    .is_some_and(|value| value.eq_ignore_ascii_case("pytest"))
        }
        "go" => second == "test",
        _ => false,
    };
    if !allowed {
        bail!("That command is not in Moco's validation allowlist.");
    }
    if parts
        .iter()
        .any(|part| part.contains([';', '&', '|', '>', '<', '`']) || Path::new(part).is_absolute())
    {
        bail!("Shell operators and absolute paths are not allowed.");
    }
    Ok(())
}

fn resolve_existing(workspace: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    let path = workspace
        .join(relative)
        .canonicalize()
        .context("Path does not exist")?;
    if !path.starts_with(workspace) {
        bail!("Path is outside the current user's accessible file area.");
    }
    Ok(path)
}

fn resolve_new(workspace: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative(relative)?;
    if relative.trim().is_empty() {
        bail!("A file path is required.");
    }
    let path = workspace.join(relative);
    let parent = path.parent().ok_or_else(|| anyhow!("Invalid file path"))?;
    let existing_parent = nearest_existing_parent(parent)?;
    if !existing_parent.canonicalize()?.starts_with(workspace) {
        bail!("Path is outside the current user's accessible file area.");
    }
    Ok(path)
}

fn nearest_existing_parent(mut path: &Path) -> Result<&Path> {
    loop {
        if path.exists() {
            return Ok(path);
        }
        path = path
            .parent()
            .ok_or_else(|| anyhow!("No valid parent folder"))?;
    }
}

fn validate_relative(relative: &str) -> Result<()> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("Use a relative path inside the current user's file area.");
    }
    Ok(())
}

fn ignored_directory(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git"
            | "appdata"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".venv"
            | "venv"
    )
}

fn required_string<'a>(arguments: &'a Value, key: &str) -> Result<&'a str> {
    string_arg(arguments, key).ok_or_else(|| anyhow!("Missing required argument: {key}"))
}

fn string_arg<'a>(arguments: &'a Value, key: &str) -> Option<&'a str> {
    arguments.get(key).and_then(Value::as_str)
}

fn truncate(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n…output truncated…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_workspace_escape() {
        assert!(validate_relative("../secret.txt").is_err());
        assert!(validate_relative("src/main.rs").is_ok());
    }

    #[test]
    fn restricts_validation_commands() {
        assert!(validate_check_command(&["cargo", "test"]).is_ok());
        assert!(validate_check_command(&["git", "status", "--short"]).is_ok());
        assert!(validate_check_command(&["powershell", "Remove-Item", "x"]).is_err());
        assert!(validate_check_command(&["npm", "test", "&&", "whoami"]).is_err());
    }

    #[test]
    fn normalizes_desktop_list_target_for_small_models() {
        let arguments = normalize_tool_arguments(
            "list_files",
            json!({ "path": ".", "depth": 1 }),
            "list the files on my desktop",
        );
        assert_eq!(arguments["path"], "Desktop");
    }
}
