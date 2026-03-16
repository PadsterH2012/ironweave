<script lang="ts">
  import {
    dispatch,
    type DispatchStatus,
    type DispatchSchedule,
  } from '../api';

  let status: DispatchStatus | null = $state(null);
  let schedules: DispatchSchedule[] = $state([]);
  let showSchedules = $state(false);
  let loading = $state(false);
  let pauseReason = $state('');
  let error: string | null = $state(null);

  let newCron = $state('');
  let newAction: 'pause' | 'resume' = $state('pause');
  let newTz = $state('Europe/London');
  let newDesc = $state('');

  async function refresh() {
    try {
      status = await dispatch.status();
      schedules = await dispatch.schedules.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load dispatch status';
    }
  }

  $effect(() => {
    refresh();
    const iv = setInterval(refresh, 15000);
    return () => clearInterval(iv);
  });

  async function togglePause() {
    loading = true;
    try {
      if (status?.paused) {
        status = await dispatch.resume();
      } else {
        status = await dispatch.pause(pauseReason || undefined);
        pauseReason = '';
      }
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Action failed';
    } finally {
      loading = false;
    }
  }

  async function addSchedule() {
    if (!newCron) return;
    try {
      await dispatch.schedules.create({
        scope: 'global',
        cron_expression: newCron,
        action: newAction,
        timezone: newTz,
        description: newDesc || undefined,
      });
      newCron = '';
      newDesc = '';
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to create schedule';
    }
  }

  async function removeSchedule(id: string) {
    try {
      await dispatch.schedules.delete(id);
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete schedule';
    }
  }

  async function toggleSchedule(s: DispatchSchedule) {
    try {
      await dispatch.schedules.update(s.id, { is_enabled: !s.is_enabled });
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to update schedule';
    }
  }
</script>

<div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-3">
      <span class="w-3 h-3 rounded-full {status?.paused ? 'bg-red-500' : 'bg-green-500'}"></span>
      <h3 class="text-sm font-semibold text-gray-300 uppercase tracking-wider">
        {status?.paused ? 'Dispatch Paused' : 'Dispatch Active'}
      </h3>
    </div>
    <button
      onclick={() => { showSchedules = !showSchedules; }}
      class="text-xs text-gray-500 hover:text-gray-300 transition-colors"
    >
      {showSchedules ? 'Hide' : 'Show'} Schedules
    </button>
  </div>

  {#if error}
    <div class="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-300">{error}</div>
  {/if}

  {#if status?.paused && status.reason}
    <div class="bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2">
      <span class="text-xs text-red-300">Reason: {status.reason}</span>
      {#if status.paused_at}
        <span class="text-[10px] text-red-400 ml-2">since {new Date(status.paused_at).toLocaleString()}</span>
      {/if}
    </div>
  {/if}

  <!-- Pause/Resume controls -->
  <div class="flex items-center gap-3">
    {#if !status?.paused}
      <input
        type="text"
        bind:value={pauseReason}
        placeholder="Pause reason (optional)"
        class="flex-1 bg-gray-800 border border-gray-700 rounded-lg px-3 py-1.5 text-sm text-gray-200 placeholder-gray-500 focus:outline-none focus:border-gray-600"
      />
    {/if}
    <button
      onclick={togglePause}
      disabled={loading}
      class="px-4 py-1.5 text-sm font-medium rounded-lg transition-colors {status?.paused
        ? 'bg-green-600/20 border border-green-600 text-green-400 hover:bg-green-600/30'
        : 'bg-red-600/20 border border-red-600 text-red-400 hover:bg-red-600/30'}"
    >
      {#if loading}
        ...
      {:else if status?.paused}
        Resume Dispatch
      {:else}
        Pause Dispatch
      {/if}
    </button>
  </div>

  <!-- Schedules section -->
  {#if showSchedules}
    <div class="border-t border-gray-800 pt-4 space-y-3">
      <h4 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Schedules</h4>

      {#if schedules.length === 0}
        <p class="text-sm text-gray-500">No schedules configured.</p>
      {:else}
        <div class="space-y-2">
          {#each schedules as s (s.id)}
            <div class="flex items-center gap-3 rounded-lg bg-gray-800/40 border border-gray-800/60 px-3 py-2">
              <button
                onclick={() => toggleSchedule(s)}
                class="w-2 h-2 rounded-full flex-shrink-0 {s.is_enabled ? 'bg-green-400' : 'bg-gray-600'}"
                title={s.is_enabled ? 'Enabled - click to disable' : 'Disabled - click to enable'}
              ></button>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-xs font-mono text-gray-300">{s.cron_expression}</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full {s.action === 'pause' ? 'bg-red-500/20 text-red-300' : 'bg-green-500/20 text-green-300'}">
                    {s.action}
                  </span>
                  <span class="text-[10px] text-gray-600">{s.timezone}</span>
                </div>
                {#if s.description}
                  <div class="text-[10px] text-gray-500 mt-0.5">{s.description}</div>
                {/if}
              </div>
              <button
                onclick={() => removeSchedule(s.id)}
                class="text-xs text-gray-600 hover:text-red-400 transition-colors"
                title="Delete schedule"
              >
                &times;
              </button>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Add schedule form -->
      <div class="bg-gray-800/30 border border-gray-800/50 rounded-lg p-3 space-y-2">
        <p class="text-[10px] text-gray-500 uppercase tracking-wider">Add Schedule</p>
        <div class="flex flex-wrap items-center gap-2">
          <input
            type="text"
            bind:value={newCron}
            placeholder="Cron (e.g. 0 18 * * FRI)"
            class="bg-gray-800 border border-gray-700 rounded px-2 py-1 text-xs text-gray-200 placeholder-gray-600 focus:outline-none focus:border-gray-600 w-44"
          />
          <select
            bind:value={newAction}
            class="bg-gray-800 border border-gray-700 rounded px-2 py-1 text-xs text-gray-200 focus:outline-none focus:border-gray-600"
          >
            <option value="pause">Pause</option>
            <option value="resume">Resume</option>
          </select>
          <input
            type="text"
            bind:value={newTz}
            placeholder="Timezone"
            class="bg-gray-800 border border-gray-700 rounded px-2 py-1 text-xs text-gray-200 placeholder-gray-600 focus:outline-none focus:border-gray-600 w-36"
          />
          <input
            type="text"
            bind:value={newDesc}
            placeholder="Description (optional)"
            class="bg-gray-800 border border-gray-700 rounded px-2 py-1 text-xs text-gray-200 placeholder-gray-600 focus:outline-none focus:border-gray-600 flex-1"
          />
          <button
            onclick={addSchedule}
            disabled={!newCron}
            class="px-3 py-1 text-xs font-medium rounded bg-purple-600 hover:bg-purple-500 text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            Add
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
