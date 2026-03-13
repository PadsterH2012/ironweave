<script lang="ts">
  import { sync, type SyncSnapshot } from '../api';

  interface Props {
    projectId: string;
  }
  let { projectId }: Props = $props();

  let snapshots: SyncSnapshot[] = $state([]);
  let selectedSnapshot: SyncSnapshot | null = $state(null);
  let diffContent: string | null = $state(null);
  let loading: boolean = $state(false);
  let loadingDiff: boolean = $state(false);
  let restoring: boolean = $state(false);
  let error: string | null = $state(null);
  let successMessage: string | null = $state(null);

  let diffLines = $derived(
    diffContent ? diffContent.split('\n') : []
  );

  async function fetchHistory() {
    loading = true;
    error = null;
    try {
      snapshots = await sync.history(projectId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load history';
    } finally {
      loading = false;
    }
  }

  async function selectSnapshot(snapshot: SyncSnapshot) {
    selectedSnapshot = snapshot;
    loadingDiff = true;
    diffContent = null;
    try {
      const res = await fetch(`/api/projects/${projectId}/sync/diff/${snapshot.change_id}`, {
        headers: {
          'Authorization': `Bearer ${localStorage.getItem('ironweave_token') || ''}`,
        },
      });
      if (!res.ok) throw new Error(`Failed to load diff: ${res.status}`);
      diffContent = await res.text();
    } catch (e) {
      diffContent = e instanceof Error ? `Error: ${e.message}` : 'Error loading diff';
    } finally {
      loadingDiff = false;
    }
  }

  async function restoreSnapshot(snapshot: SyncSnapshot) {
    if (!confirm(`Restore to snapshot ${snapshot.change_id.slice(0, 12)}?\n\n"${snapshot.description}"\n\nThis will revert files to this point in time.`)) {
      return;
    }
    restoring = true;
    error = null;
    successMessage = null;
    try {
      await sync.restore(projectId, snapshot.change_id);
      successMessage = `Restored to snapshot ${snapshot.change_id.slice(0, 12)}`;
      await fetchHistory();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to restore snapshot';
    } finally {
      restoring = false;
    }
  }

  function formatTime(dateStr: string): string {
    const d = new Date(dateStr);
    return d.toLocaleString();
  }

  function lineClass(line: string): string {
    if (line.startsWith('+') && !line.startsWith('+++')) return 'text-green-400 bg-green-900/20';
    if (line.startsWith('-') && !line.startsWith('---')) return 'text-red-400 bg-red-900/20';
    if (line.startsWith('@@')) return 'text-purple-400 bg-purple-900/10';
    if (line.startsWith('diff ') || line.startsWith('index ')) return 'text-gray-500 font-semibold';
    return 'text-gray-400';
  }

  $effect(() => {
    const pid = projectId;
    if (pid) {
      fetchHistory();
      selectedSnapshot = null;
      diffContent = null;
    }
  });
</script>

<div class="space-y-4">
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  {#if successMessage}
    <div class="rounded-lg bg-green-900/40 border border-green-700 px-4 py-3 text-green-300 text-sm">
      {successMessage}
    </div>
  {/if}

  {#if loading}
    <div class="flex items-center justify-center py-12 text-gray-500 text-sm">Loading history...</div>
  {:else}
    <div class="grid grid-cols-3 gap-4 min-h-[500px]">
      <!-- Snapshot list (1/3) -->
      <div class="col-span-1 rounded-xl bg-gray-900 border border-gray-800 flex flex-col overflow-hidden">
        <div class="px-4 py-3 border-b border-gray-800">
          <h3 class="text-sm font-semibold text-gray-300">Snapshots</h3>
        </div>
        <div class="flex-1 overflow-y-auto divide-y divide-gray-800/50">
          {#if snapshots.length === 0}
            <div class="p-4 text-gray-500 text-sm text-center">No snapshots found</div>
          {/if}
          {#each snapshots as snapshot (snapshot.change_id)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="px-4 py-3 cursor-pointer transition-colors {selectedSnapshot?.change_id === snapshot.change_id ? 'bg-purple-900/20 border-l-2 border-purple-500' : 'hover:bg-gray-800'}"
              onclick={() => selectSnapshot(snapshot)}
            >
              <div class="flex items-start justify-between gap-2">
                <div class="min-w-0 flex-1">
                  <div class="text-xs font-mono text-purple-400 truncate" title={snapshot.change_id}>
                    {snapshot.change_id.slice(0, 12)}
                  </div>
                  <div class="text-sm text-gray-300 mt-1 line-clamp-2">
                    {snapshot.description || '(no description)'}
                  </div>
                  <div class="text-xs text-gray-500 mt-1">
                    {formatTime(snapshot.timestamp)}
                  </div>
                </div>
              </div>
              <button
                onclick={(e) => { e.stopPropagation(); restoreSnapshot(snapshot); }}
                disabled={restoring}
                class="mt-2 px-3 py-1 text-xs font-medium rounded bg-gray-800 border border-gray-700 text-gray-300 hover:bg-yellow-900/30 hover:border-yellow-700 hover:text-yellow-300 disabled:opacity-50 transition-colors"
              >
                {restoring ? 'Restoring...' : 'Restore'}
              </button>
            </div>
          {/each}
        </div>
      </div>

      <!-- Diff viewer (2/3) -->
      <div class="col-span-2 rounded-xl bg-gray-900 border border-gray-800 flex flex-col overflow-hidden">
        {#if selectedSnapshot}
          <div class="px-4 py-3 border-b border-gray-800 flex items-center justify-between">
            <div class="text-sm text-gray-300">
              <span class="font-mono text-purple-400">{selectedSnapshot.change_id.slice(0, 12)}</span>
              <span class="text-gray-500 mx-2">&mdash;</span>
              <span>{selectedSnapshot.description || '(no description)'}</span>
            </div>
          </div>
          <div class="flex-1 overflow-auto">
            {#if loadingDiff}
              <div class="flex items-center justify-center h-full text-gray-500 text-sm">Loading diff...</div>
            {:else if diffContent !== null}
              <pre class="text-xs font-mono leading-relaxed p-4">{#each diffLines as line}<span class="{lineClass(line)}">{line}</span>
{/each}</pre>
            {/if}
          </div>
        {:else}
          <div class="flex items-center justify-center h-full text-gray-500 text-sm">
            Select a snapshot to view its diff
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
