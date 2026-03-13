<script lang="ts">
  import { mergeQueue, type MergeQueueEntry } from '../api';
  import { onMount } from 'svelte';

  interface Props {
    projectId: string;
  }
  let { projectId }: Props = $props();

  let entries: MergeQueueEntry[] = $state([]);
  let error: string | null = $state(null);
  let expandedId: string | null = $state(null);
  let retryingId: string | null = $state(null);

  async function fetchQueue() {
    try {
      entries = await mergeQueue.list(projectId);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch merge queue';
    }
  }

  async function handleRetry(id: string) {
    retryingId = id;
    try {
      await mergeQueue.approve(projectId, id);
      await fetchQueue();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to retry entry';
    } finally {
      retryingId = null;
    }
  }

  function statusBadge(status: string): string {
    switch (status) {
      case 'pending': return 'bg-blue-600 text-blue-100';
      case 'merging': return 'bg-yellow-600 text-yellow-100';
      case 'conflicted': return 'bg-red-600 text-red-100';
      case 'resolving': return 'bg-yellow-600 text-yellow-100';
      case 'resolved': return 'bg-green-600 text-green-100';
      case 'merged': return 'bg-green-600 text-green-100';
      case 'failed': return 'bg-gray-600 text-gray-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function timeAgo(dateStr: string): string {
    const now = Date.now();
    const then = new Date(dateStr).getTime();
    const seconds = Math.floor((now - then) / 1000);
    if (seconds < 60) return `${seconds}s ago`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    const days = Math.floor(hours / 24);
    return `${days}d ago`;
  }

  function parseConflictFiles(json: string): string[] {
    try {
      const parsed = JSON.parse(json);
      return Array.isArray(parsed) ? parsed : [];
    } catch {
      return [];
    }
  }

  onMount(() => {
    fetchQueue();
    const interval = setInterval(fetchQueue, 10000);
    return () => clearInterval(interval);
  });
</script>

<div class="space-y-4">
  <h2 class="text-lg font-semibold text-white">Merge Queue</h2>

  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  {#if entries.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      No branches in merge queue
    </div>
  {:else}
    <div class="space-y-2">
      {#each entries as entry (entry.id)}
        <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-2">
          <div class="flex items-center gap-3">
            <span class="text-sm font-medium text-white font-mono">{entry.branch_name}</span>
            <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {statusBadge(entry.status)}">
              {entry.status}
            </span>
            <span class="text-xs text-gray-500 ml-auto">{timeAgo(entry.created_at)}</span>
          </div>

          {#if entry.status === 'conflicted'}
            {@const files = parseConflictFiles(entry.conflict_files)}
            {#if files.length > 0}
              <div>
                <button
                  onclick={() => expandedId = expandedId === entry.id ? null : entry.id}
                  class="text-xs text-gray-400 hover:text-gray-200 transition-colors"
                >
                  {expandedId === entry.id ? 'Hide' : 'Show'} {files.length} conflict file{files.length !== 1 ? 's' : ''}
                </button>
                {#if expandedId === entry.id}
                  <ul class="mt-1 space-y-0.5">
                    {#each files as file}
                      <li class="text-xs text-gray-400 font-mono pl-3">{file}</li>
                    {/each}
                  </ul>
                {/if}
              </div>
            {/if}
          {/if}

          {#if entry.error_message && (entry.status === 'failed' || entry.status === 'conflicted')}
            <p class="text-xs text-red-400">{entry.error_message}</p>
          {/if}

          {#if entry.status === 'conflicted' || entry.status === 'failed'}
            <div>
              <button
                onclick={() => handleRetry(entry.id)}
                disabled={retryingId === entry.id}
                class="px-3 py-1 text-xs font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
              >
                {retryingId === entry.id ? 'Retrying...' : 'Retry'}
              </button>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
