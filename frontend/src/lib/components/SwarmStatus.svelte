<script lang="ts">
  import { swarm, type SwarmStatus, type SwarmAgent } from '../api';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let status: SwarmStatus | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let pollTimer: ReturnType<typeof setInterval> | null = $state(null);

  function scalingBadge(rec: string): string {
    switch (rec) {
      case 'scale_up': return 'bg-blue-500/20 text-blue-300 border-blue-500/30';
      case 'scale_down': return 'bg-amber-500/20 text-amber-300 border-amber-500/30';
      default: return 'bg-gray-500/20 text-gray-400 border-gray-500/30';
    }
  }

  function stateDot(state: string): string {
    switch (state) {
      case 'running': return 'bg-blue-400';
      case 'working': return 'bg-blue-400';
      case 'idle': return 'bg-gray-500';
      case 'ready': return 'bg-emerald-400';
      default: return 'bg-gray-600';
    }
  }

  async function load() {
    try {
      status = await swarm.status(projectId);
      error = null;
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : 'Failed to load swarm status';
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (projectId) {
      load();
      pollTimer = setInterval(load, 10000);
    }
    return () => {
      if (pollTimer) clearInterval(pollTimer);
    };
  });
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h3 class="text-sm font-semibold text-gray-300 uppercase tracking-wider">Swarm Status</h3>
    <button onclick={load} class="text-xs text-gray-500 hover:text-gray-300 transition-colors">
      Refresh
    </button>
  </div>

  {#if loading}
    <div class="text-center text-gray-500 py-8 text-sm">Loading swarm status...</div>
  {:else if error}
    <div class="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-300">{error}</div>
  {:else if status}
    <!-- Summary cards -->
    <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
      <div class="bg-gray-900/50 border border-gray-800 rounded-lg p-3">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Mode</div>
        <div class="text-lg font-semibold text-purple-400 mt-0.5">{status.coordination_mode}</div>
      </div>
      <div class="bg-gray-900/50 border border-gray-800 rounded-lg p-3">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Active / Total</div>
        <div class="text-lg font-semibold text-gray-200 mt-0.5">
          <span class="text-blue-400">{status.active_agents}</span>
          <span class="text-gray-600">/</span>
          {status.total_agents}
        </div>
      </div>
      <div class="bg-gray-900/50 border border-gray-800 rounded-lg p-3">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Task Pool</div>
        <div class="text-lg font-semibold text-gray-200 mt-0.5">{status.task_pool_depth}</div>
      </div>
      <div class="bg-gray-900/50 border border-gray-800 rounded-lg p-3">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Throughput</div>
        <div class="text-lg font-semibold text-gray-200 mt-0.5">{status.throughput_issues_per_hour.toFixed(1)}<span class="text-xs text-gray-500">/hr</span></div>
      </div>
    </div>

    <!-- Scaling recommendation -->
    <div class="flex items-center gap-2">
      <span class="text-xs text-gray-500">Scaling:</span>
      <span class="text-[10px] px-2 py-0.5 rounded-full border {scalingBadge(status.scaling_recommendation)}">
        {status.scaling_recommendation.replace('_', ' ')}
      </span>
    </div>

    <!-- Agent list -->
    {#if status.agents.length > 0}
      {@const activeAgents = status.agents.filter(a => a.state === 'running' || a.state === 'working')}
      {@const assignedAgents = status.agents.filter(a => a.issue_id)}

      <div class="border-t border-gray-800 pt-3">
        <div class="flex items-center justify-between mb-2">
          <h4 class="text-xs font-semibold text-gray-400">
            Agents
            <span class="text-gray-600">({status.agents.length})</span>
          </h4>
          <span class="text-[10px] text-gray-500">
            {activeAgents.length} active, {assignedAgents.length} assigned
          </span>
        </div>

        <div class="space-y-1 max-h-[400px] overflow-y-auto">
          {#each status.agents.filter(a => a.issue_id) as agent (agent.session_id)}
            <div class="flex items-center gap-2 rounded-lg bg-gray-900/30 border border-gray-800/50 px-3 py-2">
              <div class="w-2 h-2 rounded-full {stateDot(agent.state)} flex-shrink-0"></div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-xs text-gray-300 font-medium">{agent.role}</span>
                  <span class="text-[10px] text-gray-600 font-mono">{agent.session_id.slice(0, 8)}</span>
                  <span class="text-[10px] text-gray-600">{agent.runtime}</span>
                </div>
                {#if agent.issue_title}
                  <div class="text-[10px] text-gray-500 truncate mt-0.5">{agent.issue_title}</div>
                {/if}
              </div>
              <span class="text-[10px] text-gray-500">{agent.state}</span>
            </div>
          {/each}

          {#if status.agents.filter(a => !a.issue_id).length > 0}
            <div class="text-[10px] text-gray-600 pt-1 px-1">
              + {status.agents.filter(a => !a.issue_id).length} unassigned agents
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <div class="text-center text-gray-500 py-4 text-sm">No agents registered.</div>
    {/if}
  {/if}
</div>
