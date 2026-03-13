<script lang="ts">
  import { filesystem, type BrowseResponse, type BrowseEntry } from '../api';

  interface Props {
    initialPath?: string;
    onSelect: (path: string) => void;
    onClose: () => void;
  }

  let { initialPath = '/home/paddy', onSelect, onClose }: Props = $props();

  let currentPath: string = $state(initialPath);
  let entries: BrowseEntry[] = $state([]);
  let parent: string | null = $state(null);
  let loading: boolean = $state(false);
  let error: string | null = $state(null);

  let breadcrumbs = $derived(
    currentPath.split('/').filter(Boolean).map((seg, i, arr) => ({
      label: seg,
      path: '/' + arr.slice(0, i + 1).join('/'),
    }))
  );

  async function browse(path: string) {
    loading = true;
    error = null;
    try {
      const res: BrowseResponse = await filesystem.browse(path);
      currentPath = res.path;
      entries = res.entries;
      parent = res.parent;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to browse directory';
    } finally {
      loading = false;
    }
  }

  function navigateTo(path: string) {
    browse(path);
  }

  function handleSelect() {
    onSelect(currentPath);
  }

  $effect(() => {
    browse(initialPath);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 bg-black/60 z-50 flex items-center justify-center" onclick={onClose}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="bg-gray-900 border border-gray-700 rounded-xl w-full max-w-2xl mx-4 shadow-2xl" onclick={(e) => e.stopPropagation()}>
    <div class="flex items-center justify-between px-4 py-3 border-b border-gray-800">
      <h2 class="text-sm font-semibold text-white">Browse Directory</h2>
      <button onclick={onClose} class="text-gray-400 hover:text-white text-lg">&times;</button>
    </div>

    <div class="px-4 py-2 border-b border-gray-800 flex items-center gap-1 text-xs text-gray-400 overflow-x-auto">
      <button onclick={() => navigateTo('/')} class="hover:text-white shrink-0">/</button>
      {#each breadcrumbs as crumb}
        <span class="shrink-0">/</span>
        <button onclick={() => navigateTo(crumb.path)} class="hover:text-white shrink-0">{crumb.label}</button>
      {/each}
    </div>

    <div class="h-72 overflow-y-auto">
      {#if loading}
        <div class="flex items-center justify-center h-full text-gray-500 text-sm">Loading...</div>
      {:else if error}
        <div class="p-4 text-red-400 text-sm">{error}</div>
      {:else}
        <div class="divide-y divide-gray-800/50">
          {#if parent}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="px-4 py-2 text-sm text-gray-300 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
              onclick={() => navigateTo(parent!)}
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
                class="px-4 py-2 text-sm text-gray-200 hover:bg-gray-800 cursor-pointer flex items-center gap-2"
                onclick={() => navigateTo(currentPath + '/' + entry.name)}
              >
                <span class="text-yellow-500 text-xs">&#128193;</span>
                <span>{entry.name}</span>
              </div>
            {:else}
              <div class="px-4 py-2 text-sm text-gray-500 flex items-center gap-2">
                <span class="text-xs">&#128196;</span>
                <span>{entry.name}</span>
              </div>
            {/if}
          {/each}
          {#if entries.length === 0}
            <div class="p-4 text-gray-500 text-sm text-center">Empty directory</div>
          {/if}
        </div>
      {/if}
    </div>

    <div class="px-4 py-3 border-t border-gray-800 flex items-center justify-between">
      <span class="text-xs text-gray-500 font-mono truncate max-w-[60%]" title={currentPath}>{currentPath}</span>
      <div class="flex gap-2">
        <button
          onclick={onClose}
          class="px-3 py-1.5 text-sm rounded-lg bg-gray-800 text-gray-300 hover:bg-gray-700 transition-colors"
        >
          Cancel
        </button>
        <button
          onclick={handleSelect}
          class="px-3 py-1.5 text-sm rounded-lg bg-purple-600 text-white hover:bg-purple-500 transition-colors"
        >
          Select
        </button>
      </div>
    </div>
  </div>
</div>
