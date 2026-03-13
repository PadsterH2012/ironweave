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
      const title = generateTitle(requestText);
      const issue = await issues.create(projectId, {
        project_id: projectId,
        title,
        description: requestText.trim(),
        issue_type: 'task',
        scope_mode: scopeMode,
      });

      for (const file of queuedFiles) {
        await issues.attachments.upload(projectId, issue.id, file);
      }

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
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold text-gray-100">Submit a Request</h2>
      <button onclick={onClose} class="text-gray-500 hover:text-gray-300 text-xl">&times;</button>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
        {error}
      </div>
    {/if}

    <textarea
      bind:value={requestText}
      placeholder="Describe what you need — paste a bug report, feature request, or any task..."
      rows="8"
      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-4 py-3 text-sm focus:outline-none focus:border-purple-500 resize-y min-h-[120px]"
    ></textarea>

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
