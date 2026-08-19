use crate::models::{ModelDownloadProgress, ModelInfo};
use crate::storage::Database;
use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use parking_lot::Mutex;
use reqwest::{Client, StatusCode, header::RANGE};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopAction {
    Pause,
    Cancel,
}

struct DownloadControl {
    token: CancellationToken,
    action: Arc<Mutex<StopAction>>,
}

pub struct DownloadManager {
    client: Client,
    controls: Mutex<HashMap<String, DownloadControl>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(20))
                .timeout(Duration::from_secs(60 * 30))
                .user_agent("Moco/0.1 model manager")
                .build()
                .expect("download client should initialize"),
            controls: Mutex::new(HashMap::new()),
        }
    }

    pub fn pause(&self, model_id: &str) -> bool {
        self.stop(model_id, StopAction::Pause)
    }
    pub fn cancel(&self, model_id: &str) -> bool {
        self.stop(model_id, StopAction::Cancel)
    }

    fn stop(&self, model_id: &str, action: StopAction) -> bool {
        if let Some(control) = self.controls.lock().get(model_id) {
            *control.action.lock() = action;
            control.token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn download(
        &self,
        app: &AppHandle,
        database: &Database,
        model: &ModelInfo,
    ) -> Result<ModelInfo> {
        let url = model
            .download_url
            .as_ref()
            .ok_or_else(|| anyhow!("This imported model has no download source."))?;
        let expected_sha = model
            .sha256
            .as_ref()
            .ok_or_else(|| anyhow!("This model has no integrity checksum."))?;
        let target = PathBuf::from(&model.path);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let partial = target.with_extension("gguf.part");
        let existing = tokio::fs::metadata(&partial)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);
        let token = CancellationToken::new();
        let action = Arc::new(Mutex::new(StopAction::Pause));
        self.controls.lock().insert(
            model.id.clone(),
            DownloadControl {
                token: token.clone(),
                action: action.clone(),
            },
        );
        database.set_model_status(&model.id, "downloading")?;
        emit(
            app,
            &model.id,
            existing,
            model.size_bytes,
            0,
            "downloading",
            None,
        );

        let result = self
            .download_inner(app, model, url, &partial, existing, token.clone())
            .await;
        self.controls.lock().remove(&model.id);

        if token.is_cancelled() {
            let stop_action = *action.lock();
            if stop_action == StopAction::Cancel {
                let _ = tokio::fs::remove_file(&partial).await;
                database.set_model_status(&model.id, "not-downloaded")?;
                emit(app, &model.id, 0, model.size_bytes, 0, "cancelled", None);
                bail!("Model download cancelled.");
            }
            database.set_model_status(&model.id, "paused")?;
            let downloaded = tokio::fs::metadata(&partial)
                .await
                .map(|meta| meta.len())
                .unwrap_or(0);
            emit(
                app,
                &model.id,
                downloaded,
                model.size_bytes,
                0,
                "paused",
                None,
            );
            bail!("Model download paused.");
        }

        if let Err(error) = result {
            database.set_model_status(&model.id, "error")?;
            emit(
                app,
                &model.id,
                existing,
                model.size_bytes,
                0,
                "error",
                Some(error.to_string()),
            );
            return Err(error);
        }

        let partial_for_hash = partial.clone();
        let actual_sha = tokio::task::spawn_blocking(move || -> Result<String> {
            use std::io::Read;
            let mut file = std::fs::File::open(&partial_for_hash)?;
            let mut hasher = Sha256::new();
            let mut buffer = [0u8; 1024 * 1024];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
            Ok(hex::encode(hasher.finalize()))
        })
        .await??;
        if !actual_sha.eq_ignore_ascii_case(expected_sha) {
            let _ = tokio::fs::remove_file(&partial).await;
            database.set_model_status(&model.id, "error")?;
            bail!("The downloaded model failed its integrity check and was removed. Try again.");
        }
        tokio::fs::rename(&partial, &target)
            .await
            .context("Could not finish installing the model")?;
        database.set_model_status(&model.id, "unloaded")?;
        database.set_default_model(&model.id)?;
        database.audit("model.downloaded", &model.name)?;
        emit(
            app,
            &model.id,
            model.size_bytes,
            model.size_bytes,
            0,
            "complete",
            None,
        );
        let mut installed = model.clone();
        installed.status = "unloaded".into();
        installed.is_default = true;
        Ok(installed)
    }

    async fn download_inner(
        &self,
        app: &AppHandle,
        model: &ModelInfo,
        url: &str,
        partial: &PathBuf,
        mut downloaded: u64,
        token: CancellationToken,
    ) -> Result<()> {
        let mut request = self.client.get(url);
        if downloaded > 0 {
            request = request.header(RANGE, format!("bytes={downloaded}-"));
        }
        let response = request
            .send()
            .await
            .context("Could not connect to the model host")?;
        if !response.status().is_success() {
            bail!("The model host returned {}.", response.status());
        }
        let can_resume = response.status() == StatusCode::PARTIAL_CONTENT;
        if downloaded > 0 && !can_resume {
            downloaded = 0;
            let _ = tokio::fs::remove_file(partial).await;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(partial)
            .await?;
        let total = if can_resume {
            downloaded
                + response
                    .content_length()
                    .unwrap_or(model.size_bytes - downloaded)
        } else {
            response.content_length().unwrap_or(model.size_bytes)
        };
        let started = Instant::now();
        let start_bytes = downloaded;
        let mut last_emit = Instant::now();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if token.is_cancelled() {
                return Ok(());
            }
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if last_emit.elapsed() >= Duration::from_millis(160) {
                let speed = ((downloaded - start_bytes) as f64
                    / started.elapsed().as_secs_f64().max(0.01)) as u64;
                emit(
                    app,
                    &model.id,
                    downloaded,
                    total,
                    speed,
                    "downloading",
                    None,
                );
                last_emit = Instant::now();
            }
        }
        file.flush().await?;
        if downloaded != total && total > 0 {
            bail!("The download ended early ({downloaded} of {total} bytes). Resume to continue.");
        }
        Ok(())
    }
}

fn emit(
    app: &AppHandle,
    model_id: &str,
    downloaded: u64,
    total: u64,
    speed: u64,
    status: &str,
    error: Option<String>,
) {
    let _ = app.emit(
        "moco://model-download",
        ModelDownloadProgress {
            model_id: model_id.into(),
            downloaded_bytes: downloaded,
            total_bytes: total,
            percent: if total > 0 {
                downloaded as f32 / total as f32 * 100.0
            } else {
                0.0
            },
            bytes_per_second: speed,
            status: status.into(),
            error,
        },
    );
}
