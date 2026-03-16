<script lang="ts">
  import { knowledge, type KnowledgePattern } from '../api';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let patterns: KnowledgePattern[] = $state([]);
  let filterType: string = $state('');
  let filterRole: string = $state('');
  let selectedPattern: KnowledgePattern | null = $state(null);
  let showCreateForm: boolean = $state(false);
  let extracting: boolean = $state(false);
  let error: string | null = $state(null);

  // Form fields
  let newTitle: string = $state('');
  let newContent: string = $state('');
  let newType: string = $state('solution');
  let newRole: string = $state('');
  let newTaskType: string = $state('');
  let newKeywords: string = $state('');
  let newIsShared: boolean = $state(false);

  let sharedCount = $derived(patterns.filter(p => p.is_shared).length);

  async function fetchPatterns() {
    try {
      const params: { pattern_type?: string; role?: string } = {};
      if (filterType) params.pattern_type = filterType;
      if (filterRole) params.role = filterRole;
      patterns = await knowledge.list(projectId, params);
      error = null;
    } catch {
      error = 'Failed to load knowledge patterns';
    }
  }

  async function handleCreate() {
    try {
      const keywords = newKeywords.split(',').map(k => k.trim()).filter(Boolean);
      await knowledge.create(projectId, {
        pattern_type: newType,
        role: newRole || undefined,
        task_type: newTaskType || undefined,
        keywords: keywords.length > 0 ? keywords : undefined,
        title: newTitle,
        content: newContent,
        source_type: 'manual',
        is_shared: newIsShared,
      });
      showCreateForm = false;
      newTitle = '';
      newContent = '';
      newType = 'solution';
      newRole = '';
      newTaskType = '';
      newKeywords = '';
      newIsShared = false;
      await fetchPatterns();
    } catch {
      error = 'Failed to create pattern';
    }
  }

  async function handleDelete(id: string) {
    try {
      await knowledge.delete(projectId, id);
      if (selectedPattern?.id === id) selectedPattern = null;
      await fetchPatterns();
    } catch {
      error = 'Failed to delete pattern';
    }
  }

  async function handleExtract() {
    extracting = true;
    error = null;
    try {
      const result = await knowledge.extract(projectId);
      error = null;
      await fetchPatterns();
      // Show result briefly via error field (reused as info)
      if (result.extracted === 0) {
        error = 'No new patterns found to extract.';
      } else {
        error = `Extracted ${result.extracted} new pattern${result.extracted !== 1 ? 's' : ''}.`;
      }
    } catch {
      error = 'Extraction failed';
    } finally {
      extracting = false;
    }
  }

  function typeBadgeClass(type: string): string {
    switch (type) {
      case 'solution': return 'bg-green-500/20 text-green-400 border-green-500/30';
      case 'gotcha': return 'bg-amber-500/20 text-amber-400 border-amber-500/30';
      case 'preference': return 'bg-purple-500/20 text-purple-400 border-purple-500/30';
      case 'recipe': return 'bg-blue-500/20 text-blue-400 border-blue-500/30';
      default: return 'bg-gray-500/20 text-gray-400 border-gray-500/30';
    }
  }

  function confidenceColor(confidence: number): string {
    if (confidence > 0.7) return 'bg-green-500';
    if (confidence >= 0.4) return 'bg-yellow-500';
    return 'bg-red-500';
  }

  function parseFiles(filesJson: string | null): string[] {
    if (!filesJson) return [];
    try { return JSON.parse(filesJson); } catch { return []; }
  }

  function truncate(text: string, max: number): string {
    if (text.length <= max) return text;
    return text.slice(0, max) + '...';
  }

  $effect(() => {
    if (projectId) {
      fetchPatterns();
      const timer = setInterval(fetchPatterns, 30000);
      return () => clearInterval(timer);
    }
  });
</script>

