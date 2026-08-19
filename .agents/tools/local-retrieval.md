# Local Retrieval

- **Purpose:** Find the most relevant excerpts across selected or indexed documents.
- **Inputs:** User query, optional document IDs.
- **Method:** Deterministic feature-hashing vectors plus lexical overlap; all computation and storage remain local.
- **Permission:** Read-only. No confirmation required.
- **Output:** Ranked excerpts with document name, optional PDF page, and relevance score.

