<script lang="ts">
  import { features, featureTasks, type Feature, type FeatureTask, type FeatureWithTasks } from '../api';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let featureList: Feature[] = $state([]);
  let statusFilter: string = $state('');
  let selectedFeature: FeatureWithTasks | null = $state(null);
  let showCreateForm: boolean = $state(false);
  let showImportModal: boolean = $state(false);
  let expandedFeatureId: string | null = $state(null);
  let featureTasksMap: Record<string, FeatureTask[]> = $state({});
  let error: string | null = $state(null);

  // Create form fields
  let newTitle: string = $state('');
  let newDescription: string = $state('');
  let newPriority: number = $state(5);
  let newPrdContent: string = $state('');
  let creating: boolean = $state(false);

  // Import modal
  let importText: string = $state('');
  let importing: boolean = $state(false);

  // Add task inline
  let addingTaskForFeature: string | null = $state(null);
  let newTaskTitle: string = $state('');

  // Implementation notes editing
  let editingNotesId: string | null = $state(null);
  let editNotesContent: string = $state('');

  // Gap analysis
  let analyzingGaps: Record<string, boolean> = $state({});
  let gapResults: Record<string, any> = $state({});

  const statusFilters = [
    { key: '', label: 'All' },
    { key: 'idea', label: 'Ideas', icon: '💭' },
    { key: 'designed', label: 'Designed', icon: '📋' },
    { key: 'in_progress', label: 'In Progress', icon: '🚧' },
    { key: 'implemented', label: 'Implemented', icon: '✅' },
    { key: 'verified', label: 'Verified', icon: '🔍' },
    { key: 'parked', label: 'Parked', icon: '⏸️' },
  ];

  function statusBadgeClass(status: string): string {
    switch (status) {
      case 'idea': return 'bg-gray-700 text-gray-300';
      case 'designed': return 'bg-blue-900/60 text-blue-300';
      case 'in_progress': return 'bg-yellow-900/60 text-yellow-300';
      case 'implemented': return 'bg-green-900/60 text-green-300';
      case 'verified': return 'bg-purple-900/60 text-purple-300';
      case 'parked': return 'bg-amber-900/60 text-amber-300';
      case 'abandoned': return 'bg-red-900/60 text-red-300 line-through';
      default: return 'bg-gray-700 text-gray-300';
    }
  }

  let filteredFeatures = $derived(
    statusFilter
      ? featureList.filter(f => f.status === statusFilter)
      : featureList
  );

  async function fetchFeatures() {
    try {
      const params: { status?: string } = {};
      if (statusFilter) params.status = statusFilter;
      featureList = await features.list(projectId);
      error = null;
    } catch {
      error = 'Failed to load features';
    }
  }

  async function toggleExpand(feature: Feature) {
    if (expandedFeatureId === feature.id) {
      expandedFeatureId = null;
      selectedFeature = null;
      return;
    }
    expandedFeatureId = feature.id;
    try {
      selectedFeature = await features.get(projectId, feature.id);
      featureTasksMap[feature.id] = selectedFeature.tasks;
    } catch {
      error = 'Failed to load feature details';
    }
  }

  async function handleCreate() {
    if (!newTitle.trim()) return;
    creating = true;
    try {
      await features.create(projectId, {
        title: newTitle.trim(),
        description: newDescription.trim() || undefined,
        priority: newPriority,
        prd_content: newPrdContent.trim() || undefined,
      });
      showCreateForm = false;
      newTitle = '';
      newDescription = '';
      newPriority = 5;
      newPrdContent = '';
      await fetchFeatures();
    } catch {
      error = 'Failed to create feature';
    } finally {
      creating = false;
    }
  }

  async function handleImport() {
    if (!importText.trim()) return;
    importing = true;
    try {
      await features.import(projectId, importText.trim());
      showImportModal = false;
      importText = '';
      await fetchFeatures();
    } catch {
      error = 'Failed to import PRD';
    } finally {
      importing = false;
    }
  }

  async function handlePark(featureId: string) {
    try {
      await features.park(projectId, featureId, 'Parked from UI');
      await fetchFeatures();
      if (expandedFeatureId === featureId) {
        selectedFeature = await features.get(projectId, featureId);
      }
    } catch {
      error = 'Failed to park feature';
    }
  }

  async function handleVerify(featureId: string) {
    try {
      await features.verify(projectId, featureId);
      await fetchFeatures();
      if (expandedFeatureId === featureId) {
        selectedFeature = await features.get(projectId, featureId);
      }
    } catch {
      error = 'Failed to verify feature';
    }
  }

  async function handleAbandon(featureId: string) {
    try {
      await features.update(projectId, featureId, { status: 'abandoned' } as Partial<Feature>);
      await fetchFeatures();
      if (expandedFeatureId === featureId) {
        selectedFeature = await features.get(projectId, featureId);
      }
    } catch {
      error = 'Failed to abandon feature';
    }
  }

  async function handleAddTask(featureId: string) {
    if (!newTaskTitle.trim()) return;
    try {
      await featureTasks.create(featureId, { title: newTaskTitle.trim() });
      newTaskTitle = '';
      addingTaskForFeature = null;
      selectedFeature = await features.get(projectId, featureId);
      featureTasksMap[featureId] = selectedFeature.tasks;
    } catch {
      error = 'Failed to add task';
    }
  }

  async function handleToggleTask(featureId: string, task: FeatureTask) {
    const newStatus = task.status === 'done' ? 'todo' : 'done';
    try {
      await featureTasks.update(featureId, task.id, { status: newStatus });
      selectedFeature = await features.get(projectId, featureId);
      featureTasksMap[featureId] = selectedFeature.tasks;
    } catch {
      error = 'Failed to update task';
    }
  }

  async function handleImplementTask(featureId: string, taskId: string) {
    try {
      await featureTasks.implement(featureId, taskId);
      selectedFeature = await features.get(projectId, featureId);
      featureTasksMap[featureId] = selectedFeature.tasks;
    } catch {
      error = 'Failed to implement task';
    }
  }

  async function analyzeGaps(featureId: string) {
    analyzingGaps[featureId] = true;
    analyzingGaps = { ...analyzingGaps };
    try {
      const result = await features.gaps(projectId, featureId);
      gapResults[featureId] = result;
      gapResults = { ...gapResults };
    } catch (e) {
      error = 'Gap analysis failed';
    } finally {
      analyzingGaps[featureId] = false;
      analyzingGaps = { ...analyzingGaps };
    }
  }

  async function handleSaveNotes(featureId: string) {
    try {
      await features.update(projectId, featureId, { implementation_notes: editNotesContent } as Partial<Feature>);
      editingNotesId = null;
      if (expandedFeatureId === featureId) {
        selectedFeature = await features.get(projectId, featureId);
      }
    } catch {
      error = 'Failed to save notes';
    }
  }

  function taskProgress(featureId: string): { done: number; total: number } {
    const tasks = featureTasksMap[featureId];
    if (!tasks || tasks.length === 0) return { done: 0, total: 0 };
    const done = tasks.filter(t => t.status === 'done').length;
    return { done, total: tasks.length };
  }

  $effect(() => {
    fetchFeatures();
    const interval = setInterval(fetchFeatures, 30000);
    return () => clearInterval(interval);
  });
