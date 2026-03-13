<script lang="ts">
  import { push } from 'svelte-spa-router';
  import {
    workflows,
    type WorkflowDefinition,
    type WorkflowInstance,
  } from '../lib/api';
  import DagGraph from '../lib/components/DagGraph.svelte';

  interface Stage {
    id: string;
    name: string;
    runtime: string;
    prompt: string;
    depends_on: string[];
    is_manual_gate: boolean;
  }

  interface DagDefinition {
    stages: Stage[];
  }

  interface DagExecutionState {
    stage_statuses: Record<string, string>;
    execution_order: string[][];
  }

  interface Props {
    params: { id: string; wid: string };
  }

  let { params }: Props = $props();

  let definition: WorkflowDefinition | null = $state(null);
  let instances: WorkflowInstance[] = $state([]);
  let activeInstance: WorkflowInstance | null = $state(null);
  let stages: Stage[] = $state([]);
  let stageStatuses: Record<string, string> | undefined = $state(undefined);
  let error: string | null = $state(null);
  let selectedStage: Stage | null = $state(null);
  let selectedStatus: string | null = $state(null);
  let refreshInterval: ReturnType<typeof setInterval> | undefined = $state(undefined);

  function parseDag(dagJson: string): Stage[] {
    try {
      const dag: DagDefinition = JSON.parse(dagJson);
      return dag.stages || [];
    } catch {
      return [];
    }
  }

  function parseCheckpoint(checkpoint: string): DagExecutionState | null {
    if (!checkpoint) return null;
    try {
      return JSON.parse(checkpoint);
    } catch {
      return null;
    }
  }

  function isRunning(instance: WorkflowInstance | null): boolean {
    if (!instance) return false;
    const state = instance.state.toLowerCase();
    return state === 'running' || state === 'pending' || state === 'in_progress';
  }

  async function fetchDefinition() {
    try {
      const defs = await workflows.definitions.list(params.id);
      definition = defs.find((d) => d.id === params.wid) || null;
      if (definition) {
        stages = parseDag(definition.dag);
      }
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch workflow definition';
    }
  }

  async function fetchInstances() {
    if (!definition) return;
    try {
      instances = await workflows.instances.list(definition.id);
      // Find active/latest instance
      const running = instances.find((i) => isRunning(i));
      activeInstance = running || instances[0] || null;
      if (activeInstance && activeInstance.checkpoint) {
        const state = parseCheckpoint(activeInstance.checkpoint);
        stageStatuses = state?.stage_statuses;
      } else {
        stageStatuses = undefined;
      }
    } catch {
      // No instances yet - that's fine
      instances = [];
      activeInstance = null;
      stageStatuses = undefined;
    }
  }

  // Initial fetch
  $effect(() => {
    const _wid = params.wid;
    const _id = params.id;
    if (_wid && _id) {
      fetchDefinition().then(() => fetchInstances());
    }
  });

  // Auto-refresh when running
  $effect(() => {
    if (refreshInterval) {
      clearInterval(refreshInterval);
      refreshInterval = undefined;
    }

    if (isRunning(activeInstance)) {
      refreshInterval = setInterval(() => {
        fetchInstances();
      }, 5000);
    }

    return () => {
      if (refreshInterval) {
        clearInterval(refreshInterval);
        refreshInterval = undefined;
      }
    };
  });

  function handleStageClick(stage: Stage, status: string | null) {
    selectedStage = stage;
    selectedStatus = status;
  }

  function stateBadgeClass(state: string): string {
    switch (state.toLowerCase()) {
      case 'running':
      case 'in_progress':
        return 'bg-blue-600 text-blue-100';
      case 'completed':
      case 'succeeded':
        return 'bg-green-600 text-green-100';
      case 'failed':
        return 'bg-red-600 text-red-100';
      case 'pending':
        return 'bg-gray-600 text-gray-100';
      case 'waiting_approval':
        return 'bg-yellow-600 text-yellow-100';
      default:
        return 'bg-gray-600 text-gray-100';
    }
  }

  function statusBadgeClass(status: string): string {
    if (status.startsWith('Failed')) return 'bg-red-900/50 text-red-300 border border-red-700';
    switch (status) {
      case 'Running':
        return 'bg-blue-900/50 text-blue-300 border border-blue-700';
      case 'Completed':
        return 'bg-green-900/50 text-green-300 border border-green-700';
      case 'Pending':
        return 'bg-gray-800 text-gray-400 border border-gray-700';
      case 'WaitingApproval':
        return 'bg-yellow-900/50 text-yellow-300 border border-yellow-700';
      case 'Skipped':
        return 'bg-gray-800 text-gray-500 border border-gray-700';
      default:
        return 'bg-gray-800 text-gray-400 border border-gray-700';
    }
  }
