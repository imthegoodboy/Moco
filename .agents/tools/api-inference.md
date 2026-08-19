# API Inference

- **Purpose:** Optionally generate through an OpenAI-compatible API.
- **Inputs:** Base URL, model name, API key, conversation messages.
- **Permission:** The user must explicitly choose API mode. The UI warns that prompts and attached context will leave the device.
- **Output:** Streamed Markdown response.
- **Secrets:** The API key is local and is not included in exports or diagnostics.
- **Failure behavior:** Convert authentication, quota, context, and connection errors into actionable messages.