<div class="space-y-4">
  <!-- Top bar -->
  <div class="flex items-center justify-between flex-wrap gap-2">
    <div class="flex items-center gap-3">
      <span class="text-xs text-gray-400">
        {patterns.length} pattern{patterns.length !== 1 ? 's' : ''}
        ({sharedCount} shared)
      </span>
      <select
        bind:value={filterType}
        onchange={() => fetchPatterns()}
        class="text-xs bg-gray-900 border border-gray-700 rounded px-2 py-1 text-gray-300 focus:border-purple-500 focus:outline-none"
      >
        <option value="">All Types</option>
        <option value="solution">Solution</option>
        <option value="gotcha">Gotcha</option>
        <option value="preference">Preference</option>
        <option value="recipe">Recipe</option>
      </select>
      <input
        type="text"
        bind:value={filterRole}
        onchange={() => fetchPatterns()}
        placeholder="Filter by role..."
        class="text-xs bg-gray-900 border border-gray-700 rounded px-2 py-1 text-gray-300 placeholder-gray-600 focus:border-purple-500 focus:outline-none w-32"
      />
    </div>
    <div class="flex items-center gap-2">
      <button
        onclick={handleExtract}
        disabled={extracting}
        class="px-3 py-1.5 text-xs font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {extracting ? 'Extracting...' : 'Extract Now'}
      </button>
      <button
        onclick={() => showCreateForm = !showCreateForm}
        class="px-3 py-1.5 text-xs font-medium rounded-lg bg-green-600 hover:bg-green-500 text-white transition-colors"
      >
        {showCreateForm ? 'Cancel' : 'Add Pattern'}
      </button>
    </div>
  </div>

  {#if error}
    <div class="bg-gray-900 border border-gray-800 rounded-lg p-3 text-sm text-gray-300">{error}</div>
  {/if}

  <!-- Create form -->
  {#if showCreateForm}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-3">
      <h3 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">New Pattern</h3>
      <input
        type="text"
        bind:value={newTitle}
        placeholder="Title"
        class="w-full text-sm bg-gray-950 border border-gray-700 rounded-lg px-3 py-2 text-gray-200 placeholder-gray-600 focus:border-purple-500 focus:outline-none"
      />
      <textarea
        bind:value={newContent}
        placeholder="Content — describe the pattern, solution, or gotcha..."
        rows="4"
        class="w-full text-sm bg-gray-950 border border-gray-700 rounded-lg px-3 py-2 text-gray-200 placeholder-gray-600 focus:border-purple-500 focus:outline-none resize-y"
      ></textarea>
      <div class="grid grid-cols-4 gap-3">
        <div>
          <label class="text-[10px] text-gray-500 uppercase tracking-wider block mb-1">Type</label>
          <select
            bind:value={newType}
            class="w-full text-xs bg-gray-950 border border-gray-700 rounded px-2 py-1.5 text-gray-300 focus:border-purple-500 focus:outline-none"
          >
            <option value="solution">Solution</option>
            <option value="gotcha">Gotcha</option>
            <option value="preference">Preference</option>
            <option value="recipe">Recipe</option>
          </select>
        </div>
        <div>
          <label class="text-[10px] text-gray-500 uppercase tracking-wider block mb-1">Role (optional)</label>
          <input
            type="text"
            bind:value={newRole}
            placeholder="e.g. Architect"
            class="w-full text-xs bg-gray-950 border border-gray-700 rounded px-2 py-1.5 text-gray-300 placeholder-gray-600 focus:border-purple-500 focus:outline-none"
          />
        </div>
        <div>
          <label class="text-[10px] text-gray-500 uppercase tracking-wider block mb-1">Task Type (optional)</label>
          <input
            type="text"
            bind:value={newTaskType}
            placeholder="e.g. refactor"
            class="w-full text-xs bg-gray-950 border border-gray-700 rounded px-2 py-1.5 text-gray-300 placeholder-gray-600 focus:border-purple-500 focus:outline-none"
          />
        </div>
        <div>
          <label class="text-[10px] text-gray-500 uppercase tracking-wider block mb-1">Keywords (comma-sep)</label>
          <input
            type="text"
            bind:value={newKeywords}
            placeholder="e.g. svelte, api, css"
            class="w-full text-xs bg-gray-950 border border-gray-700 rounded px-2 py-1.5 text-gray-300 placeholder-gray-600 focus:border-purple-500 focus:outline-none"
          />
        </div>
      </div>
      <div class="flex items-center justify-between">
        <label class="flex items-center gap-2 text-xs text-gray-400 cursor-pointer">
          <input type="checkbox" bind:checked={newIsShared} class="rounded border-gray-600 bg-gray-950 text-purple-500 focus:ring-purple-500" />
          Share across projects
        </label>
        <div class="flex items-center gap-2">
          <button
            onclick={() => showCreateForm = false}
            class="px-3 py-1.5 text-xs font-medium rounded-lg bg-gray-800 hover:bg-gray-700 text-gray-300 transition-colors"
          >
            Cancel
          </button>
          <button
            onclick={handleCreate}
            disabled={!newTitle.trim() || !newContent.trim()}
            class="px-3 py-1.5 text-xs font-medium rounded-lg bg-green-600 hover:bg-green-500 text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Create
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Pattern cards -->
  <div class="grid grid-cols-1 gap-3">
    {#each patterns as pattern (pattern.id)}
      {@const files = parseFiles(pattern.files_involved)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="group rounded-xl bg-gray-900 border border-gray-800 p-4 hover:border-gray-700 transition-colors cursor-pointer"
        onclick={() => selectedPattern = selectedPattern?.id === pattern.id ? null : pattern}
      >
        <div class="flex items-start justify-between gap-3">
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2 mb-1.5">
              <span class="text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded border {typeBadgeClass(pattern.pattern_type)}">
                {pattern.pattern_type}
              </span>
              {#if pattern.is_shared}
                <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-purple-500/20 text-purple-400 border border-purple-500/30">shared</span>
              {/if}
              {#if pattern.role}
                <span class="text-[10px] text-gray-500">{pattern.role}</span>
              {/if}
            </div>
            <h4 class="text-sm font-semibold text-gray-200 mb-1">{pattern.title}</h4>
            <p class="text-xs text-gray-400 leading-relaxed">
              {selectedPattern?.id === pattern.id ? pattern.content : truncate(pattern.content, 180)}
            </p>
          </div>
          <button
            onclick={(e: MouseEvent) => { e.stopPropagation(); handleDelete(pattern.id); }}
            class="opacity-0 group-hover:opacity-100 text-gray-600 hover:text-red-400 transition-all text-sm shrink-0"
            title="Delete pattern"
          >
            x
          </button>
        </div>

        <!-- Confidence bar -->
        <div class="mt-3 flex items-center gap-3">
          <div class="flex-1 h-1 rounded-full bg-gray-800 overflow-hidden">
            <div
              class="h-full rounded-full transition-all {confidenceColor(pattern.confidence)}"
              style="width: {Math.round(pattern.confidence * 100)}%"
            ></div>
          </div>
          <span class="text-[10px] text-gray-500 shrink-0">
            {Math.round(pattern.confidence * 100)}%
          </span>
        </div>

        <!-- Meta row -->
        <div class="mt-2 flex items-center gap-3 text-[10px] text-gray-500">
          <span>{pattern.observations} observation{pattern.observations !== 1 ? 's' : ''}</span>
          <span class="text-gray-700">|</span>
          <span>{pattern.source_type}</span>
          {#if pattern.task_type}
            <span class="text-gray-700">|</span>
            <span>{pattern.task_type}</span>
          {/if}
        </div>

        <!-- Files involved -->
        {#if files.length > 0}
          <div class="mt-2 flex flex-wrap gap-1">
            {#each files as file}
              <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-gray-800 text-gray-400 border border-gray-700">{file}</span>
            {/each}
          </div>
        {/if}
      </div>
    {/each}

    {#if patterns.length === 0}
      <div class="text-center text-gray-600 py-12 text-sm">
        No knowledge patterns yet. Click "Extract Now" to discover patterns from project data, or add one manually.
      </div>
    {/if}
  </div>
</div>
