<script lang="ts">
  import { costTracking, performanceLog, type CostSummary, type DailySpend, type ModelStats } from '../api';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let summary: CostSummary | null = $state(null);
  let daily: DailySpend[] = $state([]);
  let stats: ModelStats[] = $state([]);
  let days: number = $state(7);
  let error: string | null = $state(null);
  let aggregating: boolean = $state(false);

  async function load() {
    try {
      [summary, daily, stats] = await Promise.all([
        costTracking.summary(projectId, days),
        costTracking.daily(projectId, days),
        performanceLog.stats(projectId, days),
      ]);
      error = null;
    } catch (e) {
      error = 'Failed to load cost data';
    }
  }

  async function aggregate() {
    aggregating = true;
    try {
      await costTracking.aggregate(projectId);
      await load();
    } catch (e) {
      error = 'Aggregation failed';
    }
    aggregating = false;
  }

  function formatCost(usd: number): string {
    return `$${usd.toFixed(4)}`;
  }

  function formatTokens(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
    return String(n);
  }

  function maxDailyCost(): number {
    if (daily.length === 0) return 1;
    return Math.max(...daily.map(d => d.cost_usd), 0.0001);
  }

  function successRate(): string {
    if (!summary || summary.task_count === 0) return '—';
    const rate = ((summary.task_count - summary.failure_count) / summary.task_count) * 100;
    return `${rate.toFixed(0)}%`;
  }

  $effect(() => { days; load(); });
</script>

