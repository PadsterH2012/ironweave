<script lang="ts">
  import { workflows, type WorkflowDefinition, type WorkflowInstance } from '../api';
  import DagGraph from './DagGraph.svelte';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let definitions: WorkflowDefinition[] = $state([]);
  let selectedDef: WorkflowDefinition | null = $state(null);
  let instances: WorkflowInstance[] = $state([]);
  let selectedInstance: WorkflowInstance | null = $state(null);
  let loading = $state(false);
  let error: string | null = $state(null);

  interface DagStage {
    id: string;
    name: string;
    runtime: string;
    prompt: string;
    depends_on: string[];
    is_manual_gate: boolean;
  }

  function parseDag(dagJson: string): DagStage[] {
    try {
      const dag = JSON.parse(dagJson);
      return (dag.stages || []).map((s: any) => ({
        id: s.id || s.name,
        name: s.name,
        runtime: s.runtime || 'claude',
        prompt: s.prompt || '',
        depends_on: s.depends_on || [],
        is_manual_gate: s.is_manual_gate || false,
      }));
    } catch {
      return [];
    }
  }

  function parseCheckpoint(checkpoint: string): Record<string, string> {
    try {
      const cp = JSON.parse(checkpoint);
      if (cp.stage_statuses) return cp.stage_statuses;
      if (cp.stages) {
        const statuses: Record<string, string> = {};
        for (const [k, v] of Object.entries(cp.stages)) {
          statuses[k] = typeof v === 'string' ? v : JSON.stringify(v);
        }
        return statuses;
      }
      return {};
    } catch {
      return {};
    }
  }

  function stateColor(state: string): string {
    switch (state) {
      case 'running': return 'text-blue-400';
      case 'completed': return 'text-emerald-400';
      case 'failed': return 'text-red-400';
      case 'paused': return 'text-amber-400';
      default: return 'text-gray-400';
    }
  }

  function stateBadgeBg(state: string): string {
    switch (state) {
      case 'running': return 'bg-blue-500/20 text-blue-300 border-blue-500/30';
      case 'completed': return 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30';
      case 'failed': return 'bg-red-500/20 text-red-300 border-red-500/30';
      case 'paused': return 'bg-amber-500/20 text-amber-300 border-amber-500/30';
      default: return 'bg-gray-500/20 text-gray-300 border-gray-500/30';
    }
  }

  async function loadDefinitions() {
    loading = true;
    error = null;
    try {
      definitions = await workflows.definitions.list(projectId);
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to load workflows';
    } finally {
      loading = false;
    }
  }

  async function selectDefinition(def: WorkflowDefinition) {
    selectedDef = def;
    selectedInstance = null;
    try {
      instances = await workflows.instances.list(def.id);
    } catch (e: unknown) {
      console.warn('Failed to load instances:', e);
      instances = [];
    }
  }

  async function createInstance() {
    if (!selectedDef) return;
    try {
      const inst = await workflows.instances.create(selectedDef.id, { definition_id: selectedDef.id });
      instances = [inst, ...instances];
      selectedInstance = inst;
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to create instance';
    }
  }

  async function approveGate(stageId: string) {
    if (!selectedDef || !selectedInstance) return;
    try {
      await workflows.instances.approveGate(selectedDef.id, selectedInstance.id, stageId);
      await refreshInstance();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to approve gate';
    }
  }

  async function refreshInstance() {
    if (!selectedDef || !selectedInstance) return;
    const updated = await workflows.instances.list(selectedDef.id);
    instances = updated;
    selectedInstance = updated.find(i => i.id === selectedInstance!.id) || selectedInstance;
  }

  async function pauseInstance() {
    if (!selectedDef || !selectedInstance) return;
    try {
      await workflows.instances.pause(selectedDef.id, selectedInstance.id);
      await refreshInstance();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to pause instance';
    }
  }

  async function resumeInstance() {
    if (!selectedDef || !selectedInstance) return;
    try {
      await workflows.instances.resume(selectedDef.id, selectedInstance.id);
      await refreshInstance();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to resume instance';
    }
  }

  async function cancelInstance() {
    if (!selectedDef || !selectedInstance) return;
    try {
      await workflows.instances.cancel(selectedDef.id, selectedInstance.id);
      await refreshInstance();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to cancel instance';
    }
  }

  function handleStageClick(stage: DagStage, status: string | null) {
    if (stage.is_manual_gate && status?.toLowerCase() === 'waitingapproval') {
      approveGate(stage.id);
    }
  }

  $effect(() => {
    if (projectId) loadDefinitions();
  });
</script>

