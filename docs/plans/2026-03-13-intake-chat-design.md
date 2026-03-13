# Intake Chat Modal & File Attachments — Design

> **Project:** Ironweave
> **Feature:** Chat-style submission modal with file upload for intake agent
> **Status:** Approved
> **Created:** 2026-03-13

---

## Vision

A clean chat-style modal where users can paste any request — bug report, feature idea, task description — and optionally attach supporting documents. The modal submits it as an issue and the intake agent handles classification, decomposition, and task creation automatically.

---

## Approach

**Approach A: Chat Modal with File Upload** (selected)

A button on the project page opens a centered modal with a large text area and file drop zone. On submit, it uploads files to the server, creates an issue with raw text as description, and the intake agent picks it up. No pre-submit AI parsing — the intake agent already does this.

### Why this approach

- Minimal backend changes — one upload endpoint, one attachments table
- Reuses entire intake agent flow (no duplicated intelligence)
- Clean separation: UI captures input, intake agent does thinking
- Modal pattern already established in the codebase (issue detail modal)

### Alternatives considered

- **Side drawer** — nice for referencing existing issues while typing, but more complex layout
- **Dedicated tab** — most screen space, but heavyweight for quick submissions

---

## Design

### 1. Data Model

New table:

```sql
CREATE TABLE attachments (
    id          TEXT PRIMARY KEY,
    issue_id    TEXT REFERENCES issues(id),
    filename    TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    stored_path TEXT NOT NULL,
    created_at  TEXT DEFAULT (datetime('now'))
);
```

File storage location: `{data_dir}/attachments/{issue_id}/{uuid}_{filename}`

Default data_dir: `/home/paddy/ironweave/data`

Configurable via `attachments_dir` setting (defaults to `{data_dir}/attachments`).

### 2. API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/projects/{pid}/issues/{id}/attachments` | Multipart file upload |
| GET | `/api/projects/{pid}/issues/{id}/attachments` | List attachments for issue |
| GET | `/api/attachments/{id}/download` | Serve file content |

Upload endpoint accepts `multipart/form-data` with a `file` field. Creates the directory structure, stores the file, inserts DB row, returns attachment metadata.

### 3. Chat Modal UI

**Trigger:** "Submit Request" button in the project header bar (visible on all tabs).

**Modal contents (top to bottom):**

1. **Text area** — large, auto-expanding. Placeholder: "Describe what you need — paste a bug report, feature request, or any task..."
2. **File drop zone** — dashed border, supports drag-and-drop and click-to-browse. Shows queued files with name, size, and remove button.
3. **Scope mode toggle** — Auto / Needs Scoping (reuses existing toggle pattern)
4. **Submit button** — disabled while empty or uploading

**On submit:**

1. Create issue: `POST /api/projects/{pid}/issues`
   - `title`: first 80 chars of text (truncated at word boundary)
   - `description`: full text
   - `issue_type`: "task"
   - `needs_intake`: 1
   - `scope_mode`: selected value
2. Upload each file: `POST /api/projects/{pid}/issues/{id}/attachments`
3. Close modal, issue board refreshes via existing 5s polling

No title/type/priority/role fields — the intake agent handles classification.

### 4. Intake Agent Integration

When building the intake agent prompt in `spawn_intake_agent()`:

- Query attachments for the issue
- For text-based files (`.txt`, `.log`, `.md`, `.json`, `.csv`): inline content in the prompt (up to 50KB per file)
- For binary files (images, PDFs): list as references with download URLs

Prompt section:

```
## Attached Files
- error-log.txt (text/plain, 12 KB):
```content
[file contents inlined here]
```
- screenshot.png (image/png, 245 KB) — available at /api/attachments/{id}/download
```

### Not in scope (v1.1)

- Image/PDF preview in the modal
- Drag-and-drop reordering of attachments
- File size limits / allowed MIME type restrictions
- Attachment display in issue detail modal
- Inline markdown preview in the text area
