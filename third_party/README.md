# Third-party runtime notices

Release builds bundle a pinned Windows CPU build of `llama.cpp` and the Q4_K_M GGUF of LiquidAI LFM2.5-230M. Their license texts are downloaded into `third_party/licenses/` by `npm run assets:runtime` and included with release artifacts.

Pinned checksums and source URLs live in `scripts/fetch-runtime.ps1` so release inputs are reproducible and integrity checked.
