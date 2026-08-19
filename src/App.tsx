import { LoaderCircle, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Composer } from "./components/Composer";
import {
  ConfirmDialog,
  type ConfirmState,
  PromptDialog,
  type PromptState,
  ToastStack,
  type Toast,
} from "./components/Dialogs";
import { EmptyState } from "./components/EmptyState";
import { LibraryView } from "./components/LibraryView";
import { MessageList } from "./components/MessageList";
import { ModelsView } from "./components/ModelsView";
import { Onboarding } from "./components/Onboarding";
import { SavedView } from "./components/SavedView";
import { SettingsView } from "./components/SettingsView";
import { Sidebar } from "./components/Sidebar";
import { Titlebar } from "./components/Titlebar";
import { Topbar } from "./components/Topbar";
import { desktop } from "./lib/desktop";
import type {
  AgentTool,
  AppSettings,
  BootstrapData,
  Conversation,
  DocumentInfo,
  GenerationEvent,
  Message,
  ModelDownloadProgress,
  ModelInfo,
  SourceRef,
  ToolMode,
  View,
} from "./types";

interface ActiveGeneration {
  id: string;
  conversationId: string;
  content: string;
  phase: string;
  sources: SourceRef[];
  error?: string;
  mode: ToolMode;
}

const pageTitles: Record<View, string> = {
  chat: "Moco",
  library: "Documents",
  saved: "Saved responses",
  models: "Models",
  settings: "Settings",
};
const toolModes: Record<AgentTool, ToolMode> = {
  auto: "agent",
  desktop: "agent",
  documents: "research",
  summarize: "summarize",
  research: "research",
  grammar: "grammar",
  rewrite: "rewrite",
  explain: "explain",
};
const modeTools: Partial<Record<ToolMode, AgentTool>> = { agent: "auto", summarize: "summarize", research: "research", grammar: "grammar", rewrite: "rewrite", explain: "explain" };

