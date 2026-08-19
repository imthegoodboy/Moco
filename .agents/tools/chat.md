# Chat

- **Purpose:** General context-aware conversation.
- **Inputs:** User message, conversation history, optional selected document excerpts.
- **Permission:** Read-only. No confirmation required.
- **Output:** Streamed Markdown response with document citations when applicable.
- **Failure behavior:** Preserve the user message, explain the local/runtime or provider error, and offer Retry.

