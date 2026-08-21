export type View = "chat" | "library" | "saved" | "models" | "settings";
export type ToolMode =
  | "chat"
  | "agent"
  | "summarize"
  | "research"
  | "news"
  | "grammar"
  | "rewrite"
  | "explain"
  | "compare";
export type AgentTool =
  | "auto"
  | "desktop"
  | "documents"
  | "summarize"
  | "research"
  | "grammar"
  | "rewrite"
  | "explain";

export interface Conversation {
  id: string;
  title: string;
  pinned: boolean;
  archived: boolean;
  documentIds: string[];
  createdAt: string;
  updatedAt: string;
}

export interface SourceRef {
  documentId: string;
  documentName: string;
  page?: number;
  excerpt: string;
  score: number;
}

export interface Message {
  id: string;
  conversationId: string;
  role: "user" | "assistant";
  content: string;
  mode: ToolMode;
  createdAt: string;
  sources: SourceRef[];
  feedback?: "up" | "down";
  saved: boolean;
}

export interface DocumentInfo {
  id: string;
  name: string;
  fileType: string;
  sizeBytes: number;
  pageCount: number;
  status: string;
  createdAt: string;
  tags: string[];
}

export interface ModelInfo {
  id: string;
  name: string;
  path: string;
  parameters: string;
  quantization: string;
  contextLength: number;
  sizeBytes: number;
  requiredRamBytes: number;
  builtIn: boolean;
  status:
    | "not-downloaded"
    | "downloading"
    | "paused"
    | "loaded"
    | "unloaded"
    | "loading"
    | "error";
  isDefault: boolean;
  downloadUrl?: string;
  sha256?: string;
  description: string;
  capabilityTier: string;
  bestFor: string;
}

export interface ModelDownloadProgress {
  modelId: string;
  downloadedBytes: number;
  totalBytes: number;
  percent: number;
  bytesPerSecond: number;
  status: string;
  error?: string;
}

export interface AppSettings {
  theme: "dark" | "light" | "system";
  provider: "local" | "api";
  apiEndpoint: string;
  apiModel: string;
  apiKey: string;
  rememberApiKey: boolean;
  customInstructions: string;
  temperature: number;
  topP: number;
  topK: number;
  maxTokens: number;
  contextSize: number;
  cpuThreads: number;
  gpuLayers: number;
  responseStyle: string;
  responseLength: string;
  documentsOnly: boolean;
  completedOnboarding: boolean;
  telemetry: boolean;
}

export interface HardwareInfo {
  os: string;
  cpu: string;
  physicalCores: number;
  logicalCores: number;
  totalRamBytes: number;
  availableRamBytes: number;
  gpu: string;
  gpuVramBytes: number;
  availableDiskBytes: number;
  acceleration: string;
  compatibility: string;
}

export interface BootstrapData {
  conversations: Conversation[];
  messages: Message[];
  documents: DocumentInfo[];
  models: ModelInfo[];
  settings: AppSettings;
  hardware: HardwareInfo;
  dataDirectory: string;
}

export interface GenerationEvent {
  generationId: string;
  conversationId: string;
  delta: string;
  content: string;
  phase: string;
  done: boolean;
  error?: string;
  sources: SourceRef[];
  tokensPerSecond?: number;
}

export interface GenerationStarted {
  generationId: string;
  userMessage: Message;
}

export interface ImportProgress {
  importId: string;
  fileName: string;
  phase: string;
  percent: number;
  error?: string;
}
