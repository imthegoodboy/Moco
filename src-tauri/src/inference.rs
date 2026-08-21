use crate::models::{AppSettings, GenerationEvent, Message, ModelInfo, SourceRef};
use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use parking_lot::Mutex;
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;

const LOCAL_PORT: u16 = 39281;

pub struct RuntimeManager {
    binary_path: PathBuf,
    process: tokio::sync::Mutex<Option<Child>>,
    loaded_model: tokio::sync::Mutex<Option<String>>,
    client: Client,
    cancellations: Mutex<HashMap<String, CancellationToken>>,
}

impl RuntimeManager {
    pub fn new(binary_path: PathBuf) -> Self {
        Self {
            binary_path,
            process: tokio::sync::Mutex::new(None),
            loaded_model: tokio::sync::Mutex::new(None),
            client: Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(300))
                .build()
                .expect("HTTP client should initialize"),
            cancellations: Mutex::new(HashMap::new()),
        }
    }

    pub async fn ensure_running(&self, model: &ModelInfo, settings: &AppSettings) -> Result<()> {
        let already_loaded = self.loaded_model.lock().await.as_deref() == Some(model.id.as_str());
        if already_loaded && self.health().await {
            return Ok(());
        }

        self.stop_runtime().await;
        if !self.binary_path.exists() {
            bail!(
                "The local AI runtime is missing. Reinstall Moco or run the runtime asset setup. Expected: {}",
                self.binary_path.display()
            );
        }
        if !Path::new(&model.path).exists() {
            bail!(
                "The model file is missing. Reinstall Moco or import a GGUF model. Expected: {}",
                model.path
            );
        }

        let threads = if settings.cpu_threads == 0 {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(4)
                .max(2)
        } else {
            settings.cpu_threads as usize
        };
        let mut command = Command::new(&self.binary_path);
        command
            .arg("-m")
            .arg(&model.path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(LOCAL_PORT.to_string())
            .arg("--ctx-size")
            .arg(settings.context_size.to_string())
            .arg("--threads")
            .arg(threads.to_string())
            .arg("--n-gpu-layers")
            .arg(settings.gpu_layers.to_string())
            .arg("--no-webui")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        #[cfg(windows)]
        {
            command.creation_flags(0x08000000);
        }

        let child = command
            .spawn()
            .context("The local AI runtime could not start")?;
        *self.process.lock().await = Some(child);
        *self.loaded_model.lock().await = Some(model.id.clone());

        for _ in 0..160 {
            if self.health().await {
                return Ok(());
            }
            if let Some(process) = self.process.lock().await.as_mut()
                && let Some(status) = process.try_wait()?
            {
                *self.loaded_model.lock().await = None;
                bail!("The local AI runtime stopped during model loading ({status}).");
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        self.stop_runtime().await;
        bail!("The model took too long to load. Try a smaller context size or fewer GPU layers.")
    }

    pub async fn health(&self) -> bool {
        self.client
            .get(format!("http://127.0.0.1:{LOCAL_PORT}/health"))
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false)
    }

    pub async fn stop_runtime(&self) {
        if let Some(mut process) = self.process.lock().await.take() {
            let _ = process.kill().await;
            let _ = process.wait().await;
        }
        *self.loaded_model.lock().await = None;
    }

    pub fn cancel(&self, generation_id: &str) -> bool {
        self.cancellations
            .lock()
            .remove(generation_id)
            .map(|token| {
                token.cancel();
                true
            })
            .unwrap_or(false)
    }

    pub async fn stream_chat(
        self: &Arc<Self>,
        app: &AppHandle,
        generation_id: &str,
        conversation_id: &str,
        messages: &[Value],
        settings: &AppSettings,
        sources: &[SourceRef],
    ) -> Result<String> {
        let token = CancellationToken::new();
        self.cancellations
            .lock()
            .insert(generation_id.to_string(), token.clone());

        let (endpoint, authorization) = if settings.provider == "local" {
            (
                format!("http://127.0.0.1:{LOCAL_PORT}/v1/chat/completions"),
                None,
            )
        } else {
            if settings.api_key.trim().is_empty() {
                bail!("Add an API key in Settings before using API mode.");
            }
            (
                format!(
                    "{}/chat/completions",
                    settings.api_endpoint.trim_end_matches('/')
                ),
                Some(format!("Bearer {}", settings.api_key.trim())),
            )
        };

        let model_name = if settings.provider == "local" {
            "local-model"
        } else {
            settings.api_model.as_str()
        };
        let payload = json!({
            "model": model_name,
            "messages": messages,
            "stream": true,
            "temperature": settings.temperature,
            "top_p": settings.top_p,
            "top_k": settings.top_k,
            "max_tokens": settings.max_tokens,
        });

        let mut request = self.client.post(endpoint).json(&payload);
        if let Some(value) = authorization {
            request = request.header("Authorization", value);
        }
        let response = request
            .send()
            .await
            .context("Could not reach the selected AI provider")?;
        let status = response.status();
        if !status.is_success() {
            let details = response.text().await.unwrap_or_default();
            bail!(
                "The AI provider returned {status}: {}",
                friendly_provider_error(&details)
            );
        }

        let started = Instant::now();
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut output = String::new();
        let mut approximate_tokens = 0usize;

        while let Some(chunk) = stream.next().await {
            if token.is_cancelled() {
                break;
            }
            buffer.push_str(&String::from_utf8_lossy(&chunk?));
            while let Some(position) = buffer.find('\n') {
                let line = buffer[..position].trim().to_string();
                buffer.drain(..=position);
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" || data.is_empty() {
                    continue;
                }
                let parsed: Value = match serde_json::from_str(data) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                let delta = parsed
                    .pointer("/choices/0/delta/content")
                    .and_then(Value::as_str)
                    .or_else(|| parsed.pointer("/choices/0/text").and_then(Value::as_str))
                    .unwrap_or_default();
                if delta.is_empty() {
                    continue;
                }
                output.push_str(delta);
                approximate_tokens += delta.split_whitespace().count();
                emit_generation(
                    app,
                    GenerationEvent {
                        generation_id: generation_id.to_string(),
                        conversation_id: conversation_id.to_string(),
                        delta: delta.to_string(),
                        content: output.clone(),
                        phase: "generating".into(),
                        done: false,
                        error: None,
                        sources: sources.to_vec(),
                        tokens_per_second: None,
                    },
                );
            }
        }

        self.cancellations.lock().remove(generation_id);
        let elapsed = started.elapsed().as_secs_f32().max(0.01);
        let speed = approximate_tokens as f32 / elapsed;
        emit_generation(
            app,
            GenerationEvent {
                generation_id: generation_id.to_string(),
                conversation_id: conversation_id.to_string(),
                delta: String::new(),
                content: output.clone(),
                phase: if token.is_cancelled() {
                    "stopped".into()
                } else {
                    "complete".into()
                },
                done: true,
                error: None,
                sources: sources.to_vec(),
                tokens_per_second: Some(speed),
            },
        );
        Ok(output)
    }

    pub async fn complete_with_tools(
        &self,
        messages: &[Value],
        settings: &AppSettings,
        tools: &[Value],
    ) -> Result<Value> {
        let (endpoint, authorization) = if settings.provider == "local" {
            (
                format!("http://127.0.0.1:{LOCAL_PORT}/v1/chat/completions"),
                None,
            )
        } else {
            if settings.api_key.trim().is_empty() {
                bail!("Add an API key in Settings before using API mode.");
            }
            (
                format!(
                    "{}/chat/completions",
                    settings.api_endpoint.trim_end_matches('/')
                ),
                Some(format!("Bearer {}", settings.api_key.trim())),
            )
        };
        let mut payload = json!({
            "model": if settings.provider == "local" { "local-model" } else { settings.api_model.as_str() },
            "messages": messages,
            "stream": false,
            "temperature": 0.2,
            "top_p": settings.top_p,
            "max_tokens": settings.max_tokens,
        });
        if !tools.is_empty() {
            payload["tools"] = Value::Array(tools.to_vec());
            payload["tool_choice"] = Value::String("auto".into());
        }
        let mut request = self.client.post(endpoint).json(&payload);
        if let Some(value) = authorization {
            request = request.header("Authorization", value);
        }
        let response = request
            .send()
            .await
            .context("Could not reach the selected AI provider")?;
        let status = response.status();
        if !status.is_success() {
            let details = response.text().await.unwrap_or_default();
            bail!(
                "The AI provider returned {status}: {}",
                friendly_provider_error(&details)
            );
        }
        let body: Value = response
            .json()
            .await
            .context("The AI provider returned invalid JSON")?;
        body.pointer("/choices/0/message")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("The AI provider returned no assistant message."))
    }
}

