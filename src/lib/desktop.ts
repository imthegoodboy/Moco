import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AgentTool,
  AppSettings,
  BootstrapData,
  Conversation,
  DocumentInfo,
  GenerationEvent,
  GenerationStarted,
  ImportProgress,
  ModelInfo,
  ModelDownloadProgress,
  ToolMode,
} from "../types";

export const isDesktop = () => "__TAURI_INTERNALS__" in window;

const demoSettings: AppSettings = {
  theme: "dark",
  provider: "local",
  apiEndpoint: "https://api.openai.com/v1",
  apiModel: "gpt-4.1-mini",
  apiKey: "",
  rememberApiKey: false,
  customInstructions: "",
  temperature: 0.7,
  topP: 0.9,
  topK: 40,
  maxTokens: 1024,
  contextSize: 8192,
  cpuThreads: 0,
  gpuLayers: 0,
  responseStyle: "balanced",
  responseLength: "normal",
  documentsOnly: false,
  completedOnboarding: true,
  telemetry: false,
};

let demoData: BootstrapData = {
  conversations: [],
  messages: [],
  documents: [],
  models: [
    {
      id: "lfm2.5-230m",
      name: "LFM2.5-230M-fine-tunned",
      path: "bundled",
      parameters: "230M",
      quantization: "Q4_K_M",
      contextLength: 32768,
      sizeBytes: 153_406_304,
      requiredRamBytes: 536_870_912,
      builtIn: true,
      status: "unloaded",
      isDefault: true,
      downloadUrl:
        "https://huggingface.co/LiquidAI/LFM2.5-230M-GGUF/resolve/fb5e743241d08c98626e04c13828feffae4acdfb/LFM2.5-230M-Q4_K_M.gguf?download=true",
      sha256:
        "7bbd90384d3deffe4c646ec9643b212802d32d4ce417c90a1ec9282100650062",
      description: "Small and fast. Best first model for any Windows PC.",
      capabilityTier: "Essential",
      bestFor: "Fast chat, extraction, and short summaries",
    },
    {
      id: "lfm2.5-350m",
      name: "LFM2.5-350M",
      path: "models/LFM2.5-350M-Q4_K_M.gguf",
      parameters: "350M",
      quantization: "Q4_K_M",
      contextLength: 32768,
      sizeBytes: 229_312_224,
      requiredRamBytes: 805_306_368,
      builtIn: false,
      status: "not-downloaded",
      isDefault: false,
      downloadUrl: "verified",
      sha256: "verified",
      description:
        "A little more capable while staying light enough for everyday laptops.",
      capabilityTier: "Everyday",
      bestFor: "General chat, rewriting, and summaries",
    },
    {
      id: "qwen3-0.6b",
      name: "Qwen3-0.6B",
      path: "models/Qwen3-0.6B-Q8_0.gguf",
      parameters: "600M",
      quantization: "Q8_0",
      contextLength: 32768,
      sizeBytes: 639_446_688,
      requiredRamBytes: 1_342_177_280,
      builtIn: false,
      status: "not-downloaded",
      isDefault: false,
      downloadUrl: "verified",
      sha256: "verified",
      description:
        "A compact multilingual model with thinking and non-thinking modes.",
      capabilityTier: "Balanced",
      bestFor: "Multilingual chat, translation, and reasoning",
    },
    {
      id: "lfm2.5-1.2b",
      name: "LFM2.5-1.2B Instruct",
      path: "models/LFM2.5-1.2B-Instruct-Q4_K_M.gguf",
      parameters: "1.2B",
      quantization: "Q4_K_M",
      contextLength: 32768,
      sizeBytes: 730_895_168,
      requiredRamBytes: 1_610_612_736,
      builtIn: false,
      status: "not-downloaded",
      isDefault: false,
      downloadUrl:
        "https://huggingface.co/LiquidAI/LFM2.5-1.2B-Instruct-GGUF/resolve/afbd8eaeab5dd94ba0b079ebfb02517d19641e38/LFM2.5-1.2B-Instruct-Q4_K_M.gguf?download=true",
      sha256:
        "b1b3de114215d9507409a662a501a631095a479a419584e8a2ded6304b19b4f5",
      description:
        "Better writing and instruction following for modern laptops.",
      capabilityTier: "Strong",
      bestFor: "Writing, research overviews, and longer conversations",
    },
    {
      id: "lfm2.5-2.6b",
      name: "LFM2.5-2.6B",
      path: "models/LFM2.5-2.6B-Q4_K_M.gguf",
      parameters: "2.6B",
      quantization: "Q4_K_M",
      contextLength: 32768,
      sizeBytes: 1_674_455_040,
      requiredRamBytes: 3_221_225_472,
      builtIn: false,
      status: "not-downloaded",
      isDefault: false,
      downloadUrl: "verified",
      sha256: "verified",
      description:
        "Higher quality for systems with more memory. A larger download.",
      capabilityTier: "Advanced",
      bestFor: "Research, analysis, and richer writing",
    },
    {
      id: "lfm2.5-8b-a1b",
      name: "LFM2.5-8B-A1B",
      path: "models/LFM2.5-8B-A1B-Q4_K_M.gguf",
      parameters: "8B / 1B active",
      quantization: "Q4_K_M",
      contextLength: 32768,
      sizeBytes: 5_155_564_768,
      requiredRamBytes: 6_442_450_944,
      builtIn: false,
      status: "not-downloaded",
      isDefault: false,
      downloadUrl: "verified",
      sha256: "verified",
      description:
        "A high-capability mixture-of-experts model for well-equipped systems.",
      capabilityTier: "Expert",
      bestFor: "Complex research, tools, analysis, and demanding writing",
    },
  ],
  settings: demoSettings,
  hardware: {
    os: "Windows 11",
    cpu: "Local CPU",
    physicalCores: 10,
    logicalCores: 12,
    totalRamBytes: 8_000_000_000,
    availableRamBytes: 4_000_000_000,
    gpu: "Integrated graphics",
    gpuVramBytes: 2_000_000_000,
    availableDiskBytes: 120_000_000_000,
    acceleration: "CPU (GPU layers optional)",
    compatibility: "Fully supported",
  },
  dataDirectory: "Moco local data",
};

