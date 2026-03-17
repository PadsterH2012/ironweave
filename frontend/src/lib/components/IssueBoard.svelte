<script lang="ts">
  import { issues, type Issue, type CreateIssue, PREDEFINED_ROLES, authHeaders } from '../api';

  interface Props {
    projectId: string;
  }
  let { projectId }: Props = $props();

  let issueList: Issue[] = $state([]);
  let error: string | null = $state(null);
  let showCreateForm: boolean = $state(false);

  // Create form fields
  let newTitle: string = $state('');
  let newDescription: string = $state('');
  let newType: string = $state('task');
  let newPriority: number = $state(3);
  let newRole: string = $state('');
  let newScopeMode: string = $state('auto');
  let creating: boolean = $state(false);

  // Modal state
  let selectedIssue: Issue | null = $state(null);
  let childIssues: Issue[] = $state([]);
  let loadingChildren: boolean = $state(false);

  // Drag state
  let draggedIssueId: string | null = $state(null);

  // Expand/collapse state for parent issues
  let expandedParents: Set<string> = $state(new Set());

  const allColumns = [
    { key: 'backlog', label: 'Backlog' },
    { key: 'open', label: 'Open' },
    { key: 'in_progress', label: 'In Progress' },
    { key: 'on_hold', label: 'On Hold' },
    { key: 'review', label: 'Review' },
    { key: 'closed', label: 'Closed' },
  ];

  let hiddenColumns: Record<string, boolean> = $state({
    'backlog': false,
    'on_hold': false,
    'closed': false,
  });

  let columns = $derived(allColumns.filter(c => !hiddenColumns[c.key]));
  let showColumnSettings: boolean = $state(false);

  // Only show top-level issues (no parent_id) in columns
  function issuesByStatus(status: string): Issue[] {
    return issueList.filter((i) => i.status === status && !i.parent_id);
  }

  // Get children of a parent issue, optionally filtered by status
  function childrenOf(parentId: string, status?: string): Issue[] {
    return issueList.filter((i) => i.parent_id === parentId && (!status || i.status === status));
  }

  function toggleExpand(issueId: string, e: Event) {
    e.stopPropagation();
    const next = new Set(expandedParents);
    if (next.has(issueId)) {
      next.delete(issueId);
    } else {
      next.add(issueId);
    }
    expandedParents = next;
  }

  function hasChildren(issueId: string): boolean {
    return issueList.some((i) => i.parent_id === issueId);
  }

  async function fetchIssues() {
    try {
      issueList = await issues.list(projectId);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch issues';
    }
  }

  $effect(() => {
    // Re-run when projectId changes
    const pid = projectId;
    if (pid) {
      fetchIssues();
      const interval = setInterval(fetchIssues, 5000);
      return () => clearInterval(interval);
    }
  });

  async function handleCreate() {
    if (!newTitle.trim()) return;
    creating = true;
    try {
      const data: CreateIssue = {
        project_id: projectId,
        title: newTitle.trim(),
        description: newDescription.trim(),
        issue_type: newType,
        priority: newPriority,
        role: newRole || undefined,
        scope_mode: newScopeMode,
      };
      await issues.create(projectId, data);
      newTitle = '';
      newDescription = '';
      newType = 'task';
      newPriority = 3;
      newRole = '';
      newScopeMode = 'auto';
      showCreateForm = false;
      await fetchIssues();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to create issue';
    } finally {
      creating = false;
    }
  }

  async function handleDelete(id: string) {
    if (!confirm('Delete this issue?')) return;
    try {
      await issues.delete(projectId, id);
      await fetchIssues();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete issue';
    }
  }

  function handleDragStart(e: DragEvent, issueId: string) {
    draggedIssueId = issueId;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', issueId);
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = 'move';
    }
  }

  async function handleDrop(e: DragEvent, targetStatus: string) {
    e.preventDefault();
    if (!draggedIssueId) return;

    const issue = issueList.find((i) => i.id === draggedIssueId);
    if (!issue || issue.status === targetStatus) {
      draggedIssueId = null;
      return;
    }

    // Optimistic update
    issueList = issueList.map((i) =>
      i.id === draggedIssueId ? { ...i, status: targetStatus } : i
    );

    try {
      await issues.updateStatus(draggedIssueId, targetStatus);
    } catch (e) {
      console.error('Failed to update issue status:', e);
      await fetchIssues();
    }

    draggedIssueId = null;
  }

  async function openIssueDetail(issue: Issue) {
    selectedIssue = issue;
    if (issue.parent_id === null) {
      loadingChildren = true;
      try {
        const res = await fetch(`/api/projects/${projectId}/issues/${issue.id}/children`, {
          headers: authHeaders()
        });
        if (res.ok) {
          childIssues = await res.json();
        }
      } catch (e) {
        childIssues = [];
      } finally {
        loadingChildren = false;
      }
    } else {
      childIssues = [];
    }
  }

  function closeModal() {
    selectedIssue = null;
    childIssues = [];
  }

  function getParentTitle(parentId: string): string {
    const parent = issueList.find(i => i.id === parentId);
    return parent ? parent.title : parentId.slice(0, 8);
  }

  function childProgress(parentId: string): string {
    const children = issueList.filter(i => i.parent_id === parentId);
    if (children.length === 0) return '';
    const done = children.filter(c => c.status === 'closed').length;
    return `${done}/${children.length}`;
  }

  function typeBadgeColor(type: string): string {
    switch (type.toLowerCase()) {
      case 'bug': return 'bg-red-600 text-red-100';
      case 'feature': return 'bg-blue-600 text-blue-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function priorityDots(priority: number): string {
    const clamped = Math.max(0, Math.min(priority, 10));
    const max = 10;
    return '\u25CF'.repeat(clamped) + '\u25CB'.repeat(max - clamped);
  }
</script>

<div class="space-y-4">
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  <!-- Column visibility toggle -->
  <div class="flex items-center justify-between mb-3">
    <div class="flex items-center gap-2">
      <button
        onclick={() => showColumnSettings = !showColumnSettings}
        class="text-xs text-gray-500 hover:text-gray-300 transition-colors"
      >
        Columns ▾
      </button>
      {#if showColumnSettings}
        <div class="flex items-center gap-3 text-xs">
          {#each allColumns as col}
            <label class="flex items-center gap-1 text-gray-400 cursor-pointer">
              <input
                type="checkbox"
                checked={!hiddenColumns[col.key]}
                onchange={() => { hiddenColumns[col.key] = !hiddenColumns[col.key]; hiddenColumns = {...hiddenColumns}; }}
                class="accent-purple-500"
              />
              {col.label}
            </label>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <!-- Kanban board -->
  <div class="grid gap-4 min-h-[400px]" style="grid-template-columns: repeat({columns.length}, minmax(0, 1fr));">
    {#each columns as col}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="rounded-xl bg-gray-900 border border-gray-800 flex flex-col"
        ondragover={handleDragOver}
        ondrop={(e) => handleDrop(e, col.key)}
      >
        <!-- Column header -->
        <div class="px-4 py-3 border-b border-gray-800 flex items-center justify-between">
          <h3 class="text-sm font-semibold text-gray-300">{col.label}</h3>
          <span class="text-xs text-gray-500 bg-gray-800 px-2 py-0.5 rounded-full">
            {issuesByStatus(col.key).length}{#if issueList.filter(i => i.status === col.key && i.parent_id).length > 0}<span class="text-gray-600 ml-0.5">(+{issueList.filter(i => i.status === col.key && i.parent_id).length})</span>{/if}
          </span>
        </div>

        <!-- Cards -->
        <div class="flex-1 p-2 space-y-2 overflow-y-auto">
          {#if col.key === 'open' || col.key === 'backlog'}
            <button
              onclick={() => showCreateForm = !showCreateForm}
              class="w-full px-3 py-2 text-xs font-medium rounded-lg border border-dashed border-gray-700 text-gray-400 hover:border-purple-500 hover:text-purple-400 transition-colors"
            >
              {showCreateForm ? 'Cancel' : '+ New Issue'}
            </button>

            {#if showCreateForm}
              <div class="rounded-lg bg-gray-800 border border-gray-700 p-3 space-y-2">
                <input
                  type="text"
                  bind:value={newTitle}
                  placeholder="Issue title"
                  class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500"
                />
                <textarea
                  bind:value={newDescription}
                  placeholder="Description"
                  rows="2"
                  class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500 resize-none"
                ></textarea>
                <select
                  bind:value={newType}
                  class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500"
                >
                  <option value="task">Task</option>
                  <option value="bug">Bug</option>
                  <option value="feature">Feature</option>
                </select>
                <div>
                  <label for="issue-priority" class="block text-xs text-gray-400 mb-1">Priority: {newPriority}</label>
                  <input
                    id="issue-priority"
                    type="range"
                    min="1"
                    max="5"
                    bind:value={newPriority}
                    class="w-full accent-purple-500"
                  />
                </div>
                <div>
                  <label for="issue-role" class="block text-xs text-gray-400 mb-1">Role</label>
                  <select
                    id="issue-role"
                    bind:value={newRole}
                    class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500"
                  >
                    <option value="">No role</option>
                    {#each PREDEFINED_ROLES as role}
                      <option value={role}>{role}</option>
                    {/each}
                  </select>
                </div>
                <div>
                  <label class="block text-xs text-gray-400 mb-1">Scope Mode</label>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      onclick={() => newScopeMode = 'auto'}
                      class="flex-1 px-2 py-1.5 text-xs rounded border transition-colors {newScopeMode === 'auto' ? 'border-purple-500 bg-purple-600/20 text-purple-300' : 'border-gray-700 bg-gray-900 text-gray-400'}"
                    >
                      Auto
                    </button>
                    <button
                      type="button"
                      onclick={() => newScopeMode = 'conversational'}
                      class="flex-1 px-2 py-1.5 text-xs rounded border transition-colors {newScopeMode === 'conversational' ? 'border-purple-500 bg-purple-600/20 text-purple-300' : 'border-gray-700 bg-gray-900 text-gray-400'}"
                    >
                      Needs Scoping
                    </button>
                  </div>
                </div>
                <button
                  onclick={handleCreate}
                  disabled={creating || !newTitle.trim()}
                  class="w-full px-3 py-1.5 text-sm font-medium rounded bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
                >
                  {creating ? 'Creating...' : 'Create'}
                </button>
              </div>
            {/if}
          {/if}

          {#each issuesByStatus(col.key) as issue (issue.id)}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              draggable="true"
              ondragstart={(e) => handleDragStart(e, issue.id)}
              onclick={() => openIssueDetail(issue)}
              class="rounded-lg bg-gray-800 border border-gray-700 p-3 space-y-2 cursor-pointer cursor-grab active:cursor-grabbing hover:border-gray-600 transition-colors"
            >
              <div class="flex items-start justify-between gap-2">
                {#if hasChildren(issue.id)}
                  <button
                    onclick={(e) => toggleExpand(issue.id, e)}
                    class="text-gray-500 hover:text-gray-300 text-xs mt-0.5 shrink-0 transition-colors"
                    title={expandedParents.has(issue.id) ? 'Collapse' : 'Expand'}
                  >
                    {expandedParents.has(issue.id) ? '▼' : '▶'}
                  </button>
                {/if}
                <span class="text-sm font-medium text-gray-200 leading-tight flex-1">{issue.title}</span>
                <button
                  onclick={(e) => { e.stopPropagation(); handleDelete(issue.id); }}
                  class="text-gray-600 hover:text-red-400 text-xs shrink-0 transition-colors"
                  title="Delete issue"
                >
                  &times;
                </button>
              </div>
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-[10px] font-medium px-1.5 py-0.5 rounded {typeBadgeColor(issue.type)}">
                  {issue.type}
                </span>
                <span class="text-xs text-yellow-500 tracking-tight" title="Priority {issue.priority}">
                  {priorityDots(issue.priority)}
                </span>
                {#if issue.role}
                  <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-purple-600 text-purple-100">
                    {issue.role}
                  </span>
                {/if}
                {#if childProgress(issue.id)}
                  <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-emerald-800 text-emerald-200">
                    {childProgress(issue.id)} done
                  </span>
                {/if}
                {#if issue.needs_intake === 1}
                  <span class="text-[10px] font-medium px-1.5 py-0.5 rounded bg-amber-800 text-amber-200">
                    intake pending
                  </span>
                {/if}
              </div>
              {#if issue.claimed_by}
                <div class="text-[10px] text-gray-500">
                  <span class="text-gray-400">Agent:</span>
                  <span class="font-mono ml-1">{issue.claimed_by.slice(0, 8)}</span>
                </div>
              {/if}
            </div>

            <!-- Expanded children -->
            {#if hasChildren(issue.id) && expandedParents.has(issue.id)}
              <div class="ml-3 border-l-2 border-gray-700 pl-2 space-y-1">
                {#each issueList.filter(i => i.parent_id === issue.id) as child (child.id)}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    onclick={() => openIssueDetail(child)}
                    class="rounded-lg bg-gray-850 border border-gray-700/50 px-3 py-2 cursor-pointer hover:border-gray-600 transition-colors"
                  >
                    <div class="flex items-center gap-2">
                      <span class={child.status === 'closed' ? 'text-emerald-400 text-xs' : child.status === 'in_progress' ? 'text-blue-400 text-xs' : 'text-gray-500 text-xs'}>
                        {child.status === 'closed' ? '✓' : child.status === 'in_progress' ? '◉' : '○'}
                      </span>
                      <span class="text-xs text-gray-300 flex-1">{child.title}</span>
                      {#if child.role}
                        <span class="text-[10px] px-1.5 py-0.5 rounded bg-purple-600/50 text-purple-300">
                          {child.role}
                        </span>
                      {/if}
                      <span class="text-[10px] px-1.5 py-0.5 rounded {child.status === 'closed' ? 'bg-emerald-800/50 text-emerald-300' : child.status === 'in_progress' ? 'bg-blue-800/50 text-blue-300' : 'bg-gray-700 text-gray-400'}">
                        {child.status}
                      </span>
                    </div>
                    {#if child.claimed_by}
                      <div class="text-[10px] text-gray-500 mt-1 ml-5">
                        Agent: <span class="font-mono">{child.claimed_by.slice(0, 8)}</span>
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {/each}
  </div>

  {#if selectedIssue}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div
      class="fixed inset-0 bg-black/60 z-50 flex items-center justify-center p-4"
      onclick={closeModal}
    >
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div
        class="bg-gray-900 border border-gray-700 rounded-2xl max-w-2xl w-full max-h-[80vh] overflow-y-auto p-6 space-y-4"
        onclick={(e) => e.stopPropagation()}
      >
        <div class="flex items-start justify-between">
          <h2 class="text-lg font-semibold text-gray-100">{selectedIssue.title}</h2>
          <button onclick={closeModal} class="text-gray-500 hover:text-gray-300 text-xl">&times;</button>
        </div>

        <div class="flex gap-2 flex-wrap">
          <span class="text-xs font-medium px-2 py-1 rounded {typeBadgeColor(selectedIssue.type)}">
            {selectedIssue.type}
          </span>
          <span class="text-xs text-gray-400 px-2 py-1 rounded bg-gray-800">
            {selectedIssue.status}
          </span>
          {#if selectedIssue.role}
            <span class="text-xs font-medium px-2 py-1 rounded bg-purple-600 text-purple-100">
              {selectedIssue.role}
            </span>
          {/if}
          {#if selectedIssue.parent_id}
            <span class="text-xs text-gray-400 px-2 py-1 rounded bg-gray-800">
              ↳ {getParentTitle(selectedIssue.parent_id)}
            </span>
          {/if}
        </div>

        {#if selectedIssue.description}
          <div class="text-sm text-gray-300 bg-gray-800 rounded-lg p-3 whitespace-pre-wrap">
            {selectedIssue.description}
          </div>
        {/if}

        {#if selectedIssue.summary}
          <div>
            <h3 class="text-xs font-semibold text-gray-400 mb-1">Summary</h3>
            <div class="text-sm text-gray-300 bg-gray-800 rounded-lg p-3 whitespace-pre-wrap">
              {selectedIssue.summary}
            </div>
          </div>
        {/if}

        {#if childIssues.length > 0}
          <div>
            <h3 class="text-xs font-semibold text-gray-400 mb-2">
              Subtasks ({childIssues.filter(c => c.status === 'closed').length}/{childIssues.length} complete)
            </h3>
            <div class="space-y-1">
              {#each childIssues as child}
                <div class="flex items-center gap-2 text-sm px-3 py-2 rounded bg-gray-800">
                  <span class={child.status === 'closed' ? 'text-emerald-400' : 'text-gray-500'}>
                    {child.status === 'closed' ? '✓' : '○'}
                  </span>
                  <span class="text-gray-200 flex-1">{child.title}</span>
                  {#if child.role}
                    <span class="text-[10px] px-1.5 py-0.5 rounded bg-purple-600/50 text-purple-300">
                      {child.role}
                    </span>
                  {/if}
                  <span class="text-[10px] text-gray-500">{child.status}</span>
                </div>
              {/each}
            </div>
          </div>
        {:else if loadingChildren}
          <div class="text-sm text-gray-500">Loading subtasks...</div>
        {/if}

        {#if selectedIssue.claimed_by}
          <div class="text-xs text-gray-500">
            <span class="text-gray-400">Claimed by:</span>
            <span class="font-mono ml-1">{selectedIssue.claimed_by}</span>
          </div>
        {/if}

        {#if selectedIssue.depends_on && selectedIssue.depends_on !== '[]'}
          <div class="text-xs text-gray-500">
            <span class="text-gray-400">Depends on:</span>
            <span class="font-mono ml-1">{selectedIssue.depends_on}</span>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
