<script lang="ts">
  import { sync, type BrowseEntry, type SyncStatus } from '../api';

  interface Props {
    projectId: string;
  }
  let { projectId }: Props = $props();

  let entries: BrowseEntry[] = $state([]);
  let currentPath: string = $state('');
  let pathStack: string[] = $state([]);
  let fileContent: string | null = $state(null);
  let selectedFile: string | null = $state(null);
  let syncStatus: SyncStatus | null = $state(null);
  let loading: boolean = $state(false);
  let loadingFile: boolean = $state(false);
  let syncing: boolean = $state(false);
  let error: string | null = $state(null);

  let breadcrumbs = $derived(
    currentPath
      ? currentPath.split('/').filter(Boolean).map((seg, i, arr) => ({
          label: seg,
          path: arr.slice(0, i + 1).join('/'),
        }))
      : []
  );

  async function fetchStatus() {
    try {
      syncStatus = await sync.status(projectId);
    } catch (e) {
      console.error('Failed to fetch sync status:', e);
    }
  }

  async function browse(path: string) {
    loading = true;
    error = null;
    try {
      entries = await sync.browseFiles(projectId, path || undefined);
      currentPath = path;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to browse files';
    } finally {
      loading = false;
    }
  }

  async function openFile(name: string) {
    const filePath = currentPath ? `${currentPath}/${name}` : name;
    selectedFile = filePath;
    loadingFile = true;
    fileContent = null;
    try {
      const res = await fetch(`/api/projects/${projectId}/files/content?path=${encodeURIComponent(filePath)}`, {
        headers: {
          'Authorization': `Bearer ${localStorage.getItem('ironweave_token') || ''}`,
        },
      });
      if (!res.ok) throw new Error(`Failed to load file: ${res.status}`);
      fileContent = await res.text();
    } catch (e) {
      fileContent = e instanceof Error ? `Error: ${e.message}` : 'Error loading file';
    } finally {
      loadingFile = false;
    }
  }

  function navigateInto(name: string) {
    pathStack = [...pathStack, currentPath];
    const newPath = currentPath ? `${currentPath}/${name}` : name;
    browse(newPath);
  }

  function navigateUp() {
    if (pathStack.length > 0) {
      const prev = pathStack[pathStack.length - 1];
      pathStack = pathStack.slice(0, -1);
      browse(prev);
    } else {
      browse('');
    }
  }

  function navigateToBreadcrumb(path: string) {
    const parts = currentPath.split('/').filter(Boolean);
    const targetParts = path.split('/').filter(Boolean);
    const depth = targetParts.length;
    pathStack = pathStack.slice(0, depth > 0 ? depth - 1 : 0);
    browse(path);
  }

  async function triggerSync() {
    syncing = true;
    try {
      syncStatus = await sync.trigger(projectId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Sync failed';
    } finally {
      syncing = false;
    }
  }

  function formatTime(dateStr: string | null): string {
    if (!dateStr) return 'Never';
    const d = new Date(dateStr);
    return d.toLocaleString();
  }

  function stateColor(state: string): string {
    switch (state) {
      case 'synced': return 'text-green-400';
      case 'syncing': return 'text-yellow-400';
      case 'error': return 'text-red-400';
      default: return 'text-gray-400';
    }
  }

  $effect(() => {
    const pid = projectId;
    if (pid) {
      fetchStatus();
      browse('');
      fileContent = null;
      selectedFile = null;
      pathStack = [];
    }
  });
</script>

<div class="space-y-4">
  <!-- Sync status bar -->
  <div class="rounded-xl bg-gray-900 border border-gray-800 px-4 py-3 flex items-center justify-between">
    <div class="flex items-center gap-6 text-sm">
      <div class="flex items-center gap-2">
        <span class="text-gray-500">Source:</span>
        <span class="text-gray-200 font-medium">{syncStatus?.source ?? '...'}</span>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-gray-500">Last synced:</span>
        <span class="text-gray-300">{formatTime(syncStatus?.last_synced_at ?? null)}</span>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-gray-500">State:</span>
        <span class="font-medium {stateColor(syncStatus?.sync_state ?? 'idle')}">
          {syncStatus?.sync_state ?? 'idle'}
        </span>
      </div>
    </div>
    <button
      onclick={triggerSync}
      disabled={syncing}
      class="px-4 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
    >
      {syncing ? 'Syncing...' : 'Sync Now'}
    </button>
  </div>

  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  <!-- Split layout -->
  <div class="grid grid-cols-3 gap-4 min-h-[500px]">
    <!-- Tree browser (1/3) -->
    <div class="col-span-1 rounded-xl bg-gray-900 border border-gray-800 flex flex-col overflow-hidden">
      <!-- Breadcrumb bar -->
      <div class="px-3 py-2 border-b border-gray-800 flex items-center gap-1 text-xs text-gray-400 overflow-x-auto">
        <button onclick={() => { pathStack = []; browse(''); }} class="hover:text-white shrink-0">/</button>
        {#each breadcrumbs as crumb}
          <span class="shrink-0">/</span>
          <button onclick={() => navigateToBreadcrumb(crumb.path)} class="hover:text-white shrink-0">{crumb.label}</button>
        {/each}
      </div>

      <!-- File list -->
      <div class="flex-1 overflow-y-auto">
        {#if loading}
          <div class="flex items-center justify-center h-full text-gray-500 text-sm">Loading...</div>
        {:else}
          <div class="divide-y divide-gray-800/50">
            {#if currentPath}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="px-3 py-2 text-sm text-gray-300 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
                onclick={navigateUp}
              >
                <span class="text-gray-500">&#8593;</span>
                <span>..</span>
              </div>
            {/if}
            {#each entries as entry}
              {#if entry.type === 'directory'}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="px-3 py-2 text-sm text-gray-200 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
                  onclick={() => navigateInto(entry.name)}
                >
                  <span class="text-yellow-500 text-xs">&#128193;</span>
                  <span>{entry.name}</span>
                </div>
              {:else}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="px-3 py-2 text-sm hover:bg-gray-800 cursor-pointer flex items-center gap-2 {selectedFile === (currentPath ? currentPath + '/' + entry.name : entry.name) ? 'bg-purple-900/30 text-purple-300 border-l-2 border-purple-500' : 'text-gray-300'}"
                  onclick={() => openFile(entry.name)}
                >
                  <span class="text-gray-500 text-xs">&#128196;</span>
                  <span>{entry.name}</span>
                </div>
              {/if}
            {/each}
            {#if entries.length === 0 && !currentPath}
              <div class="p-4 text-gray-500 text-sm text-center">No files found</div>
            {/if}
          </div>
        {/if}
      </div>
    </div>

    <!-- File viewer (2/3) -->
    <div class="col-span-2 rounded-xl bg-gray-900 border border-gray-800 flex flex-col overflow-hidden">
      {#if selectedFile}
        <div class="px-4 py-2 border-b border-gray-800 flex items-center justify-between">
          <span class="text-xs text-gray-400 font-mono truncate" title={selectedFile}>{selectedFile}</span>
          <button
            onclick={() => { selectedFile = null; fileContent = null; }}
            class="text-gray-500 hover:text-white text-sm"
          >
            &times;
          </button>
        </div>
        <div class="flex-1 overflow-auto p-4">
          {#if loadingFile}
            <div class="flex items-center justify-center h-full text-gray-500 text-sm">Loading file...</div>
          {:else if fileContent !== null}
            <pre class="text-sm text-gray-300 font-mono whitespace-pre-wrap break-words leading-relaxed">{fileContent}</pre>
          {/if}
        </div>
      {:else}
        <div class="flex items-center justify-center h-full text-gray-500 text-sm">
          Select a file to view its contents
        </div>
      {/if}
    </div>
  </div>
</div>
