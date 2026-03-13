# Intake Chat Modal & File Attachments Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a chat-style modal for submitting requests to the intake agent, with file attachment support.

**Architecture:** New `attachments` table and model for file metadata. New API module for multipart upload, list, and download. Frontend chat modal on the project page with text area and drag-and-drop file zone. Intake agent prompt extended to include attachment content.

**Tech Stack:** Rust (Axum 0.8 built-in Multipart), rusqlite, Svelte 5, TypeScript

**Design doc:** `docs/plans/2026-03-13-intake-chat-design.md`

**Test command:** `/Users/paddyharker/.cargo/bin/cargo test --lib`

**Existing patterns to follow:**
- DB migration: `src/db/migrations.rs` — ALTER TABLE pattern with error swallowing
- Model: `src/models/issue.rs` — struct with `from_row`, CRUD methods, tests
- API handler: `src/api/issues.rs` — Axum handlers with `State<AppState>`, `Path`, `Json`
- Route registration: `src/main.rs:117-185`
- Frontend API client: `frontend/src/lib/api.ts`
- Frontend component: `frontend/src/lib/components/IssueBoard.svelte`

---

### Task 1: DB Migration — Attachments Table

**Files:**
- Modify: `src/db/migrations.rs`

**Step 1: Add the attachments table migration**

In `src/db/migrations.rs`, add after the existing ALTER TABLE statements (at the end of `run_migrations`):

```rust
// Attachments table
conn.execute_batch("
    CREATE TABLE IF NOT EXISTS attachments (
        id          TEXT PRIMARY KEY,
        issue_id    TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
        filename    TEXT NOT NULL,
        mime_type   TEXT NOT NULL,
        size_bytes  INTEGER NOT NULL,
        stored_path TEXT NOT NULL,
        created_at  TEXT NOT NULL DEFAULT (datetime('now'))
    );
    CREATE INDEX IF NOT EXISTS idx_attachments_issue ON attachments(issue_id);
")?;
```

**Step 2: Run tests to verify migration doesn't break anything**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib`
Expected: All 128 tests pass

**Step 3: Commit**

```bash
git add src/db/migrations.rs
git commit -m "feat: add attachments table migration"
```

---

### Task 2: Attachment Model

**Files:**
- Create: `src/models/attachment.rs`
- Modify: `src/models/mod.rs`

**Step 1: Write the attachment model with tests**

Create `src/models/attachment.rs`:

```rust
use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub issue_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub stored_path: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAttachment {
    pub issue_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub stored_path: String,
}

impl Attachment {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            issue_id: row.get("issue_id")?,
            filename: row.get("filename")?,
            mime_type: row.get("mime_type")?,
            size_bytes: row.get("size_bytes")?,
            stored_path: row.get("stored_path")?,
            created_at: row.get("created_at")?,
        })
    }

    pub fn create(conn: &Connection, input: &CreateAttachment) -> crate::error::Result<Self> {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO attachments (id, issue_id, filename, mime_type, size_bytes, stored_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, input.issue_id, input.filename, input.mime_type, input.size_bytes, input.stored_path],
        )?;
        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> crate::error::Result<Self> {
        conn.query_row(
            "SELECT * FROM attachments WHERE id = ?1",
            params![id],
            Self::from_row,
        ).map_err(|_| crate::error::IronweaveError::NotFound(format!("attachment: {}", id)))
    }

    pub fn list_by_issue(conn: &Connection, issue_id: &str) -> crate::error::Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM attachments WHERE issue_id = ?1 ORDER BY created_at ASC"
        )?;
        let rows = stmt.query_map(params![issue_id], Self::from_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn delete(conn: &Connection, id: &str) -> crate::error::Result<()> {
        let changes = conn.execute("DELETE FROM attachments WHERE id = ?1", params![id])?;
        if changes == 0 {
            return Err(crate::error::IronweaveError::NotFound(format!("attachment: {}", id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        // Create a project and issue for FK
        conn.execute(
            "INSERT INTO projects (id, name, directory, context) VALUES ('p1', 'test', '/tmp', 'homelab')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO issues (id, project_id, issue_type, title, description, status, priority, depends_on, needs_intake, scope_mode)
             VALUES ('i1', 'p1', 'task', 'test issue', 'desc', 'open', 3, '[]', 1, 'auto')",
            [],
        ).unwrap();
        conn
    }

    #[test]
    fn test_create_and_get() {
        let conn = setup_db();
        let input = CreateAttachment {
            issue_id: "i1".to_string(),
            filename: "test.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 1024,
            stored_path: "/data/attachments/i1/abc_test.txt".to_string(),
        };
        let att = Attachment::create(&conn, &input).unwrap();
        assert_eq!(att.filename, "test.txt");
        assert_eq!(att.size_bytes, 1024);

        let fetched = Attachment::get_by_id(&conn, &att.id).unwrap();
        assert_eq!(fetched.id, att.id);
    }

    #[test]
    fn test_list_by_issue() {
        let conn = setup_db();
        for i in 0..3 {
            Attachment::create(&conn, &CreateAttachment {
                issue_id: "i1".to_string(),
                filename: format!("file{}.txt", i),
                mime_type: "text/plain".to_string(),
                size_bytes: 100,
                stored_path: format!("/data/attachments/i1/{}_file{}.txt", i, i),
            }).unwrap();
        }
        let list = Attachment::list_by_issue(&conn, "i1").unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_delete() {
        let conn = setup_db();
        let att = Attachment::create(&conn, &CreateAttachment {
            issue_id: "i1".to_string(),
            filename: "delete_me.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 50,
            stored_path: "/data/attachments/i1/del.txt".to_string(),
        }).unwrap();
        Attachment::delete(&conn, &att.id).unwrap();
        assert!(Attachment::get_by_id(&conn, &att.id).is_err());
    }
}
```

