<script lang="ts">
  import { mergeQueue, type MergeQueueEntry, type MergeQueueDiff } from '../api';
  import { onMount } from 'svelte';

  interface Props {
    projectId: string;
  }
  let { projectId }: Props = $props();

  let entries: MergeQueueEntry[] = $state([]);
  let error: string | null = $state(null);
  let expandedId: string | null = $state(null);
  let retryingId: string | null = $state(null);
  let resolvingId: string | null = $state(null);
  let rejectingId: string | null = $state(null);
  let diffData: MergeQueueDiff | null = $state(null);
  let diffEntryId: string | null = $state(null);
  let diffLoading: boolean = $state(false);

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

  async function handleResolve(id: string) {
    resolvingId = id;
    try {
      await mergeQueue.resolve(projectId, id);
      await fetchQueue();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to spawn resolver';
    } finally {
      resolvingId = null;
    }
  }

  async function handleReject(id: string) {
    rejectingId = id;
    try {
      await mergeQueue.reject(projectId, id);
      await fetchQueue();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to reject entry';
    } finally {
      rejectingId = null;
    }
  }

  async function handleApprove(id: string) {
    retryingId = id;
    try {
      await mergeQueue.approve(projectId, id);
      await fetchQueue();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to approve entry';
    } finally {
      retryingId = null;
    }
  }

  async function toggleDiff(id: string) {
    if (diffEntryId === id) {
      diffEntryId = null;
      diffData = null;
      return;
    }
    diffLoading = true;
    diffEntryId = id;
    try {
      diffData = await mergeQueue.diff(projectId, id);
    } catch (e) {
      diffData = null;
      error = e instanceof Error ? e.message : 'Failed to load diff';
    } finally {
      diffLoading = false;
    }
  }

  function statusBadge(status: string): string {
    switch (status) {
      case 'pending': return 'bg-blue-600 text-blue-100';
      case 'merging': return 'bg-yellow-600 text-yellow-100';
      case 'verifying': return 'bg-yellow-600 text-yellow-100';
      case 'reviewing': return 'bg-purple-600 text-purple-100';
      case 'conflicted': return 'bg-red-600 text-red-100';
      case 'resolving': return 'bg-yellow-600 text-yellow-100';
      case 'escalated': return 'bg-orange-600 text-orange-100';
      case 'merged': return 'bg-green-600 text-green-100';
      case 'build_failed': return 'bg-red-600 text-red-100';
      case 'review_failed': return 'bg-red-600 text-red-100';
      case 'rejected': return 'bg-gray-600 text-gray-100';
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
            {#if entry.resolver_agent_id && entry.status === 'resolving'}
              <span class="text-[10px] text-yellow-400 flex items-center gap-1">
                <span class="inline-block w-2 h-2 rounded-full bg-yellow-400 animate-pulse"></span>
                Resolver active
              </span>
            {/if}
            <span class="text-xs text-gray-500 ml-auto">{timeAgo(entry.created_at)}</span>
          </div>

          <!-- Escalated banner -->
          {#if entry.status === 'escalated'}
            <div class="rounded-lg bg-orange-900/30 border border-orange-700 px-3 py-2 text-sm text-orange-300">
              Auto-resolver failed. Human review required — view the diff and approve or reject.
            </div>
          {/if}

          <!-- Resolving spinner -->
          {#if entry.status === 'resolving'}
            <div class="flex items-center gap-2 text-sm text-yellow-300">
              <svg class="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
              </svg>
              Resolver agent working on conflicts...
            </div>
          {/if}

          <!-- Conflict files for conflicted/escalated/resolving -->
          {#if entry.status === 'conflicted' || entry.status === 'escalated' || entry.status === 'resolving'}
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

          <!-- Error message -->
          {#if entry.error_message && ['build_failed', 'review_failed', 'conflicted', 'escalated', 'rejected'].includes(entry.status)}
            <p class="text-xs text-red-400">{entry.error_message}</p>
          {/if}

          <!-- Action buttons -->
          <div class="flex gap-2 flex-wrap">
            <!-- Diff viewer toggle (for conflicted, escalated, review_failed) -->
            {#if ['conflicted', 'escalated', 'review_failed'].includes(entry.status)}
              <button
                onclick={() => toggleDiff(entry.id)}
                disabled={diffLoading && diffEntryId === entry.id}
                class="px-3 py-1 text-xs font-medium rounded-lg bg-gray-700 hover:bg-gray-600 disabled:bg-gray-800 disabled:text-gray-500 text-gray-200 transition-colors"
              >
                {#if diffLoading && diffEntryId === entry.id}
                  Loading...
                {:else if diffEntryId === entry.id}
                  Hide Diff
                {:else}
                  View Diff
                {/if}
              </button>
            {/if}

            <!-- Spawn resolver (for conflicted entries without active resolver) -->
            {#if entry.status === 'conflicted'}
              <button
                onclick={() => handleResolve(entry.id)}
                disabled={resolvingId === entry.id}
                class="px-3 py-1 text-xs font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
              >
                {resolvingId === entry.id ? 'Spawning...' : 'Spawn Resolver'}
              </button>
            {/if}

            <!-- Approve / Reject for escalated -->
            {#if entry.status === 'escalated'}
              <button
                onclick={() => handleApprove(entry.id)}
                disabled={retryingId === entry.id}
                class="px-3 py-1 text-xs font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
              >
                {retryingId === entry.id ? 'Approving...' : 'Approve & Retry'}
              </button>
              <button
                onclick={() => handleReject(entry.id)}
                disabled={rejectingId === entry.id}
                class="px-3 py-1 text-xs font-medium rounded-lg bg-red-600 hover:bg-red-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
              >
                {rejectingId === entry.id ? 'Rejecting...' : 'Reject'}
              </button>
            {/if}

            <!-- Retry for build_failed/review_failed -->
            {#if entry.status === 'build_failed' || entry.status === 'review_failed'}
              <button
                onclick={() => handleRetry(entry.id)}
                disabled={retryingId === entry.id}
                class="px-3 py-1 text-xs font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
              >
                {retryingId === entry.id ? 'Retrying...' : 'Retry'}
              </button>
            {/if}
          </div>

          <!-- Inline diff viewer -->
          {#if diffEntryId === entry.id && diffData}
            <div class="mt-2 rounded-lg bg-gray-950 border border-gray-700 overflow-hidden">
              <div class="px-3 py-2 bg-gray-800 border-b border-gray-700 flex items-center gap-2 text-xs text-gray-400">
                <span class="font-mono">{diffData.branch}</span>
                <span>vs</span>
                <span class="font-mono">{diffData.target}</span>
              </div>
              <div class="overflow-x-auto max-h-96">
                <pre class="p-3 text-xs leading-relaxed">{#each diffData.diff.split('\n') as line}{#if line.startsWith('+') && !line.startsWith('+++')}<span class="text-green-400">{line}</span>
{:else if line.startsWith('-') && !line.startsWith('---')}<span class="text-red-400">{line}</span>
{:else if line.startsWith('@@')}<span class="text-blue-400">{line}</span>
{:else}<span class="text-gray-400">{line}</span>
{/if}{/each}</pre>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
