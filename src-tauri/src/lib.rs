mod agent;
mod documents;
mod downloads;
mod inference;
mod models;
mod rag;
mod storage;

use crate::documents::{extract, supported_extension};
use crate::downloads::DownloadManager;
use crate::inference::{RuntimeManager, chat_messages, emit_error, emit_phase};
use crate::models::*;
use crate::rag::{build_context, chunk_text};
use crate::storage::Database;
use anyhow::{Context, Result, anyhow, bail};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sysinfo::{Disks, System};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

const CORE_PROMPT: &str = include_str!("../../.agents/prompts/system.md");
const SUMMARIZE_PROMPT: &str = include_str!("../../.agents/prompts/summarize.md");
const RESEARCH_PROMPT: &str = include_str!("../../.agents/prompts/research.md");
const NEWS_PROMPT: &str = include_str!("../../.agents/prompts/news.md");
const GRAMMAR_PROMPT: &str = include_str!("../../.agents/prompts/grammar.md");
const REWRITE_PROMPT: &str = include_str!("../../.agents/prompts/rewrite.md");
const EXPLAIN_PROMPT: &str = include_str!("../../.agents/prompts/explain.md");
const COMPARE_PROMPT: &str = include_str!("../../.agents/prompts/compare.md");
const AGENT_PROMPT: &str = include_str!("../../.agents/prompts/agent.md");
const DOCUMENTS_PROMPT: &str = include_str!("../../.agents/prompts/documents.md");

struct AppCore {
    database: Database,
    runtime: Arc<RuntimeManager>,
    data_directory: PathBuf,
    session_api_key: Mutex<String>,
    downloads: DownloadManager,
}

impl AppCore {
    fn current_settings(&self) -> Result<AppSettings> {
        let mut settings = self.database.settings()?;
        if settings.api_key.is_empty() {
            settings.api_key = self.session_api_key.lock().clone();
        }
        Ok(settings)
    }
}

type CoreState<'a> = State<'a, Arc<AppCore>>;

fn command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn resource_candidate(app: &AppHandle, relative: &str) -> PathBuf {
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|path| path.join(relative));
    let development = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    bundled.filter(|path| path.exists()).unwrap_or(development)
}

fn hardware_info() -> HardwareInfo {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list();
    let cpu = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".into());
    let available_disk_bytes = disks
        .iter()
        .map(|disk| disk.available_space())
        .max()
        .unwrap_or(0);
    let (gpu, gpu_vram_bytes) = windows_gpu_info();
    let total_ram_bytes = system.total_memory();
    let compatibility = if total_ram_bytes >= 2 * 1024 * 1024 * 1024 {
        "Fully supported"
    } else if total_ram_bytes >= 1024 * 1024 * 1024 {
        "May run slowly"
    } else {
        "Not enough memory"
    };
    HardwareInfo {
        os: format!(
            "{} {}",
            System::name().unwrap_or_else(|| "Windows".into()),
            System::os_version().unwrap_or_default()
        )
        .trim()
        .to_string(),
        cpu,
        physical_cores: System::physical_core_count().unwrap_or(system.cpus().len()),
        logical_cores: system.cpus().len(),
        total_ram_bytes,
        available_ram_bytes: system.available_memory(),
        gpu,
        gpu_vram_bytes,
        available_disk_bytes,
        acceleration: "CPU (GPU layers optional)".into(),
        compatibility: compatibility.into(),
    }
}

fn windows_gpu_info() -> (String, u64) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-CimInstance Win32_VideoController | Select-Object -First 1 Name,AdapterRAM | ConvertTo-Json -Compress",
            ])
            .creation_flags(0x08000000)
            .output();
        if let Ok(output) = output {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                let name = value
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Windows GPU");
                let ram = value
                    .get("AdapterRAM")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                return (name.into(), ram);
            }
        }
    }
    ("Not reported".into(), 0)
}

