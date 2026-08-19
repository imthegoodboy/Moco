# Moco Architecture

Moco is a Tauri 2 Windows desktop app. React renders the interface while a Rust core owns files, SQLite storage, document parsing, local retrieval, model lifecycle, and provider requests.

## Runtime flow

1. The user sends a message or imports a document.
2. Moco persists the input in local SQLite.
3. Selected documents are retrieved through the local chunk/vector index.
4. The prompt is composed from the core system prompt, focused tool prompt, conversation history, and ranked excerpts.
5. Generation runs either through bundled llama.cpp on loopback or an explicitly configured OpenAI-compatible API.
6. Tokens, activity phases, sources, completion state, and errors stream to the React UI.
7. The final answer and local feedback are persisted.

## Privacy boundary

Local mode does not require a network connection, authentication server, telemetry endpoint, or cloud model. API mode is a separate, visible user choice. The llama.cpp server binds only to `127.0.0.1`.

## Storage

- `moco.db`: conversations, messages, settings, model metadata, documents, chunks, vectors, feedback, and audit events.
- `documents/`: private copies of explicitly imported files.
- `models/`: explicitly imported GGUF models.
- Bundled resources: the llama.cpp Windows CPU runtime and the built-in LFM2.5-1.2B Instruct Q4_K_M model.
