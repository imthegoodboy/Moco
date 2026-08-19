# Desktop Agent

- **Purpose:** Let the single Moco agent inspect and change files available to the signed-in Windows user when the request requires it.
- **Automatic routing:** General questions go directly to inference. Requests that clearly reference files, folders, projects, or paths enter the desktop tool loop. The user can force this route with **Desktop files** in the composer.
- **Read tools:** List folders, read bounded UTF-8 text ranges, and search local text files.
- **Write tools:** Create a new file or replace one exact unique text block. Existing files are never silently overwritten by creation.
- **Validation tool:** Run allowlisted development checks in a selected user folder without invoking a shell interpreter.
- **Boundary:** Paths resolve under the current Windows user profile and remain subject to Windows permissions. Parent traversal, absolute paths, symlink escapes, shell operators, and destructive commands are rejected.
- **Evidence:** Moco must use the returned tool result before it claims that a desktop action occurred.
