use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    pub document_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub document_id: String,
    pub document_name: String,
    pub page: Option<u32>,
    pub excerpt: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub mode: String,
    pub created_at: String,
    pub sources: Vec<SourceRef>,
    pub feedback: Option<String>,
    pub saved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInfo {
    pub id: String,
    pub name: String,
    pub file_type: String,
    pub size_bytes: u64,
    pub page_count: u32,
    pub status: String,
    pub created_at: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub parameters: String,
    pub quantization: String,
    pub context_length: u32,
    pub size_bytes: u64,
    pub required_ram_bytes: u64,
    pub built_in: bool,
    pub status: String,
    pub is_default: bool,
    pub download_url: Option<String>,
    pub sha256: Option<String>,
    pub description: String,
    pub capability_tier: String,
    pub best_for: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
    pub bytes_per_second: u64,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub provider: String,
    pub api_endpoint: String,
    pub api_model: String,
    pub api_key: String,
    pub remember_api_key: bool,
    pub custom_instructions: String,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub max_tokens: u32,
    pub context_size: u32,
    pub cpu_threads: u32,
    pub gpu_layers: i32,
    pub response_style: String,
    pub response_length: String,
    pub documents_only: bool,
    pub completed_onboarding: bool,
    pub telemetry: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            provider: "local".into(),
            api_endpoint: "https://api.openai.com/v1".into(),
            api_model: "gpt-4.1-mini".into(),
            api_key: String::new(),
            remember_api_key: false,
            custom_instructions: String::new(),
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            max_tokens: 1024,
            context_size: 8192,
            cpu_threads: 0,
            gpu_layers: 0,
            response_style: "balanced".into(),
            response_length: "normal".into(),
            documents_only: false,
            completed_onboarding: false,
            telemetry: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub os: String,
    pub cpu: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub total_ram_bytes: u64,
    pub available_ram_bytes: u64,
    pub gpu: String,
    pub gpu_vram_bytes: u64,
    pub available_disk_bytes: u64,
    pub acceleration: String,
    pub compatibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub conversations: Vec<Conversation>,
    pub messages: Vec<Message>,
    pub documents: Vec<DocumentInfo>,
    pub models: Vec<ModelInfo>,
    pub settings: AppSettings,
    pub hardware: HardwareInfo,
    pub data_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub conversation_id: String,
    pub content: String,
    pub mode: String,
    #[serde(default)]
    pub document_ids: Vec<String>,
    #[serde(default = "default_agent_tool")]
    pub tool: String,
    #[serde(default)]
    pub retry_message_id: Option<String>,
}

fn default_agent_tool() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationStarted {
    pub generation_id: String,
    pub user_message: Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationEvent {
    pub generation_id: String,
    pub conversation_id: String,
    pub delta: String,
    pub content: String,
    pub phase: String,
    pub done: bool,
    pub error: Option<String>,
    pub sources: Vec<SourceRef>,
    pub tokens_per_second: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub import_id: String,
    pub file_name: String,
    pub phase: String,
    pub percent: u8,
    pub error: Option<String>,
}