#[tauri::command]
fn bootstrap(core: CoreState<'_>) -> Result<BootstrapData, String> {
    Ok(BootstrapData {
        conversations: core.database.conversations().map_err(command_error)?,
        messages: core.database.messages(None).map_err(command_error)?,
        documents: core.database.documents().map_err(command_error)?,
        models: core.database.models().map_err(command_error)?,
        settings: core.current_settings().map_err(command_error)?,
        hardware: hardware_info(),
        data_directory: core.data_directory.to_string_lossy().to_string(),
    })
}

#[tauri::command]
fn create_conversation(title: Option<String>, core: CoreState<'_>) -> Result<Conversation, String> {
    core.database
        .create_conversation(title.as_deref())
        .map_err(command_error)
}

#[tauri::command]
fn rename_conversation(id: String, title: String, core: CoreState<'_>) -> Result<(), String> {
    core.database
        .rename_conversation(&id, &title)
        .map_err(command_error)
}

#[tauri::command]
fn set_conversation_flag(
    id: String,
    flag: String,
    value: bool,
    core: CoreState<'_>,
) -> Result<(), String> {
    core.database
        .set_conversation_flag(&id, &flag, value)
        .map_err(command_error)
}

#[tauri::command]
fn delete_conversation(id: String, core: CoreState<'_>) -> Result<(), String> {
    core.database
        .delete_conversation(&id)
        .map_err(command_error)
}

#[tauri::command]
fn delete_message(id: String, core: CoreState<'_>) -> Result<(), String> {
    core.database.delete_message(&id).map_err(command_error)
}

#[tauri::command]
fn set_message_feedback(
    id: String,
    feedback: Option<String>,
    core: CoreState<'_>,
) -> Result<(), String> {
    core.database
        .set_message_feedback(&id, feedback.as_deref())
        .map_err(command_error)
}

#[tauri::command]
fn set_message_saved(id: String, saved: bool, core: CoreState<'_>) -> Result<(), String> {
    core.database
        .set_message_saved(&id, saved)
        .map_err(command_error)
}

#[tauri::command]
fn save_settings(mut settings: AppSettings, core: CoreState<'_>) -> Result<AppSettings, String> {
    *core.session_api_key.lock() = settings.api_key.clone();
    let visible = settings.clone();
    if !settings.remember_api_key {
        settings.api_key.clear();
    }
    core.database
        .save_settings(&settings)
        .map_err(command_error)?;
    Ok(visible)
}

#[tauri::command]
async fn generate(
    request: GenerateRequest,
    app: AppHandle,
    core: CoreState<'_>,
) -> Result<GenerationStarted, String> {
    if request.content.trim().is_empty() {
        return Err("Write a message before sending.".into());
    }
    let user_message = core
        .database
        .insert_message(
            &request.conversation_id,
            "user",
            request.content.trim(),
            &request.mode,
            &[],
        )
        .map_err(command_error)?;
    let generation_id = Uuid::new_v4().to_string();
    let generation_result = GenerationStarted {
        generation_id: generation_id.clone(),
        user_message: user_message.clone(),
    };

    let app_core = core.inner().clone();
    let conversation_id = request.conversation_id.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_generation(
            &app,
            &app_core,
            &generation_id,
            &conversation_id,
            &request.content,
            &request.mode,
            &request.tool,
            &request.document_ids,
            &user_message.id,
        )
        .await;
        if let Err(error) = result {
            emit_error(&app, &generation_id, &conversation_id, &error);
        }
    });
    Ok(generation_result)
}

