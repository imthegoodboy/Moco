# Model Manager

- **Purpose:** Inspect, import, select, load, unload, and remove local GGUF models.
- **Import:** The user explicitly selects a `.gguf` file; Moco copies it into its private model directory.
- **Delete:** Built-in models cannot be removed. Removing an imported model requires confirmation in the UI.
- **Runtime:** Switching models safely stops the current local runtime before loading another model.