<div class="space-y-4">
  <!-- Workflow definitions list -->
  <div class="flex items-center justify-between">
    <h3 class="text-sm font-semibold text-gray-300 uppercase tracking-wider">Workflows</h3>
    <button onclick={loadDefinitions} class="text-xs text-gray-500 hover:text-gray-300 transition-colors">
      Refresh
    </button>
  </div>

  {#if loading}
    <div class="text-center text-gray-500 py-8 text-sm">Loading workflows...</div>
  {:else if error}
    <div class="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-300">{error}</div>
  {:else if definitions.length === 0}
    <div class="text-center text-gray-500 py-8 text-sm">No workflow definitions found for this project.</div>
  {:else}
    <div class="grid grid-cols-1 gap-2">
      {#each definitions as def (def.id)}
        <button
          onclick={() => selectDefinition(def)}
          class="text-left rounded-lg border p-3 transition-all {selectedDef?.id === def.id
            ? 'bg-purple-500/10 border-purple-500/40 shadow-lg shadow-purple-500/5'
            : 'bg-gray-900/50 border-gray-800 hover:border-gray-700'}"
        >
          <div class="flex items-center justify-between">
            <span class="text-sm font-medium text-gray-200">{def.name}</span>
            <span class="text-[10px] text-gray-500 font-mono">v{def.version}</span>
          </div>
          {#if def.git_sha}
            <div class="text-[10px] text-gray-600 font-mono mt-0.5">{def.git_sha.slice(0, 8)}</div>
          {/if}
        </button>
      {/each}
    </div>
  {/if}

  <!-- Selected definition: instances & DAG -->
  {#if selectedDef}
    <div class="border-t border-gray-800 pt-4 space-y-3">
      <div class="flex items-center justify-between">
        <h4 class="text-sm font-semibold text-gray-300">
          Instances of <span class="text-purple-400">{selectedDef.name}</span>
        </h4>
        <div class="flex gap-2">
          {#if selectedInstance?.state === 'running'}
            <button
              onclick={pauseInstance}
              class="px-3 py-1 text-xs font-medium rounded-lg bg-amber-600 hover:bg-amber-500 text-white transition-colors"
            >Pause</button>
          {/if}
          {#if selectedInstance?.state === 'paused'}
            <button
              onclick={resumeInstance}
              class="px-3 py-1 text-xs font-medium rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white transition-colors"
            >Resume</button>
          {/if}
          {#if selectedInstance && selectedInstance.state !== 'completed' && selectedInstance.state !== 'cancelled'}
            <button
              onclick={cancelInstance}
              class="px-3 py-1 text-xs font-medium rounded-lg bg-red-600 hover:bg-red-500 text-white transition-colors"
            >Cancel</button>
          {/if}
          <button
            onclick={createInstance}
            class="px-3 py-1 text-xs font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
          >
            + Start New
          </button>
        </div>
      </div>

      {#if instances.length === 0}
        <div class="text-center text-gray-500 py-4 text-sm">No instances yet. Click "Start New" to run this workflow.</div>
      {:else}
        <div class="space-y-1">
          {#each instances as inst (inst.id)}
            <button
              onclick={() => selectedInstance = inst}
              class="w-full text-left rounded-lg border p-2.5 transition-all {selectedInstance?.id === inst.id
                ? 'bg-gray-800/80 border-gray-600'
                : 'bg-gray-900/30 border-gray-800/50 hover:border-gray-700'}"
            >
              <div class="flex items-center justify-between">
                <span class="text-xs font-mono text-gray-400">{inst.id.slice(0, 8)}</span>
                <span class="text-[10px] px-2 py-0.5 rounded-full border {stateBadgeBg(inst.state)}">{inst.state}</span>
              </div>
              <div class="flex items-center gap-3 mt-1 text-[10px] text-gray-500">
                {#if inst.started_at}
                  <span>Started: {new Date(inst.started_at).toLocaleString()}</span>
                {/if}
                {#if inst.total_tokens > 0}
                  <span>{inst.total_tokens.toLocaleString()} tokens</span>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <!-- DAG visualization for selected instance -->
    {#if selectedInstance}
      {@const stages = parseDag(selectedDef.dag)}
      {@const stageStatuses = parseCheckpoint(selectedInstance.checkpoint)}
      <div class="border-t border-gray-800 pt-4">
        <div class="flex items-center justify-between mb-2">
          <h4 class="text-sm font-semibold text-gray-300">DAG — {selectedInstance.id.slice(0, 8)}</h4>
          {#if selectedInstance.current_stage}
            <span class="text-[10px] text-blue-400">Current: {selectedInstance.current_stage}</span>
          {/if}
        </div>
        {#if stages.length > 0}
          <div class="h-[400px]">
            <DagGraph {stages} {stageStatuses} onStageClick={handleStageClick} />
          </div>
          <p class="text-[10px] text-gray-600 mt-1">Click a waiting gate node to approve it.</p>
        {:else}
          <div class="text-center text-gray-500 py-4 text-sm">No stages defined in DAG.</div>
        {/if}
      </div>
    {/if}
  {/if}
</div>