async fn run_generation(
    app: &AppHandle,
    core: &Arc<AppCore>,
    generation_id: &str,
    conversation_id: &str,
    content: &str,
    mode: &str,
    tool: &str,
    document_ids: &[String],
    current_user_message_id: &str,
) -> Result<()> {
    let settings = core.current_settings()?;
    emit_phase(app, generation_id, conversation_id, "understanding", &[]);
    let sources = core.database.retrieve(content, document_ids, 5)?;
    if !sources.is_empty() {
        emit_phase(
            app,
            generation_id,
            conversation_id,
            "reading-documents",
            &sources,
        );
    }
    let model = core.database.default_model()?;
    if settings.provider == "local" {
        emit_phase(
            app,
            generation_id,
            conversation_id,
            "loading-model",
            &sources,
        );
        core.database.set_model_status(&model.id, "loading")?;
        if let Err(error) = core.runtime.ensure_running(&model, &settings).await {
            core.database.set_model_status(&model.id, "error")?;
            return Err(error);
        }
        core.database.set_model_status(&model.id, "loaded")?;
    } else {
        emit_phase(app, generation_id, conversation_id, "connecting", &sources);
    }

    let history = core.database.messages(Some(conversation_id))?;
    let use_desktop_tools = mode == "agent"
        && (tool == "desktop" || (tool == "auto" && request_needs_desktop_tools(content)));
    let prompt_mode = if mode == "agent" && !use_desktop_tools {
        "chat"
    } else {
        mode
    };
    let selected_prompt = selected_tool_prompt(tool);
    let system_prompt = if selected_prompt.is_empty() {
        compose_system_prompt(prompt_mode, &settings)
    } else {
        selected_prompt.to_string()
    };
    let user_content = format!("{}{}", content, build_context(&sources));
    let request_history = if tool == "auto" {
        history
            .into_iter()
            .filter(|message| message.id != current_user_message_id)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let messages = chat_messages(&request_history, &system_prompt, &user_content);
    if use_desktop_tools {
        let desktop_root = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| anyhow!("Moco could not locate the current user's files."))?;
        let output = agent::run_agent(
            &core.runtime,
            app,
            generation_id,
            conversation_id,
            messages,
            &settings,
            &desktop_root,
        )
        .await?;
        if !output.trim().is_empty() {
            core.database
                .insert_message(conversation_id, "assistant", &output, mode, &sources)?;
        }
        return Ok(());
    }
    emit_phase(app, generation_id, conversation_id, "generating", &sources);
    let output = core
        .runtime
        .stream_chat(
            app,
            generation_id,
            conversation_id,
            &messages,
            &settings,
            &sources,
        )
        .await?;
    if !output.trim().is_empty() {
        core.database
            .insert_message(conversation_id, "assistant", &output, mode, &sources)?;
    }
    Ok(())
}

fn request_needs_desktop_tools(content: &str) -> bool {
    let request = content.to_ascii_lowercase();
    let action = [
        "find", "list", "read", "open", "inspect", "search", "create", "write", "edit", "change",
        "replace", "fix", "build", "test", "run", "copy", "move", "rename",
    ]
    .iter()
    .any(|word| {
        request
            .split(|character: char| !character.is_alphanumeric())
            .any(|part| part == *word)
    });
    let target = [
        "desktop",
        "download",
        "downloads",
        "document",
        "documents",
        "file",
        "files",
        "folder",
        "directory",
        "repo",
        "repository",
        "codebase",
        "project",
        "readme",
    ]
    .iter()
    .any(|word| {
        request
            .split(|character: char| !character.is_alphanumeric())
            .any(|part| part == *word)
    });
    let looks_like_path = request.contains('\\')
        || request.contains('/')
        || [
            ".txt", ".md", ".json", ".toml", ".rs", ".ts", ".tsx", ".js", ".py",
        ]
        .iter()
        .any(|extension| request.contains(extension));
    action && (target || looks_like_path)
}

fn selected_tool_prompt(tool: &str) -> &'static str {
    match tool {
        "documents" => DOCUMENTS_PROMPT,
        "summarize" => SUMMARIZE_PROMPT,
        "research" => RESEARCH_PROMPT,
        "grammar" => GRAMMAR_PROMPT,
        "rewrite" => REWRITE_PROMPT,
        "explain" => EXPLAIN_PROMPT,
        _ => "",
    }
}

