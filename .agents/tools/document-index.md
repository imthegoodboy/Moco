# Document Index

- **Purpose:** Copy supported files into Moco's private local library, extract text, split it into overlapping chunks, and create a local vector index.
- **Inputs:** PDF, DOCX, TXT, Markdown, CSV, or HTML files selected by the user.
- **Permission:** Explicit file selection is consent to read and locally copy those files.
- **Output:** Indexed document metadata and progress events.
- **Network:** Never used.
- **Failure behavior:** Keep other imports running and show a plain-language error for the failed file.