const demoGenerationListeners = new Set<(event: GenerationEvent) => void>();

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isDesktop()) return invoke<T>(command, args);
  return demoCall<T>(command, args);
}

async function demoCall<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  await new Promise((resolve) => setTimeout(resolve, 120));
  if (command === "bootstrap") return structuredClone(demoData) as T;
  if (command === "create_conversation") {
    const now = new Date().toISOString();
    const conversation: Conversation = {
      id: crypto.randomUUID(),
      title: (args?.title as string | undefined) ?? "New conversation",
      pinned: false,
      archived: false,
      documentIds: [],
      createdAt: now,
      updatedAt: now,
    };
    demoData.conversations.unshift(conversation);
    return conversation as T;
  }
  if (command === "save_settings") {
    demoData.settings = args?.settings as AppSettings;
    return structuredClone(demoData.settings) as T;
  }
  if (command === "generate") {
    throw new Error("AI generation is available in the Windows desktop app.");
  }
  if (command === "load_model") {
    const model = demoData.models.find((item) => item.id === args?.id);
    if (model) {
      demoData.models.forEach((item) => {
        item.status =
          item.id === model.id
            ? "loaded"
            : item.status === "loaded"
              ? "unloaded"
              : item.status;
        item.isDefault = item.id === model.id;
      });
      return structuredClone(model) as T;
    }
  }
  if (command === "download_model") {
    const model = demoData.models.find((item) => item.id === args?.id);
    if (model) {
      model.status = "unloaded";
      demoData.models.forEach((item) => {
        item.isDefault = item.id === model.id;
      });
      return structuredClone(model) as T;
    }
  }
  if (command === "rename_conversation") {
    const conversation = demoData.conversations.find(
      (item) => item.id === args?.id,
    );
    if (conversation) conversation.title = args?.title as string;
  }
  if (command === "set_conversation_documents") {
    const conversation = demoData.conversations.find(
      (item) => item.id === args?.id,
    );
    if (conversation)
      conversation.documentIds = (args?.documentIds as string[]) ?? [];
  }
  if (command === "delete_conversation") {
    demoData.conversations = demoData.conversations.filter(
      (item) => item.id !== args?.id,
    );
  }
  return undefined as T;
}