fn compose_system_prompt(mode: &str, settings: &AppSettings) -> String {
    let mode_prompt = match mode {
        "agent" => AGENT_PROMPT,
        "summarize" => SUMMARIZE_PROMPT,
        "research" => RESEARCH_PROMPT,
        "news" => NEWS_PROMPT,
        "grammar" => GRAMMAR_PROMPT,
        "rewrite" => REWRITE_PROMPT,
        "explain" => EXPLAIN_PROMPT,
        "compare" => COMPARE_PROMPT,
        _ => "",
    };
    format!(
        "{CORE_PROMPT}\n\n{mode_prompt}\n\nResponse style: {}. Response length: {}.\n{}\n{}",
        settings.response_style,
        settings.response_length,
        if settings.documents_only {
            "Use documents only is enabled."
        } else {
            "You may use general model knowledge."
        },
        if settings.custom_instructions.trim().is_empty() {
            String::new()
        } else {
            format!(
                "User's custom instructions:\n{}",
                settings.custom_instructions
            )
        }
    )
}

#[tauri::command]
fn stop_generation(generation_id: String, core: CoreState<'_>) -> bool {
    core.runtime.cancel(&generation_id)
}

#[tauri::command]
async fn unload_model(core: CoreState<'_>) -> Result<(), String> {
    core.runtime.stop_runtime().await;
    for model in core.database.models().map_err(command_error)? {
        core.database
            .set_model_status(&model.id, "unloaded")
            .map_err(command_error)?;
    }
    Ok(())
}

#[tauri::command]
async fn load_model(id: String, core: CoreState<'_>) -> Result<ModelInfo, String> {
    core.database
        .set_default_model(&id)
        .map_err(command_error)?;
    let model = core.database.default_model().map_err(command_error)?;
    let settings = core.current_settings().map_err(command_error)?;
    core.database
        .set_model_status(&id, "loading")
        .map_err(command_error)?;
    core.runtime
        .ensure_running(&model, &settings)
        .await
        .map_err(command_error)?;
    core.database
        .set_model_status(&id, "loaded")
        .map_err(command_error)?;
    let mut loaded = model;
    loaded.status = "loaded".into();
    Ok(loaded)
}

#[tauri::command]
fn import_model(path: String, core: CoreState<'_>) -> Result<ModelInfo, String> {
    let result = (|| -> Result<ModelInfo> {
        let source = PathBuf::from(&path);
        if source
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("gguf"))
            != Some(true)
        {
            bail!("Choose a GGUF model file.");
        }
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("Invalid model file name."))?;
        let models_dir = core.data_directory.join("models");
        std::fs::create_dir_all(&models_dir)?;
        let target = models_dir.join(file_name);
        std::fs::copy(&source, &target).context("The model could not be copied into Moco")?;
        let metadata = std::fs::metadata(&target)?;
        let lower = file_name.to_ascii_lowercase();
        let quantization = [
            "q2_k", "q3_k_m", "q4_0", "q4_k_m", "q5_k_m", "q6_k", "q8_0", "f16",
        ]
        .into_iter()
        .find(|value| lower.contains(value))
        .unwrap_or("unknown")
        .to_ascii_uppercase();
        let model = ModelInfo {
            id: Uuid::new_v4().to_string(),
            name: source
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Imported model")
                .into(),
            path: target.to_string_lossy().to_string(),
            parameters: "Imported GGUF".into(),
            quantization,
            context_length: 8192,
            size_bytes: metadata.len(),
            required_ram_bytes: (metadata.len() as f64 * 1.35) as u64,
            built_in: false,
            status: "unloaded".into(),
            is_default: false,
            download_url: None,
            sha256: None,
            description: "A GGUF model imported from this computer.".into(),
            capability_tier: "Imported".into(),
            best_for: "Depends on the imported model".into(),
        };
        core.database.insert_model(&model)?;
        core.database.audit("model.imported", &model.name)?;
        Ok(model)
    })();
    result.map_err(command_error)
}

