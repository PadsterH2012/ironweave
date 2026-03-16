<script lang="ts">
  import { projectDocuments, features, type ProjectDocument, type GapAnalysis } from '../api';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let intentDoc: ProjectDocument | null = $state(null);
  let realityDoc: ProjectDocument | null = $state(null);
  let gaps: GapAnalysis | null = $state(null);
  let saving: boolean = $state(false);
  let scanning: boolean = $state(false);
  let removals: string[] = $state([]);
  let showRemovals: boolean = $state(false);
  let error: string | null = $state(null);

  let intentContent: string = $state('');

  async function fetchAll() {
    try {
      const [intent, reality, gapData] = await Promise.allSettled([
        projectDocuments.get(projectId, 'intent'),
        projectDocuments.get(projectId, 'reality'),
        projectDocuments.gaps(projectId),
      ]);
      if (intent.status === 'fulfilled') {
        intentDoc = intent.value;
        intentContent = intent.value.content;
      }
      if (reality.status === 'fulfilled') {
        realityDoc = reality.value;
      }
      if (gapData.status === 'fulfilled') {
        gaps = gapData.value;
      }
      error = null;
    } catch {
      error = 'Failed to load project documents';
    }
  }

  async function handleSaveIntent() {
    saving = true;
    try {
      const result = await projectDocuments.update(projectId, 'intent', intentContent);
      intentDoc = result.document;
      if (result.removals && result.removals.length > 0) {
        removals = result.removals;
        showRemovals = true;
      }
      // Refresh gaps after save
      try {
        gaps = await projectDocuments.gaps(projectId);
      } catch { /* ignore */ }
      error = null;
    } catch {
      error = 'Failed to save intent document';
    } finally {
      saving = false;
    }
  }

  async function handleRescan() {
    scanning = true;
    try {
      await projectDocuments.scan(projectId);
      // Refetch reality and gaps
      try {
        realityDoc = await projectDocuments.get(projectId, 'reality');
      } catch { /* may not exist yet */ }
      try {
        gaps = await projectDocuments.gaps(projectId);
      } catch { /* ignore */ }
      error = null;
    } catch {
      error = 'Failed to scan project';
    } finally {
      scanning = false;
    }
  }

  async function handleCreateFeatureFromGap(title: string) {
    try {
      await features.create(projectId, { title, status: 'idea' });
      // Refresh gaps
      try {
        gaps = await projectDocuments.gaps(projectId);
      } catch { /* ignore */ }
    } catch {
      error = 'Failed to create feature';
    }
  }

  $effect(() => {
    fetchAll();
  });
</script>

<div class="space-y-6">
  <!-- Error banner -->
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
      <button onclick={() => { error = null; }} class="ml-2 text-red-400 hover:text-red-200">Dismiss</button>
    </div>
  {/if}

  <!-- Intent & Reality columns -->
  <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
    <!-- Intent -->
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 flex flex-col">
      <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
          <h3 class="text-sm font-semibold text-gray-300">Intent</h3>
          {#if intentDoc}
            <span class="text-xs px-2 py-0.5 rounded-full bg-gray-800 text-gray-400">v{intentDoc.version}</span>
          {/if}
        </div>
        <button
          onclick={handleSaveIntent}
          disabled={saving}
          class="px-3 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors disabled:opacity-50"
        >
          {saving ? 'Saving...' : 'Save'}
        </button>
      </div>
      <textarea
        bind:value={intentContent}
        placeholder="Describe what this project should be: goals, features, keywords..."
        class="flex-1 min-h-[300px] w-full rounded-lg bg-gray-950 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500 resize-y"
      ></textarea>

      <!-- Removals warning -->
      {#if showRemovals && removals.length > 0}
        <div class="mt-3 rounded-lg bg-amber-900/20 border border-amber-800/40 px-4 py-3">
          <div class="flex items-center justify-between mb-2">
            <span class="text-xs font-medium text-amber-300">Lines removed from previous version:</span>
            <button
              onclick={() => { showRemovals = false; removals = []; }}
              class="text-xs text-amber-400 hover:text-amber-200"
            >
              Dismiss
            </button>
          </div>
          <ul class="space-y-1">
            {#each removals as line}
              <li class="text-xs text-amber-400 font-mono">- {line}</li>
            {/each}
          </ul>
        </div>
      {/if}
    </div>

    <!-- Reality -->
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 flex flex-col">
      <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
          <h3 class="text-sm font-semibold text-gray-300">Reality</h3>
          {#if realityDoc}
            <span class="text-xs text-gray-500">Updated {new Date(realityDoc.updated_at).toLocaleDateString()}</span>
          {/if}
        </div>
        <button
          onclick={handleRescan}
          disabled={scanning}
          class="px-3 py-1.5 text-sm font-medium rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors disabled:opacity-50"
        >
          {scanning ? 'Scanning...' : 'Rescan'}
        </button>
      </div>
      {#if realityDoc && realityDoc.content}
        <pre class="flex-1 min-h-[300px] rounded-lg bg-gray-950 border border-gray-700 text-gray-300 px-3 py-2 text-sm font-mono overflow-auto whitespace-pre-wrap">{realityDoc.content}</pre>
      {:else}
        <div class="flex-1 min-h-[300px] flex items-center justify-center rounded-lg bg-gray-950 border border-gray-700 text-gray-500 text-sm">
          No reality scan yet. Click Rescan to generate.
        </div>
      {/if}
    </div>
  </div>

  <!-- Gap Analysis -->
  {#if gaps && ((gaps.missing && gaps.missing.length > 0) || (gaps.undocumented && gaps.undocumented.length > 0))}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5">
      <h3 class="text-sm font-semibold text-gray-300 mb-4">Gap Analysis</h3>
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <!-- Missing from code -->
        <div>
          <h4 class="text-xs font-medium text-red-400 uppercase tracking-wider mb-2">Missing from code</h4>
          {#if gaps.missing && gaps.missing.length > 0}
            <div class="space-y-1">
              {#each gaps.missing as item}
                <div class="flex items-center justify-between rounded-lg bg-gray-800/40 px-3 py-2">
                  <span class="text-sm text-red-300">{item}</span>
                  <button
                    onclick={() => handleCreateFeatureFromGap(item)}
                    class="text-xs px-2 py-1 rounded bg-purple-600 hover:bg-purple-500 text-white transition-colors"
                  >
                    Create Feature
                  </button>
                </div>
              {/each}
            </div>
          {:else}
            <p class="text-xs text-gray-600">None detected.</p>
          {/if}
        </div>

        <!-- Undocumented -->
        <div>
          <h4 class="text-xs font-medium text-amber-400 uppercase tracking-wider mb-2">Undocumented</h4>
          {#if gaps.undocumented && gaps.undocumented.length > 0}
            <div class="space-y-1">
              {#each gaps.undocumented as item}
                <div class="rounded-lg bg-gray-800/40 px-3 py-2">
                  <span class="text-sm text-amber-300">{item}</span>
                </div>
              {/each}
            </div>
          {:else}
            <p class="text-xs text-gray-600">None detected.</p>
          {/if}
        </div>
      </div>
    </div>
  {/if}
</div>