export const desktop = {
  bootstrap: () => call<BootstrapData>("bootstrap"),
  createConversation: (title?: string) =>
    call<Conversation>("create_conversation", { title }),
  renameConversation: (id: string, title: string) =>
    call<void>("rename_conversation", { id, title }),
  setConversationFlag: (
    id: string,
    flag: "pinned" | "archived",
    value: boolean,
  ) => call<void>("set_conversation_flag", { id, flag, value }),
  setConversationDocuments: (id: string, documentIds: string[]) =>
    call<void>("set_conversation_documents", { id, documentIds }),
  deleteConversation: (id: string) => call<void>("delete_conversation", { id }),
  deleteMessage: (id: string) => call<void>("delete_message", { id }),
  setMessageFeedback: (id: string, feedback?: "up" | "down") =>
    call<void>("set_message_feedback", { id, feedback }),
  setMessageSaved: (id: string, saved: boolean) =>
    call<void>("set_message_saved", { id, saved }),
  saveSettings: (settings: AppSettings) =>
    call<AppSettings>("save_settings", { settings }),
  generate: (
    conversationId: string,
    content: string,
    mode: ToolMode,
    documentIds: string[],
    tool: AgentTool,
  ) =>
    call<GenerationStarted>("generate", {
      request: { conversationId, content, mode, documentIds, tool },
    }),
  stopGeneration: (generationId: string) =>
    call<boolean>("stop_generation", { generationId }),
  loadModel: (id: string) => call<ModelInfo>("load_model", { id }),
  downloadModel: (id: string) => call<ModelInfo>("download_model", { id }),
  pauseModelDownload: (id: string) =>
    call<boolean>("pause_model_download", { id }),
  cancelModelDownload: (id: string) =>
    call<boolean>("cancel_model_download", { id }),
  unloadModel: () => call<void>("unload_model"),
  importModel: (path: string) => call<ModelInfo>("import_model", { path }),
  deleteModel: (id: string) => call<void>("delete_model", { id }),
  importDocuments: (paths: string[]) =>
    call<DocumentInfo[]>("import_documents", { paths }),
  deleteDocument: (id: string) => call<void>("delete_document", { id }),
  exportConversation: (id: string, path: string, format: "md" | "txt") =>
    call<void>("export_conversation", { id, path, format }),
  clearData: (scope: "chats" | "documents" | "all") =>
    call<void>("clear_data", { scope }),
  onGeneration: async (
    handler: (event: GenerationEvent) => void,
  ): Promise<UnlistenFn> => {
    if (!isDesktop()) {
      demoGenerationListeners.add(handler);
      return () => {
        demoGenerationListeners.delete(handler);
      };
    }
    return listen<GenerationEvent>("moco://generation", (event) =>
      handler(event.payload),
    );
  },
  onImportProgress: async (
    handler: (event: ImportProgress) => void,
  ): Promise<UnlistenFn> => {
    if (!isDesktop()) return () => undefined;
    return listen<ImportProgress>("moco://import-progress", (event) =>
      handler(event.payload),
    );
  },
  onModelDownload: async (
    handler: (event: ModelDownloadProgress) => void,
  ): Promise<UnlistenFn> => {
    if (!isDesktop()) return () => undefined;
    return listen<ModelDownloadProgress>("moco://model-download", (event) =>
      handler(event.payload),
    );
  },
  chooseDocuments: async (): Promise<string[]> => {
    if (!isDesktop()) return [];
    const result = await open({
      multiple: true,
      directory: false,
      filters: [
        {
          name: "Documents",
          extensions: [
            "pdf",
            "docx",
            "txt",
            "md",
            "markdown",
            "csv",
            "html",
            "htm",
          ],
        },
      ],
    });
    return result ? (Array.isArray(result) ? result : [result]) : [];
  },
  chooseFolder: async (): Promise<string[]> => {
    if (!isDesktop()) return [];
    const result = await open({ multiple: false, directory: true });
    return result ? [result as string] : [];
  },
  chooseModel: async (): Promise<string | null> => {
    if (!isDesktop()) return null;
    const result = await open({
      multiple: false,
      filters: [{ name: "GGUF models", extensions: ["gguf"] }],
    });
    return (result as string | null) ?? null;
  },
  chooseExport: async (
    title: string,
    format: "md" | "txt",
  ): Promise<string | null> => {
    if (!isDesktop()) return null;
    const result = await save({
      defaultPath: `${title.replace(/[<>:"/\\|?*]/g, "-")}.${format}`,
    });
    return result ?? null;
  },
};