export default function App() {
  const [data, setData] = useState<BootstrapData>();
  const [loadError, setLoadError] = useState<string>();
  const [view, setView] = useState<View>("chat");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [activeConversationId, setActiveConversationId] = useState<string>();
  const [draft, setDraft] = useState("");
  const [selectedTool, setSelectedTool] = useState<AgentTool>("auto");
  const [selectedDocumentIds, setSelectedDocumentIds] = useState<string[]>([]);
  const [generation, setGeneration] = useState<ActiveGeneration>();
  const generationRef = useRef<ActiveGeneration | undefined>(undefined);
  const [importing, setImporting] = useState<string>();
  const [busyModelId, setBusyModelId] = useState<string>();
  const [modelDownloads, setModelDownloads] = useState<
    Record<string, ModelDownloadProgress>
  >({});
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [confirm, setConfirm] = useState<ConfirmState>();
  const [prompt, setPrompt] = useState<PromptState>();

  const notify = useCallback(
    (message: string, type: Toast["type"] = "success") => {
      const id = crypto.randomUUID();
      setToasts((items) => [...items, { id, type, message }]);
      setTimeout(
        () => setToasts((items) => items.filter((item) => item.id !== id)),
        3800,
      );
    },
    [],
  );

  const load = useCallback(async () => {
    setLoadError(undefined);
    try {
      const bootstrap = await desktop.bootstrap();
      setData(bootstrap);
      setActiveConversationId(
        (current) =>
          current ?? bootstrap.conversations.find((item) => !item.archived)?.id,
      );
    } catch (error) {
      setLoadError(String(error));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let disposed = false;
    let unlistenGeneration: () => void = () => undefined;
    let unlistenImport: () => void = () => undefined;
    let unlistenModel: () => void = () => undefined;
    void desktop
      .onGeneration((event) => handleGeneration(event))
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenGeneration = unlisten;
      });
    void desktop
      .onImportProgress((event) => {
        if (event.phase === "complete")
          setImporting(`Indexed ${event.fileName}`);
        else if (event.phase === "error") {
          setImporting(undefined);
          notify(`${event.fileName}: ${event.error}`, "error");
        } else
          setImporting(
            `${event.phase === "reading" ? "Reading" : "Indexing"} ${event.fileName} · ${event.percent}%`,
          );
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenImport = unlisten;
      });
    void desktop
      .onModelDownload((event) => {
        setModelDownloads((current) => ({
          ...current,
          [event.modelId]: event,
        }));
        setData((current) =>
          current
            ? {
                ...current,
                models: current.models.map((model) =>
                  model.id === event.modelId
                    ? {
                        ...model,
                        status:
                          event.status === "complete"
                            ? "unloaded"
                            : event.status === "cancelled"
                              ? "not-downloaded"
                              : (event.status as ModelInfo["status"]),
                      }
                    : model,
                ),
              }
            : current,
        );
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else unlistenModel = unlisten;
      });
    return () => {
      disposed = true;
      unlistenGeneration();
      unlistenImport();
      unlistenModel();
    };
  }, [notify]);

  const handleGeneration = useCallback(
    (event: GenerationEvent) => {
      const current = generationRef.current;
      if (current && current.id !== event.generationId) return;
      if (event.done && !event.error) {
        if (event.content.trim()) {
          const assistant: Message = {
            id: `assistant-${event.generationId}`,
            conversationId: event.conversationId,
            role: "assistant",
            content: event.content,
            mode: current?.mode ?? "agent",
            createdAt: new Date().toISOString(),
            sources: event.sources,
            saved: false,
          };
          setData((existing) => {
            if (
              !existing ||
              existing.messages.some((message) => message.id === assistant.id)
            )
              return existing;
            return {
              ...existing,
              messages: [...existing.messages, assistant],
              models: existing.models.map((model) =>
                model.isDefault ? { ...model, status: "loaded" } : model,
              ),
            };
          });
        }
        if (event.tokensPerSecond)
          notify(
            `Completed at ${event.tokensPerSecond.toFixed(1)} tokens/sec`,
            "info",
          );
        generationRef.current = undefined;
        setGeneration(undefined);
        return;
      }
      const next: ActiveGeneration = {
        id: event.generationId,
        conversationId: event.conversationId,
        content: event.content || current?.content || "",
        phase: event.phase,
        sources: event.sources.length
          ? event.sources
          : (current?.sources ?? []),
        error: event.error,
        mode: current?.mode ?? "agent",
      };
      generationRef.current = next;
      setGeneration(next);
    },
    [notify],
  );

  useEffect(() => {
    if (!data) return;
    const theme =
      data.settings.theme === "system"
        ? window.matchMedia("(prefers-color-scheme: light)").matches
          ? "light"
          : "dark"
        : data.settings.theme;
    document.documentElement.dataset.theme = theme;
  }, [data?.settings.theme]);

  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (!event.ctrlKey) {
        if (event.key === "Escape" && generation) void stop();
        return;
      }
      const key = event.key.toLowerCase();
      if (key === "n") {
        event.preventDefault();
        void newChat();
      }
      if (key === "k") {
        event.preventDefault();
        setSidebarCollapsed(false);
      }
      if (key === "o") {
        event.preventDefault();
        void importDocuments(true);
      }
      if (event.key === ",") {
        event.preventDefault();
        setView("settings");
      }
    };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  });

  const activeConversation = data?.conversations.find(
    (item) => item.id === activeConversationId,
  );
  const activeMessages = useMemo(
    () =>
      data?.messages.filter(
        (item) => item.conversationId === activeConversationId,
      ) ?? [],
    [data?.messages, activeConversationId],
  );
  const defaultModel = data?.models.find((item) => item.isDefault);

  const newChat = async (preferredTool: AgentTool = "auto", seed = "") => {
    try {
      const conversation = await desktop.createConversation();
      setData((current) =>
        current
          ? {
              ...current,
              conversations: [conversation, ...current.conversations],
            }
          : current,
      );
      setActiveConversationId(conversation.id);
      setView("chat");
      setSelectedTool(preferredTool);
      setDraft(seed);
      setSelectedDocumentIds([]);
    } catch (error) {
      notify(String(error), "error");
    }
  };

  const send = async () => {
    const content = draft.trim();
    if (!content || generation) return;
    let conversationId = activeConversationId;
    if (!conversationId) {
      const conversation = await desktop.createConversation();
      conversationId = conversation.id;
      setActiveConversationId(conversation.id);
      setData((current) =>
        current
          ? {
              ...current,
              conversations: [conversation, ...current.conversations],
            }
          : current,
      );
    }
    setDraft("");
    try {
      const requestMode = toolModes[selectedTool];
      const started = await desktop.generate(
        conversationId,
        content,
        requestMode,
        selectedDocumentIds,
        selectedTool,
      );
      const activeGeneration = {
        id: started.generationId,
        conversationId,
        content: "",
        phase: "understanding",
        sources: [],
        mode: requestMode,
      };
      generationRef.current = activeGeneration;
      setGeneration(activeGeneration);
      setSelectedTool("auto");
      setData((current) =>
        current
          ? {
              ...current,
              messages: [...current.messages, started.userMessage],
              conversations: current.conversations.map((conversation) =>
                conversation.id === conversationId
                  ? {
                      ...conversation,
                      title:
                        conversation.title === "New conversation"
                          ? content.split(/\s+/).slice(0, 7).join(" ")
                          : conversation.title,
                      updatedAt: new Date().toISOString(),
                    }
                  : conversation,
              ),
            }
          : current,
      );
    } catch (error) {
      setDraft(content);
      notify(String(error), "error");
    }
  };

  const stop = async () => {
    if (generation) await desktop.stopGeneration(generation.id);
  };

  const importDocuments = async (attachToChat = false) => {
    const paths = await desktop.chooseDocuments();
    if (!paths.length) return;
    setImporting(
      `Preparing ${paths.length} file${paths.length === 1 ? "" : "s"}…`,
    );
    try {
      const documents = await desktop.importDocuments(paths);
      setData((current) =>
        current
          ? { ...current, documents: [...documents, ...current.documents] }
          : current,
      );
      if (attachToChat)
        setSelectedDocumentIds((ids) => [
          ...new Set([...ids, ...documents.map((item) => item.id)]),
        ]);
      notify(
        `${documents.length} document${documents.length === 1 ? "" : "s"} indexed locally`,
      );
    } catch (error) {
      notify(String(error), "error");
    } finally {
      setTimeout(() => setImporting(undefined), 1200);
    }
  };

  const importFolder = async () => {
    const paths = await desktop.chooseFolder();
    if (!paths.length) return;
    setImporting("Scanning folder…");
    try {
      const documents = await desktop.importDocuments(paths);
      setData((current) =>
        current
          ? { ...current, documents: [...documents, ...current.documents] }
          : current,
      );
      notify(`${documents.length} documents indexed`);
    } catch (error) {
      notify(String(error), "error");
    } finally {
      setImporting(undefined);
    }
  };

  const updateSettings = async (settings: AppSettings) => {
    const saved = await desktop.saveSettings(settings);
    setData((current) => (current ? { ...current, settings: saved } : current));
    notify("Settings saved");
  };

  const setDocumentsOnly = (value: boolean) => {
    if (!data) return;
    const settings = { ...data.settings, documentsOnly: value };
    setData({ ...data, settings });
    void desktop.saveSettings(settings);
  };

  const askDocument = async (document: DocumentInfo) => {
    await newChat("documents");
    setSelectedDocumentIds([document.id]);
    setDraft(`Tell me the most important things in ${document.name}.`);
  };

  const exportChat = async () => {
    if (!activeConversation) return;
    const path = await desktop.chooseExport(activeConversation.title, "md");
    if (!path) return;
    try {
      await desktop.exportConversation(activeConversation.id, path, "md");
      notify("Conversation exported");
    } catch (error) {
      notify(String(error), "error");
    }
  };

  const loadModel = async (model: ModelInfo) => {
    setBusyModelId(model.id);
    try {
      const loaded = await desktop.loadModel(model.id);
      setData((current) =>
        current
          ? {
              ...current,
              models: current.models.map((item) =>
                item.id === loaded.id
                  ? loaded
                  : { ...item, status: "unloaded", isDefault: false },
              ),
            }
          : current,
      );
      notify(`${model.name} is ready`);
    } catch (error) {
      notify(String(error), "error");
    } finally {
      setBusyModelId(undefined);
    }
  };

  const importModel = async () => {
    const path = await desktop.chooseModel();
    if (!path) return;
    try {
      const model = await desktop.importModel(path);
      setData((current) =>
        current ? { ...current, models: [...current.models, model] } : current,
      );
      notify(`${model.name} imported`);
    } catch (error) {
      notify(String(error), "error");
    }
  };

  const downloadModel = async (model: ModelInfo) => {
    try {
      const installed = await desktop.downloadModel(model.id);
      setData((current) =>
        current
          ? {
              ...current,
              models: current.models.map((item) =>
                item.id === model.id
                  ? installed
                  : { ...item, isDefault: false },
              ),
            }
          : current,
      );
      setModelDownloads((current) => {
        const next = { ...current };
        delete next[model.id];
        return next;
      });
      notify(`${model.name} downloaded and ready`);
    } catch (error) {
      const message = String(error);
      if (!/paused|cancelled/i.test(message)) notify(message, "error");
    }
  };

  const clearData = (scope: "chats" | "documents" | "all") =>
    setConfirm({
      title: scope === "all" ? "Clear all local data?" : `Delete ${scope}?`,
      message:
        "This permanently removes the selected local data. Installed model files are kept.",
      confirmLabel: "Delete",
      danger: true,
      onConfirm: () =>
        void desktop
          .clearData(scope)
          .then(() => load())
          .then(() => notify("Local data deleted"))
          .catch((error) => notify(String(error), "error")),
    });

  if (loadError)
    return (
      <div className="fatal-screen">
        <span>
          <RefreshCw size={23} />
        </span>
        <h1>Moco could not start</h1>
        <p>{loadError}</p>
        <button
          className="primary-button"
          type="button"
          onClick={() => void load()}
        >
          Try again
        </button>
      </div>
    );
  if (!data || !defaultModel)
    return (
      <div className="loading-screen">
        <span className="brand-mark">M</span>
        <LoaderCircle className="loading-icon" size={18} />
        <p>Preparing your private AI agent</p>
      </div>
    );
  if (!data.settings.completedOnboarding)
    return (
      <Onboarding
        hardware={data.hardware}
        model={defaultModel}
        onComplete={async () =>
          updateSettings({ ...data.settings, completedOnboarding: true })
        }
      />
    );

  return (
    <div className="app-frame">
      <Titlebar />
      <div className="app-body">
        <Sidebar
          collapsed={sidebarCollapsed}
          view={view}
          conversations={data.conversations}
          activeConversationId={activeConversationId}
          onToggle={() => setSidebarCollapsed((value) => !value)}
          onView={setView}
          onNewChat={() => void newChat()}
          onSelectChat={(id) => {
            setActiveConversationId(id);
            setView("chat");
          }}
          onRename={(conversation) =>
            setPrompt({
              title: "Rename conversation",
              message: "Choose a short title that will be easy to find.",
              value: conversation.title,
              confirmLabel: "Rename",
              onConfirm: (title) =>
                void desktop
                  .renameConversation(conversation.id, title)
                  .then(() =>
                    setData((current) =>
                      current
                        ? {
                            ...current,
                            conversations: current.conversations.map((item) =>
                              item.id === conversation.id
                                ? { ...item, title }
                                : item,
                            ),
                          }
                        : current,
                    ),
                  ),
            })
          }
          onPin={(conversation) =>
            void desktop
              .setConversationFlag(
                conversation.id,
                "pinned",
                !conversation.pinned,
              )
              .then(() =>
                setData((current) =>
                  current
                    ? {
                        ...current,
                        conversations: current.conversations.map((item) =>
                          item.id === conversation.id
                            ? { ...item, pinned: !item.pinned }
                            : item,
                        ),
                      }
                    : current,
                ),
              )
          }
          onArchive={(conversation) =>
            void desktop
              .setConversationFlag(conversation.id, "archived", true)
              .then(() =>
                setData((current) =>
                  current
                    ? {
                        ...current,
                        conversations: current.conversations.map((item) =>
                          item.id === conversation.id
                            ? { ...item, archived: true }
                            : item,
                        ),
                      }
                    : current,
                ),
              )
          }
          onDelete={(conversation) =>
            setConfirm({
              title: "Delete this conversation?",
              message: `“${conversation.title}” and all of its messages will be permanently removed.`,
              confirmLabel: "Delete",
              danger: true,
              onConfirm: () =>
                void desktop.deleteConversation(conversation.id).then(() => {
                  setData((current) =>
                    current
                      ? {
                          ...current,
                          conversations: current.conversations.filter(
                            (item) => item.id !== conversation.id,
                          ),
                          messages: current.messages.filter(
                            (item) => item.conversationId !== conversation.id,
                          ),
                        }
                      : current,
                  );
                  if (activeConversationId === conversation.id)
                    setActiveConversationId(undefined);
                }),
            })
          }
        />
        <section className="workspace">
          <Topbar
            title={pageTitles[view]}
            conversation={view === "chat" ? activeConversation : undefined}
            model={defaultModel}
            sidebarCollapsed={sidebarCollapsed}
            provider={data.settings.provider}
            onToggleSidebar={() => setSidebarCollapsed(false)}
            onModels={() => setView("models")}
            onExport={
              activeConversation && activeMessages.length
                ? () => void exportChat()
                : undefined
            }
          />
          {view === "chat" && (
            <div
              className={`chat-view ${!activeMessages.length && !generation ? "chat-welcome" : ""}`}
            >
              {activeMessages.length || generation ? (
                <MessageList
                  messages={activeMessages}
                  streamingContent={generation?.content ?? ""}
                  streamingSources={generation?.sources ?? []}
                  phase={generation?.phase}
                  error={generation?.error}
                  onCopy={(text) =>
                    void navigator.clipboard
                      .writeText(text)
                      .then(() => notify("Copied"))
                  }
                  onDelete={(message) =>
                    setConfirm({
                      title: "Delete this message?",
                      message: "This removes the message from local history.",
                      confirmLabel: "Delete",
                      danger: true,
                      onConfirm: () =>
                        void desktop
                          .deleteMessage(message.id)
                          .then(() =>
                            setData((current) =>
                              current
                                ? {
                                    ...current,
                                    messages: current.messages.filter(
                                      (item) => item.id !== message.id,
                                    ),
                                  }
                                : current,
                            ),
                          ),
                    })
                  }
                  onRetry={(message) => {
                    const index = activeMessages.findIndex(
                      (item) => item.id === message.id,
                    );
                    const user = [...activeMessages.slice(0, index)]
                      .reverse()
                      .find((item) => item.role === "user");
                    if (user) {
                      setDraft(user.content);
                      setSelectedTool(modeTools[user.mode] ?? "auto");
                    }
                  }}
                  onFeedback={(message, feedback) =>
                    void desktop
                      .setMessageFeedback(message.id, feedback)
                      .then(() =>
                        setData((current) =>
                          current
                            ? {
                                ...current,
                                messages: current.messages.map((item) =>
                                  item.id === message.id
                                    ? { ...item, feedback }
                                    : item,
                                ),
                              }
                            : current,
                        ),
                      )
                  }
                  onSave={(message) =>
                    void desktop
                      .setMessageSaved(message.id, !message.saved)
                      .then(() =>
                        setData((current) =>
                          current
                            ? {
                                ...current,
                                messages: current.messages.map((item) =>
                                  item.id === message.id
                                    ? { ...item, saved: !item.saved }
                                    : item,
                                ),
                              }
                            : current,
                        ),
                      )
                  }
                />
              ) : (
                <EmptyState />
              )}
              <Composer
                value={draft}
                selectedTool={selectedTool}
                documents={data.documents}
                selectedDocumentIds={selectedDocumentIds}
                documentsOnly={data.settings.documentsOnly}
                generating={Boolean(generation && !generation.error)}
                onChange={setDraft}
                onToolChange={setSelectedTool}
                onDocumentsOnly={setDocumentsOnly}
                onAttach={() => void importDocuments(true)}
                onRemoveDocument={(id) =>
                  setSelectedDocumentIds((ids) =>
                    ids.filter((item) => item !== id),
                  )
                }
                onSubmit={() => void send()}
                onStop={() => void stop()}
              />
            </div>
          )}
          {view === "library" && (
            <LibraryView
              documents={data.documents}
              importing={importing}
              onImport={() => void importDocuments()}
              onFolder={() => void importFolder()}
              onAsk={(document) => void askDocument(document)}
              onDelete={(document) =>
                setConfirm({
                  title: "Remove this document?",
                  message: `“${document.name}” and its local search index will be deleted.`,
                  confirmLabel: "Remove",
                  danger: true,
                  onConfirm: () =>
                    void desktop
                      .deleteDocument(document.id)
                      .then(() =>
                        setData((current) =>
                          current
                            ? {
                                ...current,
                                documents: current.documents.filter(
                                  (item) => item.id !== document.id,
                                ),
                              }
                            : current,
                        ),
                      ),
                })
              }
            />
          )}
          {view === "models" && (
            <ModelsView
              models={data.models}
              hardware={data.hardware}
              downloads={modelDownloads}
              busyModelId={busyModelId}
              onImport={() => void importModel()}
              onDownload={(model) => void downloadModel(model)}
              onPause={(model) => void desktop.pauseModelDownload(model.id)}
              onCancel={(model) => void desktop.cancelModelDownload(model.id)}
              onLoad={(model) => void loadModel(model)}
              onUnload={() =>
                void desktop
                  .unloadModel()
                  .then(() =>
                    setData((current) =>
                      current
                        ? {
                            ...current,
                            models: current.models.map((item) => ({
                              ...item,
                              status: "unloaded",
                            })),
                          }
                        : current,
                    ),
                  )
              }
              onDelete={(model) =>
                setConfirm({
                  title: "Remove this model?",
                  message: `The downloaded file for “${model.name}” will be permanently deleted. You can download it again later.`,
                  confirmLabel: "Remove",
                  danger: true,
                  onConfirm: () =>
                    void desktop
                      .deleteModel(model.id)
                      .then(() =>
                        setData((current) =>
                          current
                            ? {
                                ...current,
                                models: current.models.map((item) =>
                                  item.id === model.id
                                    ? { ...item, status: "not-downloaded" }
                                    : item,
                                ),
                              }
                            : current,
                        ),
                      ),
                })
              }
            />
          )}
          {view === "saved" && (
            <SavedView
              messages={data.messages}
              onCopy={(text) =>
                void navigator.clipboard
                  .writeText(text)
                  .then(() => notify("Copied"))
              }
              onRemove={(message) =>
                void desktop
                  .setMessageSaved(message.id, false)
                  .then(() =>
                    setData((current) =>
                      current
                        ? {
                            ...current,
                            messages: current.messages.map((item) =>
                              item.id === message.id
                                ? { ...item, saved: false }
                                : item,
                            ),
                          }
                        : current,
                    ),
                  )
              }
            />
          )}
          {view === "settings" && (
            <SettingsView
              settings={data.settings}
              hardware={data.hardware}
              dataDirectory={data.dataDirectory}
              onSave={updateSettings}
              onClear={clearData}
            />
          )}
        </section>
      </div>
      <ToastStack
        toasts={toasts}
        onDismiss={(id) =>
          setToasts((items) => items.filter((item) => item.id !== id))
        }
      />
      <ConfirmDialog state={confirm} onClose={() => setConfirm(undefined)} />
      <PromptDialog state={prompt} onClose={() => setPrompt(undefined)} />
    </div>
  );
}