#[tauri::command]
async fn download_model(
    id: String,
    app: AppHandle,
    core: CoreState<'_>,
) -> Result<ModelInfo, String> {
    let model = core
        .database
        .models()
        .map_err(command_error)?
        .into_iter()
        .find(|model| model.id == id)
        .ok_or_else(|| "Model not found.".to_string())?;
    core.downloads
        .download(&app, &core.database, &model)
        .await
        .map_err(command_error)
}

#[tauri::command]
fn pause_model_download(id: String, core: CoreState<'_>) -> bool {
    core.downloads.pause(&id)
}

#[tauri::command]
fn cancel_model_download(id: String, core: CoreState<'_>) -> bool {
    core.downloads.cancel(&id)
}

#[tauri::command]
fn delete_model(id: String, core: CoreState<'_>) -> Result<(), String> {
    core.database.delete_model(&id).map_err(command_error)
}

#[tauri::command]
async fn import_documents(
    paths: Vec<String>,
    app: AppHandle,
    core: CoreState<'_>,
) -> Result<Vec<DocumentInfo>, String> {
    let mut imported = Vec::new();
    let import_id = Uuid::new_v4().to_string();
    let mut files = Vec::new();
    let mut pending: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let child = entry.path();
                    if child.is_dir() || supported_extension(&child) {
                        pending.push(child);
                    }
                }
            }
        } else if supported_extension(&path) {
            files.push(path);
        }
    }
    if files.is_empty() {
        return Err("No supported documents were found. Choose PDF, DOCX, TXT, Markdown, CSV, or HTML files.".into());
    }
    files.sort();
    for (file_index, path) in files.iter().enumerate() {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Document")
            .to_string();
        let base_percent = ((file_index * 100) / files.len().max(1)) as u8;
        let _ = app.emit(
            "moco://import-progress",
            ImportProgress {
                import_id: import_id.clone(),
                file_name: file_name.clone(),
                phase: "reading".into(),
                percent: base_percent,
                error: None,
            },
        );
        let result = import_one_document(path, core.inner().as_ref());
        match result {
            Ok(document) => {
                imported.push(document);
                let _ = app.emit(
                    "moco://import-progress",
                    ImportProgress {
                        import_id: import_id.clone(),
                        file_name,
                        phase: "complete".into(),
                        percent: (((file_index + 1) * 100) / files.len().max(1)) as u8,
                        error: None,
                    },
                );
            }
            Err(error) => {
                let _ = app.emit(
                    "moco://import-progress",
                    ImportProgress {
                        import_id: import_id.clone(),
                        file_name,
                        phase: "error".into(),
                        percent: base_percent,
                        error: Some(error.to_string()),
                    },
                );
            }
        }
    }
    Ok(imported)
}

fn import_one_document(path: &Path, core: &AppCore) -> Result<DocumentInfo> {
    if !supported_extension(path) {
        bail!("Supported files: PDF, DOCX, TXT, Markdown, CSV, and HTML.");
    }
    let metadata = std::fs::metadata(path).context("The selected file could not be read")?;
    if metadata.len() > 100 * 1024 * 1024 {
        bail!("This file is larger than the 100 MB import limit.");
    }
    let pages = extract(path)?;
    let id = Uuid::new_v4().to_string();
    let documents_dir = core.data_directory.join("documents");
    std::fs::create_dir_all(&documents_dir)?;
    let original_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Invalid file name."))?;
    let target = documents_dir.join(format!("{}-{}", &id[..8], original_name));
    std::fs::copy(path, &target)
        .context("Moco could not copy this file into its private library")?;
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("file")
        .to_ascii_uppercase();
    let page_count = pages
        .iter()
        .filter_map(|page| page.page)
        .max()
        .unwrap_or(pages.len() as u32);
    let document = core.database.insert_document(
        &id,
        original_name,
        &target,
        &extension,
        metadata.len(),
        page_count,
    )?;
    let mut ordinal = 0usize;
    for page in pages {
        for chunk in chunk_text(&page.text, 1_400, 180) {
            core.database
                .insert_chunk(&id, page.page, &chunk, ordinal)?;
            ordinal += 1;
        }
    }
    Ok(document)
}