</script>

<div class="space-y-4">
  <!-- Error banner -->
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
      <button onclick={() => { error = null; }} class="ml-2 text-red-400 hover:text-red-200">Dismiss</button>
    </div>
  {/if}

  <!-- Top bar -->
  <div class="flex flex-wrap items-center gap-2">
    <!-- Status filters -->
    {#each statusFilters as sf}
      <button
        onclick={() => { statusFilter = sf.key; }}
        class="px-3 py-1.5 text-xs font-medium rounded-lg transition-colors {statusFilter === sf.key ? 'bg-purple-600 text-white' : 'bg-gray-800 text-gray-400 hover:text-gray-200 hover:bg-gray-700'}"
      >
        {#if sf.icon}{sf.icon} {/if}{sf.label}
      </button>
    {/each}

    <div class="flex-1"></div>

    <button
      onclick={() => { showImportModal = true; }}
      class="px-3 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
    >
      Import PRD
    </button>
    <button
      onclick={() => { showCreateForm = !showCreateForm; }}
      class="px-3 py-1.5 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 text-white transition-colors"
    >
      {showCreateForm ? 'Cancel' : 'Add Feature'}
    </button>
  </div>

  <!-- Create form -->
  {#if showCreateForm}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <h3 class="text-sm font-semibold text-gray-300">New Feature</h3>
      <div class="space-y-3">
        <div>
          <label for="feat-title" class="block text-sm font-medium text-gray-400 mb-1">Title</label>
          <input
            id="feat-title"
            type="text"
            bind:value={newTitle}
            placeholder="Feature title"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          />
        </div>
        <div>
          <label for="feat-desc" class="block text-sm font-medium text-gray-400 mb-1">Description</label>
          <textarea
            id="feat-desc"
            bind:value={newDescription}
            placeholder="Feature description"
            rows="3"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          ></textarea>
        </div>
        <div>
          <label for="feat-priority" class="block text-sm font-medium text-gray-400 mb-1">Priority: {newPriority}</label>
          <input
            id="feat-priority"
            type="range"
            min="1"
            max="10"
            bind:value={newPriority}
            class="w-full accent-purple-500"
          />
        </div>
        <div>
          <label for="feat-prd" class="block text-sm font-medium text-gray-400 mb-1">PRD Content (optional)</label>
          <textarea
            id="feat-prd"
            bind:value={newPrdContent}
            placeholder="Paste PRD content..."
            rows="4"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500"
          ></textarea>
        </div>
      </div>
      <div class="flex gap-2">
        <button
          onclick={handleCreate}
          disabled={creating || !newTitle.trim()}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 text-white transition-colors disabled:opacity-50"
        >
          {creating ? 'Creating...' : 'Create'}
        </button>
        <button
          onclick={() => { showCreateForm = false; }}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}

  <!-- Import modal -->
  {#if showImportModal}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <h3 class="text-sm font-semibold text-gray-300">Import PRD</h3>
      <textarea
        bind:value={importText}
        placeholder="Paste your PRD or feature description..."
        rows="8"
        class="w-full rounded-lg bg-gray-950 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500"
      ></textarea>
      <div class="flex gap-2">
        <button
          onclick={handleImport}
          disabled={importing || !importText.trim()}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors disabled:opacity-50"
        >
          {importing ? 'Importing...' : 'Import'}
        </button>
        <button
          onclick={() => { showImportModal = false; }}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}

  <!-- Feature list -->
  {#if filteredFeatures.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500 text-sm">
      {statusFilter ? `No ${statusFilter} features.` : 'No features yet. Add one or import a PRD.'}
    </div>
  {:else}
    <div class="space-y-3">
      {#each filteredFeatures as feature (feature.id)}
        <div class="rounded-xl bg-gray-900 border border-gray-800 overflow-hidden">
          <!-- Feature header -->
          <button
            onclick={() => toggleExpand(feature)}
            class="w-full flex items-center gap-3 px-5 py-4 text-left hover:bg-gray-800/40 transition-colors"
          >
            <span class="text-xs font-medium px-2 py-0.5 rounded-full {statusBadgeClass(feature.status)}">
              {feature.status.replace('_', ' ')}
            </span>
            <span class="flex-1 text-sm font-semibold text-gray-200 truncate">{feature.title}</span>
            {#if feature.gap_status === 'complete'}
              <span class="text-[10px] font-bold px-1.5 py-0.5 rounded {feature.gap_not_found === 0 ? 'bg-green-600/20 text-green-400' : 'bg-red-600/20 text-red-400'}">
                GA:{feature.gap_not_found}
              </span>
            {:else if feature.gap_status === 'pending'}
              <span class="text-[10px] font-bold px-1.5 py-0.5 rounded bg-blue-600/20 text-blue-400 animate-pulse">
                GA:...
              </span>
            {/if}
            <span class="text-xs font-mono px-2 py-0.5 rounded bg-gray-800 text-gray-400">P{feature.priority}</span>
            {#if featureTasksMap[feature.id]}
              {@const prog = taskProgress(feature.id)}
              {#if prog.total > 0}
                <div class="flex items-center gap-2">
                  <div class="w-16 h-1.5 rounded-full bg-gray-700 overflow-hidden">
                    <div class="h-full bg-green-500 rounded-full" style="width: {(prog.done / prog.total) * 100}%"></div>
                  </div>
                  <span class="text-xs text-gray-500">{prog.done}/{prog.total}</span>
                </div>
              {/if}
            {/if}
            <span class="text-gray-500 text-sm">{expandedFeatureId === feature.id ? '▾' : '▸'}</span>
          </button>

          <!-- Expanded content -->
          {#if expandedFeatureId === feature.id && selectedFeature}
            <div class="border-t border-gray-800 px-5 py-4 space-y-4">
              <!-- Description -->
              {#if selectedFeature.description}
                <div>
                  <h4 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-1">Description</h4>
                  <p class="text-sm text-gray-300">{selectedFeature.description}</p>
                </div>
              {/if}

              <!-- PRD Content -->
              {#if selectedFeature.prd_content}
                <details class="group">
                  <summary class="text-xs font-medium text-gray-500 uppercase tracking-wider cursor-pointer hover:text-gray-300">
                    PRD Content
                  </summary>
                  <pre class="mt-2 text-sm text-gray-300 bg-gray-950 rounded-lg p-3 overflow-auto max-h-64 font-mono whitespace-pre-wrap">{selectedFeature.prd_content}</pre>
                </details>
              {/if}

              <!-- Gap Analysis Results (from enriched get endpoint) -->
              {#if selectedFeature.gap_summary}
                <details class="group">
                  <summary class="text-xs font-medium text-gray-500 uppercase tracking-wider cursor-pointer hover:text-gray-300">
                    Gap Analysis — {selectedFeature.gap_found} found · {selectedFeature.gap_partial} partial · {selectedFeature.gap_not_found} missing
                  </summary>
                  <pre class="mt-2 text-xs text-gray-300 bg-gray-950 rounded-lg p-3 overflow-auto max-h-64 whitespace-pre-wrap">{selectedFeature.gap_summary}</pre>
                </details>
              {/if}

              <!-- Action buttons (before tasks so they're visible without scrolling) -->
              <div class="flex gap-2 py-2 border-y border-gray-800">
                {#if selectedFeature.status !== 'parked' && selectedFeature.status !== 'abandoned'}
                  <button
                    onclick={() => handlePark(feature.id)}
                    class="px-3 py-1.5 text-xs font-medium rounded-lg bg-amber-600 hover:bg-amber-500 text-white transition-colors"
                  >
                    Park
                  </button>
                {/if}
                {#if selectedFeature.status === 'implemented'}
                  <button
                    onclick={() => handleVerify(feature.id)}
                    class="px-3 py-1.5 text-xs font-medium rounded-lg bg-green-600 hover:bg-green-500 text-white transition-colors"
                  >
                    Verify
                  </button>
                {/if}
                <button
                  onclick={() => handleAbandon(feature.id)}
                  class="px-3 py-1.5 text-xs font-medium rounded-lg bg-red-600 hover:bg-red-500 text-white transition-colors"
                >
                  Abandon
                </button>
                <button
                  onclick={() => analyzeGaps(feature.id)}
                  disabled={analyzingGaps[feature.id]}
                  class="px-3 py-1.5 text-xs font-medium rounded-lg bg-cyan-600 hover:bg-cyan-500 text-white transition-colors disabled:opacity-50"
                >
                  {analyzingGaps[feature.id] ? 'Dispatching...' : 'Request Gap Analysis'}
                </button>
              </div>

              <!-- Gap Analysis Results -->
              {#if gapResults[feature.id]}
                {@const result = gapResults[feature.id]}
                <div class="rounded-lg bg-cyan-900/20 border border-cyan-800/40 px-3 py-2 text-xs text-cyan-300">
                  {result.message}
                  <span class="text-gray-500 ml-2">Issue: {result.issue_id.slice(0, 8)}</span>
                </div>
              {/if}

              <!-- Tasks -->
              <div>
                <h4 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-2">Tasks</h4>
                {#if selectedFeature.tasks.length === 0}
                  <p class="text-xs text-gray-600">No tasks yet.</p>
                {:else}
                  <div class="space-y-1">
                    {#each selectedFeature.tasks as task (task.id)}
                      <div class="flex items-center gap-2 rounded-lg px-3 py-2 bg-gray-800/40">
                        <button
                          onclick={() => handleToggleTask(feature.id, task)}
                          class="flex-shrink-0"
                        >
                          {#if task.status === 'done'}
                            <span class="text-green-400">&#10003;</span>
                          {:else if task.status === 'skipped'}
                            <span class="text-gray-600 line-through">&#9711;</span>
                          {:else}
                            <span class="text-gray-500">&#9711;</span>
                          {/if}
                        </button>
                        <span class="flex-1 text-sm {task.status === 'done' ? 'text-gray-500 line-through' : task.status === 'skipped' ? 'text-gray-600 line-through' : 'text-gray-300'}">
                          {task.title}
                        </span>
                        {#if task.issue_id}
                          <span class="text-xs px-2 py-0.5 rounded-full bg-purple-900/50 text-purple-300 font-mono">
                            {task.issue_id.slice(0, 8)}
                          </span>
                        {:else if task.status === 'todo'}
                          <button
                            onclick={() => handleImplementTask(feature.id, task.id)}
                            class="text-xs px-2 py-1 rounded bg-purple-600 hover:bg-purple-500 text-white transition-colors"
                          >
                            Implement
                          </button>
                        {/if}
                      </div>
                    {/each}
                  </div>
                {/if}

                <!-- Add task inline -->
                {#if addingTaskForFeature === feature.id}
                  <div class="flex gap-2 mt-2">
                    <input
                      type="text"
                      bind:value={newTaskTitle}
                      placeholder="Task title..."
                      class="flex-1 rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-1.5 text-sm focus:outline-none focus:border-purple-500"
                      onkeydown={(e: KeyboardEvent) => { if (e.key === 'Enter') handleAddTask(feature.id); }}
                    />
                    <button
                      onclick={() => handleAddTask(feature.id)}
                      disabled={!newTaskTitle.trim()}
                      class="px-3 py-1.5 text-sm rounded-lg bg-green-600 hover:bg-green-500 text-white disabled:opacity-50 transition-colors"
                    >
                      Add
                    </button>
                    <button
                      onclick={() => { addingTaskForFeature = null; newTaskTitle = ''; }}
                      class="px-3 py-1.5 text-sm rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors"
                    >
                      Cancel
                    </button>
                  </div>
                {:else}
                  <button
                    onclick={() => { addingTaskForFeature = feature.id; }}
                    class="mt-2 text-xs text-purple-400 hover:text-purple-300"
                  >
                    + Add Task
                  </button>
                {/if}
              </div>

              <!-- Implementation Notes -->
              {#if ['implemented', 'verified'].includes(selectedFeature.status)}
                <div>
                  <h4 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-1">Implementation Notes</h4>
                  {#if editingNotesId === feature.id}
                    <textarea
                      bind:value={editNotesContent}
                      rows="4"
                      class="w-full rounded-lg bg-gray-950 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
                    ></textarea>
                    <div class="flex gap-2 mt-2">
                      <button
                        onclick={() => handleSaveNotes(feature.id)}
                        class="px-3 py-1.5 text-sm rounded-lg bg-green-600 hover:bg-green-500 text-white transition-colors"
                      >
                        Save
                      </button>
                      <button
                        onclick={() => { editingNotesId = null; }}
                        class="px-3 py-1.5 text-sm rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors"
                      >
                        Cancel
                      </button>
                    </div>
                  {:else}
                    <p class="text-sm text-gray-400">{selectedFeature.implementation_notes || 'No notes yet.'}</p>
                    <button
                      onclick={() => { editingNotesId = feature.id; editNotesContent = selectedFeature?.implementation_notes || ''; }}
                      class="mt-1 text-xs text-purple-400 hover:text-purple-300"
                    >
                      Edit Notes
                    </button>
                  {/if}
                </div>
              {/if}

              <!-- Parked info -->
              {#if selectedFeature.status === 'parked' && selectedFeature.parked_reason}
                <div class="rounded-lg bg-amber-900/20 border border-amber-800/40 px-3 py-2">
                  <p class="text-xs text-amber-300">Parked: {selectedFeature.parked_reason}</p>
                </div>
              {/if}

              <!-- (Action buttons moved above tasks) -->
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
