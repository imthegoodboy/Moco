# Moco

Moco is a private, local-first AI agent for Windows. It opens like a normal chatbot, answers with a bundled LiquidAI LFM model, searches local documents, and can choose safe desktop tools when a request needs files or development checks.

## What is included

- One AI agent with normal conversation and automatic tool routing
- LFM2.5-230M (shown as `LFM2.5-230M-fine-tunned`) bundled in the Windows installer and selected by default
- Optional one-click GGUF model downloads with progress, pause, resume, integrity checks, selection, and removal
- Hardware-aware model recommendations and a **My models** view
- Local PDF, DOCX, Markdown, text, CSV, and HTML indexing with cited retrieval
- Explicit composer tools for Desktop files, My documents, Summarize, Research, Grammar, Rewrite, and Explain
- Local SQLite history, saved responses, export, dark/light themes, and no telemetry
- Optional OpenAI-compatible API provider configured by the user

## Desktop tool boundary

General questions go directly to the selected model. Desktop/file requests enter a tool controller that can list, read, search, create, or precisely edit files under the signed-in Windows user's profile and run allowlisted development checks. Windows permissions apply. Absolute paths, parent traversal, symlink escapes, shell operators, destructive commands, and silent overwrites are rejected.

## Development

Requirements: Node.js 20+, Rust stable, and Visual Studio Build Tools with the Desktop development with C++ workload.

```powershell
npm install
npm run assets:runtime
npm run desktop:dev
```

The browser Vite page is only a UI preview; AI generation runs in the Tauri desktop application.

## Verification and packaging

```powershell
npm run build
npm run verify:offline
npm test
npm run desktop:build
```

`npm run assets:runtime` downloads pinned release assets and verifies their SHA-256 hashes. Release builds bundle the local llama.cpp runtime and LFM model; those large binaries are intentionally excluded from Git.

Third-party notices are in [`third_party`](third_party/README.md). Agent prompts and tool contracts are Markdown under [`.agents`](.agents/).