#[tauri::command]
fn delete_document(id: String, core: CoreState<'_>) -> Result<(), String> {
    core.database.delete_document(&id).map_err(command_error)
}

#[tauri::command]
fn export_conversation(
    id: String,
    path: String,
    format: String,
    core: CoreState<'_>,
) -> Result<(), String> {
    let result = (|| -> Result<()> {
        let conversation = core
            .database
            .conversations()?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| anyhow!("Conversation not found."))?;
        let messages = core.database.messages(Some(&id))?;
        let mut output = if format == "md" {
            format!("# {}\n\n", conversation.title)
        } else {
            format!("{}\n\n", conversation.title)
        };
        for message in messages {
            let label = if message.role == "user" {
                "You"
            } else {
                "Moco"
            };
            if format == "md" {
                output.push_str(&format!("## {label}\n\n{}\n\n", message.content));
            } else {
                output.push_str(&format!("{label}:\n{}\n\n", message.content));
            }
        }
        std::fs::write(&path, output).context("The export file could not be written")?;
        core.database
            .audit("conversation.exported", &conversation.title)?;
        Ok(())
    })();
    result.map_err(command_error)
}

#[tauri::command]
fn clear_data(scope: String, core: CoreState<'_>) -> Result<(), String> {
    core.database.clear_data(&scope).map_err(command_error)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_directory = app
                .path()
                .app_data_dir()
                .context("Could not resolve Moco's data directory")?;
            std::fs::create_dir_all(&data_directory)?;
            let database = Database::open(&data_directory.join("moco.db"))?;
            let model_path =
                resource_candidate(app.handle(), "resources/models/LFM2.5-230M-Q4_K_M.gguf");
            let runtime_path =
                resource_candidate(app.handle(), "resources/runtime/llama-server.exe");
            database.seed_model_catalog(&data_directory.join("models"), &model_path)?;
            app.manage(Arc::new(AppCore {
                database,
                runtime: Arc::new(RuntimeManager::new(runtime_path)),
                data_directory,
                session_api_key: Mutex::new(String::new()),
                downloads: DownloadManager::new(),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            create_conversation,
            rename_conversation,
            set_conversation_flag,
            delete_conversation,
            delete_message,
            set_message_feedback,
            set_message_saved,
            save_settings,
            generate,
            stop_generation,
            load_model,
            unload_model,
            import_model,
            download_model,
            pause_model_download,
            cancel_model_download,
            delete_model,
            import_documents,
            delete_document,
            export_conversation,
            clear_data,
        ])
        .run(tauri::generate_context!())
        .expect("Moco encountered a fatal desktop error");
}

#[cfg(test)]
mod tests {
    use super::{request_needs_desktop_tools, selected_tool_prompt};

    #[test]
    fn everyday_questions_do_not_enter_the_tool_loop() {
        assert!(!request_needs_desktop_tools("654 + 54"));
        assert!(!request_needs_desktop_tools(
            "How does photosynthesis work?"
        ));
        assert!(!request_needs_desktop_tools(
            "Write a short poem about rain"
        ));
    }

    #[test]
    fn desktop_tasks_enter_the_tool_loop() {
        assert!(request_needs_desktop_tools("List the files on my Desktop"));
        assert!(request_needs_desktop_tools("Read Desktop/notes.txt"));
        assert!(request_needs_desktop_tools(
            "Run tests in my project folder"
        ));
    }

    #[test]
    fn selected_tools_add_an_authoritative_instruction() {
        assert!(selected_tool_prompt("grammar").contains("Correct spelling"));
        assert!(selected_tool_prompt("summarize").contains("Summarize"));
        assert_eq!(selected_tool_prompt("auto"), "");
    }
}