</script>

<div class="space-y-6">
  <!-- Back link -->
  <button
    onclick={() => push(`/projects/${params.id}`)}
    class="text-sm text-gray-400 hover:text-white transition-colors"
  >
    &larr; Back to Project
  </button>

  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  {#if definition}
    <!-- Header -->
    <div class="flex items-center gap-4">
      <div>
        <h1 class="text-2xl font-bold text-white">{definition.name}</h1>
        <p class="mt-1 text-sm text-gray-500">
          Version {definition.version}
          {#if definition.git_sha}
            <span class="font-mono ml-2">({definition.git_sha.slice(0, 7)})</span>
          {/if}
        </p>
      </div>
      {#if activeInstance}
        <span class="text-xs font-medium px-2.5 py-1 rounded-full {stateBadgeClass(activeInstance.state)}">
          {activeInstance.state}
        </span>
        {#if isRunning(activeInstance)}
          <span class="text-xs text-gray-500 animate-pulse">Auto-refreshing</span>
        {/if}
      {:else}
        <span class="text-xs font-medium px-2.5 py-1 rounded-full bg-gray-700 text-gray-300">
          Definition only
        </span>
      {/if}
    </div>

    <!-- DAG Graph -->
    {#if stages.length > 0}
      <div class="h-[500px]">
        <DagGraph {stages} {stageStatuses} onStageClick={handleStageClick} />
      </div>
    {:else}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
        No stages defined in this workflow.
      </div>
    {/if}

    <!-- Selected stage detail panel -->
    {#if selectedStage}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
        <div class="flex items-center justify-between">
          <h2 class="text-lg font-semibold text-white">{selectedStage.name}</h2>
          <div class="flex items-center gap-2">
            {#if selectedStage.is_manual_gate}
              <span class="text-xs font-medium px-2.5 py-1 rounded-full bg-yellow-900/50 text-yellow-300 border border-yellow-700">
                Manual Gate
              </span>
            {/if}
            {#if selectedStatus}
              <span class="text-xs font-medium px-2.5 py-1 rounded-full {statusBadgeClass(selectedStatus)}">
                {selectedStatus}
              </span>
            {/if}
          </div>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-1">Runtime</h3>
            <p class="text-sm text-gray-300 font-mono">{selectedStage.runtime}</p>
          </div>
          <div>
            <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-1">Stage ID</h3>
            <p class="text-sm text-gray-300 font-mono">{selectedStage.id}</p>
          </div>
        </div>

        {#if selectedStage.depends_on.length > 0}
          <div>
            <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-1">Dependencies</h3>
            <div class="flex flex-wrap gap-2">
              {#each selectedStage.depends_on as dep}
                <span class="text-xs font-mono px-2 py-1 rounded bg-gray-800 text-gray-400 border border-gray-700">
                  {dep}
                </span>
              {/each}
            </div>
          </div>
        {/if}

        <div>
          <h3 class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-1">Prompt</h3>
          <pre class="text-sm text-gray-300 bg-gray-950 rounded-lg p-3 border border-gray-800 whitespace-pre-wrap font-mono max-h-48 overflow-y-auto">{selectedStage.prompt}</pre>
        </div>
      </div>
    {/if}

    <!-- Instance list -->
    {#if instances.length > 1}
      <div class="space-y-3">
        <h2 class="text-sm font-semibold text-gray-400 uppercase tracking-wider">Other Instances</h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
          {#each instances as inst (inst.id)}
            <button
              onclick={() => { activeInstance = inst; const state = parseCheckpoint(inst.checkpoint); stageStatuses = state?.stage_statuses; selectedStage = null; selectedStatus = null; }}
              class="rounded-lg bg-gray-900 border p-3 text-left transition-colors {inst.id === activeInstance?.id ? 'border-purple-600' : 'border-gray-800 hover:border-gray-600'}"
            >
              <div class="flex items-center justify-between">
                <span class="text-xs font-mono text-gray-400">{inst.id.slice(0, 8)}</span>
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {stateBadgeClass(inst.state)}">
                  {inst.state}
                </span>
              </div>
              {#if inst.started_at}
                <p class="text-xs text-gray-500 mt-1">
                  Started: {new Date(inst.started_at).toLocaleString()}
                </p>
              {/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {:else if !error}
    <div class="text-gray-500">Loading workflow...</div>
  {/if}
</div>
