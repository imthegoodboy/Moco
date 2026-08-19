# Local Inference

- **Purpose:** Generate responses with a GGUF model through the bundled llama.cpp runtime.
- **Default model:** LiquidAI LFM2.5-1.2B Instruct, Q4_K_M.
- **Binding:** Loopback only (`127.0.0.1`); the runtime is not exposed to the local network.
- **Permission:** Read-only generation. No confirmation required.
- **Cancellation:** A running generation can be stopped without closing the app.
- **Network:** Never used after installation.