pub fn chat_messages(history: &[Message], system_prompt: &str, user_content: &str) -> Vec<Value> {
    let mut messages = vec![json!({ "role": "system", "content": system_prompt })];
    let mut chars = 0usize;
    let mut selected = Vec::new();
    for message in history.iter().rev() {
        if chars + message.content.chars().count() > 26_000 {
            break;
        }
        chars += message.content.chars().count();
        selected.push(message);
    }
    for message in selected.into_iter().rev() {
        messages.push(json!({ "role": message.role, "content": message.content }));
    }
    messages.push(json!({ "role": "user", "content": user_content }));
    messages
}

pub fn emit_generation(app: &AppHandle, event: GenerationEvent) {
    let _ = app.emit("moco://generation", event);
}

pub fn emit_phase(
    app: &AppHandle,
    generation_id: &str,
    conversation_id: &str,
    phase: &str,
    sources: &[SourceRef],
) {
    emit_generation(
        app,
        GenerationEvent {
            generation_id: generation_id.to_string(),
            conversation_id: conversation_id.to_string(),
            delta: String::new(),
            content: String::new(),
            phase: phase.to_string(),
            done: false,
            error: None,
            sources: sources.to_vec(),
            tokens_per_second: None,
        },
    );
}

pub fn emit_error(
    app: &AppHandle,
    generation_id: &str,
    conversation_id: &str,
    error: &anyhow::Error,
) {
    emit_generation(
        app,
        GenerationEvent {
            generation_id: generation_id.to_string(),
            conversation_id: conversation_id.to_string(),
            delta: String::new(),
            content: String::new(),
            phase: "error".into(),
            done: true,
            error: Some(error.to_string()),
            sources: Vec::new(),
            tokens_per_second: None,
        },
    );
}

fn friendly_provider_error(raw: &str) -> String {
    if raw.contains("insufficient_quota") {
        "The API account has no available quota.".into()
    } else if raw.contains("invalid_api_key") || raw.contains("Incorrect API key") {
        "The API key was rejected. Check it in Settings.".into()
    } else if raw.contains("context_length") {
        "This conversation is larger than the model context. Start a new chat or reduce the context size.".into()
    } else {
        let compact = raw.replace(['\n', '\r'], " ");
        compact.chars().take(320).collect()
    }
}