<div class="space-y-4">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-white">Cost & Performance</h2>
    <div class="flex items-center gap-3">
      <select
        bind:value={days}
        class="text-xs rounded-lg bg-gray-800 border border-gray-700 text-gray-300 px-2 py-1"
      >
        <option value={7}>7 days</option>
        <option value={14}>14 days</option>
        <option value={30}>30 days</option>
      </select>
      <button
        onclick={aggregate}
        disabled={aggregating}
        class="text-xs text-gray-500 hover:text-gray-300 transition-colors disabled:opacity-50"
      >
        {aggregating ? 'Aggregating...' : 'Refresh'}
      </button>
    </div>
  </div>

  {#if error}
    <div class="text-xs text-red-400">{error}</div>
  {/if}

  {#if summary}
    <!-- Summary cards -->
    <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-4">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Total Spend</div>
        <div class="text-xl font-semibold text-white mt-1">{formatCost(summary.total_cost_usd)}</div>
      </div>
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-4">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Tokens</div>
        <div class="text-xl font-semibold text-white mt-1">{formatTokens(summary.total_tokens)}</div>
      </div>
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-4">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Tasks</div>
        <div class="text-xl font-semibold text-white mt-1">{summary.task_count}</div>
      </div>
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-4">
        <div class="text-[10px] text-gray-500 uppercase tracking-wider">Success Rate</div>
        <div class="text-xl font-semibold {summary.failure_count > 0 ? 'text-yellow-400' : 'text-green-400'} mt-1">
          {successRate()}
        </div>
      </div>
    </div>

    <!-- Daily spend chart (bar chart) -->
    {#if daily.length > 0}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5">
        <h3 class="text-sm font-medium text-gray-400 mb-3">Daily Spend</h3>
        <div class="flex items-end gap-1 h-32">
          {#each daily as d}
            {@const height = (d.cost_usd / maxDailyCost()) * 100}
            <div class="flex-1 flex flex-col items-center gap-1 group relative">
              <div
                class="w-full rounded-t bg-purple-600/60 hover:bg-purple-500/80 transition-colors min-h-[2px]"
                style="height: {Math.max(height, 2)}%"
              ></div>
              <span class="text-[8px] text-gray-600">{d.date.slice(5)}</span>
              <!-- Tooltip -->
              <div class="absolute bottom-full mb-1 hidden group-hover:block bg-gray-800 border border-gray-700 rounded px-2 py-1 text-[10px] text-gray-300 whitespace-nowrap z-10">
                {formatCost(d.cost_usd)} · {formatTokens(d.tokens)} tokens
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- Breakdowns -->
    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <!-- By Role -->
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5">
        <h3 class="text-sm font-medium text-gray-400 mb-3">Cost by Role</h3>
        <div class="space-y-2">
          {#each Object.entries(summary.by_role).sort(([,a], [,b]) => (b as number) - (a as number)) as [role, cost]}
            {@const pct = summary!.total_cost_usd > 0 ? ((cost as number) / summary!.total_cost_usd) * 100 : 0}
            <div class="space-y-1">
              <div class="flex justify-between text-xs">
                <span class="text-gray-300">{role}</span>
                <span class="text-gray-500">{formatCost(cost as number)}</span>
              </div>
              <div class="h-1.5 rounded-full bg-gray-800">
                <div class="h-full rounded-full bg-cyan-500/60" style="width: {pct}%"></div>
              </div>
            </div>
          {/each}
          {#if Object.keys(summary.by_role).length === 0}
            <div class="text-xs text-gray-600">No data</div>
          {/if}
        </div>
      </div>

      <!-- By Model -->
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5">
        <h3 class="text-sm font-medium text-gray-400 mb-3">Cost by Model</h3>
        <div class="space-y-2">
          {#each Object.entries(summary.by_model).sort(([,a], [,b]) => (b as number) - (a as number)) as [model, cost]}
            {@const pct = summary!.total_cost_usd > 0 ? ((cost as number) / summary!.total_cost_usd) * 100 : 0}
            <div class="space-y-1">
              <div class="flex justify-between text-xs">
                <span class="text-gray-300 font-mono">{model}</span>
                <span class="text-gray-500">{formatCost(cost as number)}</span>
              </div>
              <div class="h-1.5 rounded-full bg-gray-800">
                <div class="h-full rounded-full bg-purple-500/60" style="width: {pct}%"></div>
              </div>
            </div>
          {/each}
          {#if Object.keys(summary.by_model).length === 0}
            <div class="text-xs text-gray-600">No data</div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <!-- Model stats table -->
  {#if stats.length > 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5">
      <h3 class="text-sm font-medium text-gray-400 mb-3">Model Performance</h3>
      <div class="overflow-x-auto">
        <table class="w-full text-xs">
          <thead>
            <tr class="text-gray-500 border-b border-gray-800">
              <th class="text-left py-2 px-2">Model</th>
              <th class="text-left py-2 px-2">CLI</th>
              <th class="text-left py-2 px-2">Role</th>
              <th class="text-right py-2 px-2">Tasks</th>
              <th class="text-right py-2 px-2">Success</th>
              <th class="text-right py-2 px-2">Avg Cost</th>
              <th class="text-right py-2 px-2">Avg Time</th>
            </tr>
          </thead>
          <tbody>
            {#each stats as s}
              <tr class="border-b border-gray-800/50 hover:bg-gray-800/30">
                <td class="py-2 px-2 text-gray-300 font-mono">{s.model}</td>
                <td class="py-2 px-2 text-gray-400">{s.runtime}</td>
                <td class="py-2 px-2 text-gray-400">{s.role}</td>
                <td class="py-2 px-2 text-right text-gray-300">{s.total}</td>
                <td class="py-2 px-2 text-right {s.success_rate >= 0.8 ? 'text-green-400' : s.success_rate >= 0.5 ? 'text-yellow-400' : 'text-red-400'}">
                  {(s.success_rate * 100).toFixed(0)}%
                </td>
                <td class="py-2 px-2 text-right text-gray-400">{formatCost(s.avg_cost)}</td>
                <td class="py-2 px-2 text-right text-gray-400">{s.avg_duration.toFixed(0)}s</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}

  {#if !summary && !error}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      Loading cost data...
    </div>
  {/if}
</div>
