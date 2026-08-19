# Moco Tool Registry

Each Markdown file in this directory is the human-readable contract for a Moco tool. Tool behavior is intentionally kept separate from the application UI so prompts, permissions, inputs, and results can evolve without redesigning the product.

Built-in tools are local by default. A tool may use a remote model only after the user explicitly changes the provider in Settings and supplies an API key.

The desktop agent contract is documented in `desktop-agent.md`. Tool selection is automatic by default and can also be chosen explicitly from the chat composer.
