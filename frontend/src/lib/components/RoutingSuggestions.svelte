<script lang="ts">
  import { routingOverrides, type RoutingOverride } from '../api';
  import { timeAgo } from '../utils';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let overrides: RoutingOverride[] = $state([]);
  let error: string | null = $state(null);
  let detecting: boolean = $state(false);
  let actingOn: string | null = $state(null);
  let filter: 'all' | 'suggested' | 'accepted' | 'rejected' = $state('all');

  async function load() {
    try {
      overrides = await routingOverrides.list(projectId);
      error = null;
    } catch (e) {
      error = 'Failed to load routing overrides';
    }
  }

  async function detect() {
    detecting = true;
    try {
      await routingOverrides.detect(projectId);
      await load();
    } catch (e) {
      error = 'Pattern detection failed';
    }
    detecting = false;
  }

  async function accept(id: string) {
    actingOn = id;
    try {
      await routingOverrides.accept(id);
      await load();
    } catch (e) {
      error = 'Failed to accept override';
    }
    actingOn = null;
  }

  async function reject(id: string) {
    actingOn = id;
    try {
      await routingOverrides.reject(id);
      await load();
    } catch (e) {
      error = 'Failed to reject override';
    }
    actingOn = null;
  }

  function statusColor(status: string): string {
    switch (status) {
      case 'suggested': return 'text-yellow-400 bg-yellow-600/10 border-yellow-600/30';
      case 'accepted': return 'text-green-400 bg-green-600/10 border-green-600/30';
      case 'rejected': return 'text-red-400 bg-red-600/10 border-red-600/30';
      case 'expired': return 'text-gray-500 bg-gray-700/10 border-gray-700/30';
      default: return 'text-gray-400 bg-gray-700/10 border-gray-700/30';
    }
  }

  function confidenceBar(conf: number): string {
    if (conf >= 0.8) return 'bg-green-500';
    if (conf >= 0.6) return 'bg-yellow-500';
    return 'bg-red-500';
  }

  let filtered = $derived(
    filter === 'all' ? overrides : overrides.filter(o => o.status === filter)
  );

  let pendingCount = $derived(overrides.filter(o => o.status === 'suggested').length);

  $effect(() => { load(); });
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <h2 class="text-lg font-semibold text-white">Routing Suggestions</h2>
      {#if pendingCount > 0}
        <span class="px-2 py-0.5 text-[10px] font-medium rounded-full bg-yellow-600/20 text-yellow-400 border border-yellow-600/30">
          {pendingCount} pending
        </span>
      {/if}
    </div>
    <div class="flex items-center gap-3">
      <select
        bind:value={filter}
        class="text-xs rounded-lg bg-gray-800 border border-gray-700 text-gray-300 px-2 py-1"
      >
        <option value="all">All</option>
        <option value="suggested">Pending</option>
        <option value="accepted">Accepted</option>
        <option value="rejected">Rejected</option>
      </select>
      <button
        onclick={detect}
        disabled={detecting}
        class="px-3 py-1.5 text-xs rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors disabled:opacity-50"
      >
        {detecting ? 'Scanning...' : 'Detect Patterns'}
      </button>
    </div>
  </div>

  {#if error}
    <div class="text-xs text-red-400">{error}</div>
  {/if}

  {#if filtered.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      {filter === 'all' ? 'No routing suggestions yet. Click "Detect Patterns" to scan performance data.' : `No ${filter} suggestions.`}
    </div>
  {:else}
    <div class="space-y-3">
      {#each filtered as o}
        <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-3">
          <!-- Header -->
          <div class="flex items-start justify-between">
            <div class="space-y-1">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-white">{o.role}</span>
                <span class="text-xs text-gray-500">{o.task_type}</span>
                <span class="px-1.5 py-0.5 text-[10px] rounded border {statusColor(o.status)} capitalize">
                  {o.status}
                </span>
              </div>
              <div class="flex items-center gap-2 text-xs text-gray-400">
                {#if o.from_model}
                  <span class="font-mono">{o.from_model}</span>
                  <span class="text-gray-600">→</span>
                {/if}
                <span class="font-mono text-purple-400">{o.to_model}</span>
                <span class="text-gray-600">·</span>
                <span>Tier {o.to_tier}</span>
              </div>
            </div>

            {#if o.status === 'suggested'}
              <div class="flex gap-2">
                <button
                  onclick={() => accept(o.id)}
                  disabled={actingOn === o.id}
                  class="px-2.5 py-1 text-xs rounded-lg bg-green-600/20 border border-green-600 text-green-400 hover:bg-green-600/30 transition-colors disabled:opacity-50"
                >
                  Accept
                </button>
                <button
                  onclick={() => reject(o.id)}
                  disabled={actingOn === o.id}
                  class="px-2.5 py-1 text-xs rounded-lg bg-red-600/20 border border-red-600 text-red-400 hover:bg-red-600/30 transition-colors disabled:opacity-50"
                >
                  Reject
                </button>
              </div>
            {/if}
          </div>

          <!-- Reason and confidence -->
          <div class="text-xs text-gray-400">{o.reason}</div>

          <div class="flex items-center gap-4 text-[10px] text-gray-500">
            <div class="flex items-center gap-2 flex-1">
              <span>Confidence</span>
              <div class="flex-1 h-1.5 rounded-full bg-gray-800 max-w-[100px]">
                <div class="h-full rounded-full {confidenceBar(o.confidence)}" style="width: {o.confidence * 100}%"></div>
              </div>
              <span>{(o.confidence * 100).toFixed(0)}%</span>
            </div>
            <span>{o.observations} observations</span>
            <span>{timeAgo(o.created_at)}</span>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
