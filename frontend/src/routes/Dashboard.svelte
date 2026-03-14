<script lang="ts">
  import {
    dashboard,
    agents,
    loom as loomApi,
    type AgentInfo,
    type DashboardStats,
    type ActivityLogEntry,
    type MetricsResponse,
    type SystemHealth,
    type LoomEntry,
  } from '../lib/api';
  import ActivityFeed from '../lib/components/ActivityFeed.svelte';
  import LoomFeed from '../lib/components/LoomFeed.svelte';
  import MetricsChart from '../lib/components/MetricsChart.svelte';
  import SystemHealthPanel from '../lib/components/SystemHealth.svelte';

  let stats: DashboardStats | null = $state(null);
  let agentSessions: AgentInfo[] = $state([]);
  let activityEntries: ActivityLogEntry[] = $state([]);
  let metricsData: MetricsResponse | null = $state(null);
  let healthData: SystemHealth | null = $state(null);
  let loomEntries: LoomEntry[] = $state([]);
  let error: string | null = $state(null);
  let metricsDays: number = $state(7);

  async function fetchAll() {
    try {
      const [dashStats, agentIds, activity, metrics, system, loomData] = await Promise.all([
        dashboard.stats(),
        agents.list(),
        dashboard.activity(50, 0),
        dashboard.metrics(metricsDays),
        dashboard.system(),
        loomApi.recent(50),
      ]);
      stats = dashStats;
      activityEntries = activity;
      metricsData = metrics;
      healthData = system;
      loomEntries = loomData;

      agentSessions = agentIds;
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch dashboard data';
    }
  }

  $effect(() => {
    fetchAll();
    const interval = setInterval(fetchAll, 5000);
    return () => clearInterval(interval);
  });

  function truncateId(id: string): string {
    return id.length > 8 ? id.slice(0, 8) : id;
  }

  function runtimeColor(runtime: string): string {
    switch (runtime.toLowerCase()) {
      case 'claude': return 'bg-purple-600 text-purple-100';
      case 'opencode': return 'bg-blue-600 text-blue-100';
      case 'gemini': return 'bg-green-600 text-green-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function stateColor(state: string): string {
    switch (state.toLowerCase()) {
      case 'idle': return 'bg-gray-400';
      case 'working': return 'bg-green-400 animate-pulse';
      case 'blocked': return 'bg-yellow-400';
      case 'crashed': return 'bg-red-500';
      default: return 'bg-gray-400';
    }
  }

  function timeAgo(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const secs = Math.floor(diff / 1000);
    if (secs < 60) return `${secs}s ago`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    return `${hrs}h ago`;
  }

  const statCards = $derived([
    { label: 'Active Agents', value: stats?.active_agents ?? 0, accent: 'text-purple-400' },
    { label: 'Projects', value: stats?.project_count ?? 0, accent: 'text-blue-400' },
    { label: 'Open Issues', value: stats?.open_issues ?? 0, accent: 'text-yellow-400' },
    { label: 'Running Workflows', value: stats?.running_workflows ?? 0, accent: 'text-green-400' },
  ]);

  function setDays(d: number) {
    metricsDays = d;
    // Re-fetch metrics with new range
    dashboard.metrics(d).then(m => { metricsData = m; }).catch(() => {});
  }
</script>

<div class="space-y-8">
  <!-- Header -->
  <div>
    <h1 class="text-2xl font-bold text-white">Dashboard</h1>
    <p class="mt-1 text-sm text-gray-400">Overview of projects, agents, and workflows.</p>
  </div>

  <!-- Error banner -->
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  <!-- Summary stats -->
  <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
    {#each statCards as card}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5">
        <p class="text-sm font-medium text-gray-400">{card.label}</p>
        <p class="mt-2 text-3xl font-bold {card.accent}">{card.value}</p>
      </div>
    {/each}
  </div>

  <!-- Activity Feed + Metrics Charts -->
  <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
    <!-- Activity Feed -->
    <ActivityFeed entries={activityEntries} />

    <!-- Metrics Charts + Merge Stats -->
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-4">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-semibold text-gray-300">Metrics</h3>
        <div class="flex gap-1">
          <button
            class="text-xs px-2 py-1 rounded {metricsDays === 7 ? 'bg-purple-600 text-white' : 'bg-gray-800 text-gray-400 hover:text-gray-200'}"
            onclick={() => setDays(7)}
          >7d</button>
          <button
            class="text-xs px-2 py-1 rounded {metricsDays === 30 ? 'bg-purple-600 text-white' : 'bg-gray-800 text-gray-400 hover:text-gray-200'}"
            onclick={() => setDays(30)}
          >30d</button>
        </div>
      </div>

      {#if metricsData}
        <MetricsChart daily={metricsData.daily} days={metricsDays} />

        <!-- Merge Stats -->
        <div class="pt-3 border-t border-gray-800 space-y-2">
          <p class="text-sm text-gray-300">
            Merges:
            <span class="text-green-400 font-mono">{metricsData.merge_stats.clean}</span> clean,
            <span class="text-yellow-400 font-mono">{metricsData.merge_stats.conflicted}</span> conflicts,
            <span class="text-red-400 font-mono">{metricsData.merge_stats.escalated}</span> escalated
          </p>
          <p class="text-sm text-gray-400">
            Avg resolution: <span class="text-gray-200 font-mono">{metricsData.avg_resolution_hours.toFixed(1)}</span> hours
          </p>
        </div>
      {:else}
        <div class="h-48 flex items-center justify-center text-gray-500 text-sm">
          Loading metrics...
        </div>
      {/if}
    </div>
  </div>

  <!-- Loom Feed -->
  <LoomFeed entries={loomEntries} />

  <!-- System Health -->
  {#if healthData}
    <SystemHealthPanel health={healthData} />
  {/if}

  <!-- Active agents -->
  <div>
    <h2 class="text-lg font-semibold text-white mb-4">Active Agents</h2>

    {#if agentSessions.length === 0}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
        No active agent sessions.
      </div>
    {:else}
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
        {#each agentSessions as agent (agent.id)}
          <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-3">
            <!-- Top row: ID + runtime badge -->
            <div class="flex items-center justify-between">
              <span class="font-mono text-sm text-gray-200" title={agent.id}>
                {truncateId(agent.id)}
              </span>
              <span class="text-xs font-medium px-2 py-0.5 rounded-full {runtimeColor(agent.runtime)}">
                {agent.runtime}
              </span>
            </div>

            <!-- Role -->
            {#if agent.role}
              <p class="text-xs text-gray-400">{agent.role}</p>
            {/if}

            <!-- Claimed issue -->
            {#if agent.claimed_issue}
              <p class="text-xs text-purple-300 truncate" title={agent.claimed_issue}>{agent.claimed_issue}</p>
            {/if}

            <!-- State + last activity -->
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <span class="inline-block h-2.5 w-2.5 rounded-full {stateColor(agent.state)}"></span>
                <span class="text-sm text-gray-300 capitalize">{agent.state}</span>
              </div>
              {#if agent.last_heartbeat}
                <span class="text-xs text-gray-500">{timeAgo(agent.last_heartbeat)}</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
