<script lang="ts">
  import { coordinator, type CoordinatorMemory } from '../api';
  import { timeAgo } from '../utils';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let coord: CoordinatorMemory | null = $state(null);
  let error: string | null = $state(null);
  let acting: boolean = $state(false);
  let allCoordinators: CoordinatorMemory[] = $state([]);

  async function load() {
    try {
      coord = await coordinator.get(projectId);
      error = null;
    } catch (e) {
      error = 'Failed to load coordinator state';
    }
  }

  async function loadAll() {
    try {
      allCoordinators = await coordinator.list();
    } catch (e) {
      console.warn('Failed to load coordinator list:', e);
    }
  }

  async function wake() {
    acting = true;
    try {
      const sessionId = `manual-${Date.now()}`;
      coord = await coordinator.wake(projectId, sessionId);
      error = null;
    } catch (e) {
      error = 'Failed to wake coordinator';
    }
    acting = false;
  }

  async function sleep() {
    acting = true;
    try {
      coord = await coordinator.sleep(projectId);
      error = null;
    } catch (e) {
      error = 'Failed to sleep coordinator';
    }
    acting = false;
  }

  function stateColor(state: string): string {
    switch (state) {
      case 'active': return 'text-green-400';
      case 'dormant': return 'text-gray-500';
      default: return 'text-yellow-400';
    }
  }

  function stateDot(state: string): string {
    switch (state) {
      case 'active': return 'bg-green-400';
      case 'dormant': return 'bg-gray-600';
      default: return 'bg-yellow-400';
    }
  }

  $effect(() => { load(); loadAll(); });
</script>

<div class="space-y-4">
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold text-white">Coordinator</h2>
    <button
      onclick={() => { load(); loadAll(); }}
      class="text-xs text-gray-500 hover:text-gray-300 transition-colors"
    >
      Refresh
    </button>
  </div>

  {#if error}
    <div class="text-xs text-red-400">{error}</div>
  {/if}

  {#if coord}
    <!-- Current project coordinator -->
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="w-2.5 h-2.5 rounded-full {stateDot(coord.state)} {coord.state === 'active' ? 'animate-pulse' : ''}"></div>
          <div>
            <div class="text-sm font-medium text-white capitalize">{coord.state}</div>
            <div class="text-[10px] text-gray-500">Last active: {timeAgo(coord.last_active_at)}</div>
          </div>
        </div>

        <div class="flex gap-2">
          {#if coord.state === 'dormant'}
            <button
              onclick={wake}
              disabled={acting}
              class="px-3 py-1.5 text-xs rounded-lg bg-green-600/20 border border-green-600 text-green-400 hover:bg-green-600/30 transition-colors disabled:opacity-50"
            >
              {acting ? 'Waking...' : 'Wake'}
            </button>
          {:else}
            <button
              onclick={sleep}
              disabled={acting}
              class="px-3 py-1.5 text-xs rounded-lg bg-gray-700/50 border border-gray-600 text-gray-400 hover:bg-gray-700 transition-colors disabled:opacity-50"
            >
              {acting ? 'Sleeping...' : 'Sleep'}
            </button>
          {/if}
        </div>
      </div>

      <!-- Details -->
      <div class="grid grid-cols-2 gap-3 text-xs">
        <div>
          <span class="text-gray-500">Session</span>
          <div class="text-gray-300 font-mono mt-0.5">{coord.session_id ? coord.session_id.slice(0, 12) + '...' : '—'}</div>
        </div>
        <div>
          <span class="text-gray-500">Created</span>
          <div class="text-gray-300 mt-0.5">{timeAgo(coord.created_at)}</div>
        </div>
      </div>
    </div>

    <!-- All coordinators -->
    {#if allCoordinators.length > 1}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5">
        <h3 class="text-sm font-medium text-gray-400 mb-3">All Project Coordinators</h3>
        <div class="space-y-2">
          {#each allCoordinators as c}
            <div class="flex items-center justify-between py-1.5 px-2 rounded-lg {c.project_id === projectId ? 'bg-gray-800/50' : ''}">
              <div class="flex items-center gap-2">
                <div class="w-2 h-2 rounded-full {stateDot(c.state)}"></div>
                <span class="text-xs text-gray-300 font-mono">{c.project_id.slice(0, 8)}</span>
              </div>
              <span class="text-xs {stateColor(c.state)} capitalize">{c.state}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {:else if !error}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      Loading coordinator state...
    </div>
  {/if}
</div>