**Step 2: Register the module**

In `src/models/mod.rs`, add:

```rust
pub mod attachment;
```

**Step 3: Run tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib`
Expected: All tests pass (128 existing + 3 new = 131)

**Step 4: Commit**

```bash
git add src/models/attachment.rs src/models/mod.rs
git commit -m "feat: add Attachment model with CRUD operations"
```

---

### Task 3: Attachments API — Upload, List, Download

**Files:**
- Create: `src/api/attachments.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/state.rs`
- Modify: `src/main.rs`

**Step 1: Add `data_dir` to AppState**

In `src/state.rs`, add a field to `AppState`:

```rust
pub data_dir: std::path::PathBuf,
```

In `src/main.rs`, where `AppState` is constructed (around line 106), add:

```rust
data_dir: config.data_dir.clone(),
```

**Step 2: Create the attachments API handler**

Create `src/api/attachments.rs`:

```rust
use axum::{
    extract::{Path, State, Multipart},
    response::IntoResponse,
    Json, http::StatusCode,
};
use crate::state::AppState;
use crate::models::attachment::{Attachment, CreateAttachment};

pub async fn upload(
    State(state): State<AppState>,
    Path((_pid, issue_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Attachment>), StatusCode> {
    // Extract the file from multipart
    let field = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "upload".to_string());

    let mime_type = field
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let data = field
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let size_bytes = data.len() as i64;

    // Build storage path: {data_dir}/attachments/{issue_id}/{uuid}_{filename}
    let file_id = uuid::Uuid::new_v4().to_string();
    let safe_filename = filename.replace(['/', '\\', '\0'], "_");
    let dir = state.data_dir.join("attachments").join(&issue_id);
    let file_path = dir.join(format!("{}_{}", &file_id[..8], safe_filename));

    // Create directory and write file
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create attachment dir: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tokio::fs::write(&file_path, &data)
        .await
        .map_err(|e| {
            tracing::error!("Failed to write attachment: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Save to DB
    let input = CreateAttachment {
        issue_id,
        filename: safe_filename,
        mime_type,
        size_bytes,
        stored_path: file_path.to_string_lossy().to_string(),
    };

    let conn = state.db.lock().unwrap();
    Attachment::create(&conn, &input)
        .map(|a| (StatusCode::CREATED, Json(a)))
        .map_err(|e| {
            tracing::error!("Failed to save attachment record: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn list(
    State(state): State<AppState>,
    Path((_pid, issue_id)): Path<(String, String)>,
) -> Result<Json<Vec<Attachment>>, StatusCode> {
    let conn = state.db.lock().unwrap();
    Attachment::list_by_issue(&conn, &issue_id)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let attachment = {
        let conn = state.db.lock().unwrap();
        Attachment::get_by_id(&conn, &id).map_err(|_| StatusCode::NOT_FOUND)?
    };

    let data = tokio::fs::read(&attachment.stored_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let headers = [
        (axum::http::header::CONTENT_TYPE, attachment.mime_type),
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", attachment.filename),
        ),
    ];

    Ok((headers, data))
}
```

**Step 3: Register the module in `src/api/mod.rs`**

Add:

```rust
pub mod attachments;
```

**Step 4: Register routes in `src/main.rs`**

After the issues routes (around line 146), add:

```rust
// Attachments
.route("/api/projects/{pid}/issues/{id}/attachments", get(api::attachments::list).post(api::attachments::upload))
.route("/api/attachments/{id}/download", get(api::attachments::download))
```

**Step 5: Build and run tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib`
Expected: All tests pass (131)

**Step 6: Commit**

```bash
git add src/api/attachments.rs src/api/mod.rs src/state.rs src/main.rs
git commit -m "feat: add attachments API with upload, list, and download"
```

---

### Task 4: Frontend API Client — Attachment Types and Methods

**Files:**
- Modify: `frontend/src/lib/api.ts`

**Step 1: Add Attachment interface**

After the `UpdateIssue` interface (around line 188), add:

```typescript
export interface Attachment {
  id: string;
  issue_id: string;
  filename: string;
  mime_type: string;
  size_bytes: number;
  stored_path: string;
  created_at: string;
}
```

**Step 2: Add attachment API methods**

Add to the `issues` export object (around line 501):

```typescript
attachments: {
  list: (projectId: string, issueId: string) =>
    get<Attachment[]>(`/projects/${projectId}/issues/${issueId}/attachments`),
  upload: async (projectId: string, issueId: string, file: File): Promise<Attachment> => {
    const form = new FormData();
    form.append('file', file);
    const res = await fetch(`${BASE}/projects/${projectId}/issues/${issueId}/attachments`, {
      method: 'POST',
      headers: { ...authHeaders() },
      body: form,
    });
    if (!res.ok) {
      handle401(res);
      throw new Error(`Upload failed: ${res.status}`);
    }
    return res.json();
  },
  downloadUrl: (attachmentId: string) => `${BASE}/attachments/${attachmentId}/download`,
},
```

**Step 3: Commit**

```bash
git add frontend/src/lib/api.ts
git commit -m "feat: add attachment types and API methods to frontend client"
```

---

### Task 5: Chat Modal Component

**Files:**
- Create: `frontend/src/lib/components/IntakeChat.svelte`

**Step 1: Create the IntakeChat component**

Create `frontend/src/lib/components/IntakeChat.svelte`:

```svelte
<script lang="ts">
  import { issues, type Attachment } from '../api';

  interface Props {
    projectId: string;
    onClose: () => void;
    onSubmitted: () => void;
  }
  let { projectId, onClose, onSubmitted }: Props = $props();

  let requestText: string = $state('');
  let scopeMode: string = $state('auto');
  let queuedFiles: File[] = $state([]);
  let submitting: boolean = $state(false);
  let error: string | null = $state(null);
  let dragOver: boolean = $state(false);

  function generateTitle(text: string): string {
    const firstLine = text.split('\n')[0].trim();
    if (firstLine.length <= 80) return firstLine;
    // Truncate at word boundary
    const truncated = firstLine.substring(0, 80);
    const lastSpace = truncated.lastIndexOf(' ');
    return lastSpace > 40 ? truncated.substring(0, lastSpace) + '...' : truncated + '...';
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    if (e.dataTransfer?.files) {
      queuedFiles = [...queuedFiles, ...Array.from(e.dataTransfer.files)];
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    dragOver = true;
  }

  function handleDragLeave() {
    dragOver = false;
  }

  function handleFileInput(e: Event) {
    const input = e.target as HTMLInputElement;
    if (input.files) {
      queuedFiles = [...queuedFiles, ...Array.from(input.files)];
    }
  }

  function removeFile(index: number) {
    queuedFiles = queuedFiles.filter((_, i) => i !== index);
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function handleSubmit() {
    if (!requestText.trim()) return;
    submitting = true;
    error = null;

    try {
      // 1. Create the issue
      const title = generateTitle(requestText);
      const issue = await issues.create(projectId, {
        project_id: projectId,
        title,
        description: requestText.trim(),
        issue_type: 'task',
        scope_mode: scopeMode,
      });

      // 2. Upload files
      for (const file of queuedFiles) {
        await issues.attachments.upload(projectId, issue.id, file);
      }

      // 3. Done
      onSubmitted();
      onClose();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Submission failed';
    } finally {
      submitting = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-4"
  onclick={onClose}
>
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="bg-gray-900 border border-gray-700 rounded-2xl max-w-2xl w-full max-h-[85vh] overflow-y-auto p-6 space-y-4"
    onclick={(e) => e.stopPropagation()}
  >
    <!-- Header -->
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold text-gray-100">Submit a Request</h2>
      <button onclick={onClose} class="text-gray-500 hover:text-gray-300 text-xl">&times;</button>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
        {error}
      </div>
    {/if}

    <!-- Text area -->
    <textarea
      bind:value={requestText}
      placeholder="Describe what you need — paste a bug report, feature request, or any task..."
      rows="8"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-4 py-3 text-sm focus:outline-none focus:border-purple-500 resize-y min-h-[120px]"
    ></textarea>

    <!-- File drop zone -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="rounded-lg border-2 border-dashed px-4 py-6 text-center transition-colors {dragOver
        ? 'border-purple-500 bg-purple-600/10'
        : 'border-gray-700 hover:border-gray-600'}"
      ondrop={handleDrop}
      ondragover={handleDragOver}
      ondragleave={handleDragLeave}
    >
      <p class="text-sm text-gray-400">
        Drag & drop files here, or
        <label class="text-purple-400 hover:text-purple-300 cursor-pointer underline">
          browse
          <input type="file" multiple class="hidden" onchange={handleFileInput} />
        </label>
      </p>
    </div>

    <!-- Queued files -->
    {#if queuedFiles.length > 0}
      <div class="space-y-1">
        {#each queuedFiles as file, i}
          <div class="flex items-center gap-2 text-sm px-3 py-2 rounded bg-gray-800">
            <span class="text-gray-200 flex-1 truncate">{file.name}</span>
            <span class="text-xs text-gray-500">{formatSize(file.size)}</span>
            <button
              onclick={() => removeFile(i)}
              class="text-gray-500 hover:text-red-400 text-xs"
            >&times;</button>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Scope mode toggle -->
    <div>
      <label class="block text-xs text-gray-400 mb-1">Scope Mode</label>
      <div class="flex gap-2">
        <button
          type="button"
          onclick={() => scopeMode = 'auto'}
          class="flex-1 px-2 py-1.5 text-xs rounded border transition-colors {scopeMode === 'auto'
            ? 'border-purple-500 bg-purple-600/20 text-purple-300'
            : 'border-gray-700 bg-gray-900 text-gray-400'}"
        >
          Auto
        </button>
        <button
          type="button"
          onclick={() => scopeMode = 'conversational'}
          class="flex-1 px-2 py-1.5 text-xs rounded border transition-colors {scopeMode === 'conversational'
            ? 'border-purple-500 bg-purple-600/20 text-purple-300'
            : 'border-gray-700 bg-gray-900 text-gray-400'}"
        >
          Needs Scoping
        </button>
      </div>
    </div>

    <!-- Submit -->
    <div class="flex justify-end">
      <button
        onclick={handleSubmit}
        disabled={submitting || !requestText.trim()}
        class="px-6 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
      >
        {submitting ? 'Submitting...' : 'Submit'}
      </button>
    </div>
  </div>
</div>
```

**Step 2: Commit**

```bash
git add frontend/src/lib/components/IntakeChat.svelte
git commit -m "feat: add IntakeChat modal component"
```

---

### Task 6: Wire Chat Modal into Project Page

**Files:**
- Modify: `frontend/src/routes/ProjectDetail.svelte`

**Step 1: Import IntakeChat and add state**

At the top of the `<script>` block, add to imports:

```typescript
import IntakeChat from '../lib/components/IntakeChat.svelte';
```

Add state variable:

```typescript
let showIntakeChat: boolean = $state(false);
```

**Step 2: Add "Submit Request" button to project header**

In the project header `<div>` (around line 376), after the mount button, add:

```svelte
<button
  onclick={() => showIntakeChat = true}
  class="ml-auto px-4 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
>
  Submit Request
</button>
```

**Step 3: Add the modal at the end of the template**

Just before the closing `</div>` of the main container (before `{:else if !error}`), add:

```svelte
{#if showIntakeChat}
  <IntakeChat
    projectId={params.id}
    onClose={() => showIntakeChat = false}
    onSubmitted={() => {}}
  />
{/if}
```

**Step 4: Build frontend to verify**

Run: `cd frontend && npm run build`
Expected: No errors

**Step 5: Commit**

```bash
git add frontend/src/routes/ProjectDetail.svelte
git commit -m "feat: wire IntakeChat modal into project page header"
```

---

### Task 7: Intake Agent Prompt — Include Attachments

**Files:**
- Modify: `src/orchestrator/runner.rs`

**Step 1: Find `spawn_intake_agent` method**

This method is in `src/orchestrator/runner.rs`. It builds the intake agent prompt. Look for the `spawn_intake_agent` function (around line 1000-1107).

**Step 2: After gathering project context, query attachments**

After the prompt is built and before spawning the agent, add an attachments section. Insert this code after the `let prompt = format!(...)` block but before `let session_id = ...`:

```rust
// Append attachment info to prompt
let attachments_section = {
    let conn = self.db.lock().unwrap();
    let attachments = crate::models::attachment::Attachment::list_by_issue(&conn, &issue.id)
        .unwrap_or_default();
    if attachments.is_empty() {
        String::new()
    } else {
        let mut section = String::from("\n\n## Attached Files\n\n");
        for att in &attachments {
            let is_text = matches!(
                att.mime_type.as_str(),
                "text/plain" | "text/markdown" | "text/csv"
                    | "application/json" | "application/xml"
                    | "text/x-log" | "text/x-rust" | "text/x-python"
            ) || att.filename.ends_with(".log")
              || att.filename.ends_with(".txt")
              || att.filename.ends_with(".md")
              || att.filename.ends_with(".json")
              || att.filename.ends_with(".csv");

            if is_text && att.size_bytes <= 50_000 {
                // Inline text file content
                if let Ok(content) = std::fs::read_to_string(&att.stored_path) {
                    section.push_str(&format!(
                        "### {} ({}, {} bytes)\n```\n{}\n```\n\n",
                        att.filename, att.mime_type, att.size_bytes, content
                    ));
                } else {
                    section.push_str(&format!(
                        "- {} ({}, {} bytes) — file could not be read\n",
                        att.filename, att.mime_type, att.size_bytes
                    ));
                }
            } else {
                section.push_str(&format!(
                    "- {} ({}, {} bytes) — binary file, available at /api/attachments/{}/download\n",
                    att.filename, att.mime_type, att.size_bytes, att.id
                ));
            }
        }
        section
    }
};

let prompt = format!("{}{}", prompt, attachments_section);
```

Note: This shadows the existing `prompt` variable with the extended version.

**Step 3: Run tests**

Run: `/Users/paddyharker/.cargo/bin/cargo test --lib`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src/orchestrator/runner.rs
git commit -m "feat: include attachment content in intake agent prompt"
```

---

### Task 8: Build, Deploy, Smoke Test

**Files:** None (deployment task)

**Step 1: Build frontend**

```bash
cd frontend && npm run build && cd ..
```

**Step 2: Run full test suite**

```bash
/Users/paddyharker/.cargo/bin/cargo test --lib
```

Expected: All tests pass (131+)

**Step 3: Deploy to server**

Follow existing deploy pattern:
```bash
rsync -avz --exclude target --exclude node_modules --exclude .git . paddy@10.202.28.205:/home/paddy/ironweave/
ssh paddy@10.202.28.205 "cd /home/paddy/ironweave && cargo build --release && sudo systemctl restart ironweave"
```

**Step 4: Smoke test**

1. Open project page in browser
2. Click "Submit Request" button in header
3. Type a test request in the text area
4. Drag a file into the drop zone
5. Click Submit
6. Verify issue appears on the kanban board with "intake pending" badge
7. Check that the file was stored in `/home/paddy/ironweave/data/attachments/`

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: deployment adjustments for intake chat"
```
