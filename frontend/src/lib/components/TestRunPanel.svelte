<script lang="ts">
  import { testRunner, type TestRun } from '../api';

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let runs: TestRun[] = $state([]);
  let selectedRun: TestRun | null = $state(null);
  let running: boolean = $state(false);
  let error: string | null = $state(null);
  let showOutput: boolean = $state(false);
  let pollRunTimer: ReturnType<typeof setInterval> | null = $state(null);

  async function fetchRuns() {
    try {
      runs = await testRunner.list(projectId);
      error = null;
    } catch (e) {
      error = 'Failed to load test runs';
    }
  }

  async function triggerRun(testType: string) {
    running = true;
    error = null;
    try {
      selectedRun = await testRunner.trigger(projectId, testType);
      showOutput = false;
      pollRun(selectedRun.id);
    } catch (e) {
      error = 'Failed to trigger test run';
      running = false;
    }
  }

  function pollRun(runId: string) {
    if (pollRunTimer) clearInterval(pollRunTimer);
    pollRunTimer = setInterval(async () => {
      try {
        const updated = await testRunner.get(projectId, runId);
        selectedRun = updated;
        runs = runs.map(r => r.id === updated.id ? updated : r);
        if (updated.status !== 'pending' && updated.status !== 'running') {
          if (pollRunTimer) clearInterval(pollRunTimer);
          pollRunTimer = null;
          running = false;
          await fetchRuns();
        }
      } catch {
        if (pollRunTimer) clearInterval(pollRunTimer);
        pollRunTimer = null;
        running = false;
      }
    }, 3000);
  }

  async function stopRun(runId: string) {
    try {
      await testRunner.stop(projectId, runId);
      running = false;
      if (pollRunTimer) clearInterval(pollRunTimer);
      pollRunTimer = null;
      await fetchRuns();
    } catch (e) {
      error = 'Failed to stop run';
    }
  }

  function selectRun(run: TestRun) {
    selectedRun = run;
    showOutput = false;
  }

  function statusColor(status: string): string {
    switch (status) {
      case 'passed': return 'text-green-400';
      case 'failed': case 'error': return 'text-red-400';
      case 'running': return 'text-blue-400';
      case 'pending': return 'text-yellow-400';
      default: return 'text-gray-400';
    }
  }

  function statusBg(status: string): string {
    switch (status) {
      case 'passed': return 'bg-green-500/10 border-green-500/30';
      case 'failed': case 'error': return 'bg-red-500/10 border-red-500/30';
      case 'running': return 'bg-blue-500/10 border-blue-500/30';
      case 'pending': return 'bg-yellow-500/10 border-yellow-500/30';
      default: return 'bg-gray-500/10 border-gray-500/30';
    }
  }

  function formatDuration(seconds: number | null): string {
    if (seconds == null) return '—';
    if (seconds < 60) return `${Math.round(seconds)}s`;
    const m = Math.floor(seconds / 60);
    const s = Math.round(seconds % 60);
    return `${m}m ${s}s`;
  }

  function formatTime(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  function parseFailed(json: string): string[] {
    try {
      return JSON.parse(json);
    } catch {
      return [];
    }
  }

  let latestRun = $derived(runs.length > 0 ? runs[0] : null);

  $effect(() => {
    if (projectId) {
      fetchRuns();
      const timer = setInterval(fetchRuns, 15000);
      return () => clearInterval(timer);
    }
  });
</script>

<div class="space-y-4">
  <!-- Top bar -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-2">
      <button
        onclick={() => triggerRun('e2e')}
        disabled={running}
        class="px-3 py-1.5 text-xs font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Run E2E
      </button>
      <button
        onclick={() => triggerRun('unit')}
        disabled={running}
        class="px-3 py-1.5 text-xs font-medium rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Unit
      </button>
      <button
        onclick={() => triggerRun('full')}
        disabled={running}
        class="px-3 py-1.5 text-xs font-medium rounded-lg bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Full
      </button>
      {#if running && selectedRun}
        <button
          onclick={() => stopRun(selectedRun!.id)}
          class="px-3 py-1.5 text-xs font-medium rounded-lg bg-red-600 hover:bg-red-500 text-white transition-colors"
        >
          Stop
        </button>
      {/if}
    </div>
    {#if latestRun}
      <div class="text-xs text-gray-500">
        Last run: <span class="text-green-400">{latestRun.passed} passed</span>, <span class="text-red-400">{latestRun.failed} failed</span> — {formatDuration(latestRun.duration_seconds)}
      </div>
    {/if}
  </div>

  <!-- Error banner -->
  {#if error}
    <div class="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-300">{error}</div>
  {/if}

  <!-- Two-column grid -->
  <div class="grid grid-cols-3 gap-4">
    <!-- Left: Run History -->
    <div class="col-span-1 space-y-2 max-h-[600px] overflow-y-auto">
      <h3 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Run History</h3>
      {#each runs as run (run.id)}
        <button
          onclick={() => selectRun(run)}
          class="w-full text-left rounded-lg bg-gray-900 border p-3 transition-colors hover:bg-gray-800/60 {selectedRun?.id === run.id ? 'border-purple-500' : 'border-gray-800'}"
        >
          <div class="flex items-center justify-between mb-1">
            <span class="text-[10px] font-bold uppercase tracking-wider {statusColor(run.status)}">{run.status}</span>
            <span class="text-[10px] text-gray-500">{run.test_type}</span>
          </div>
          <div class="flex items-center justify-between text-[10px] text-gray-500">
            <span>{formatDuration(run.duration_seconds)}</span>
            <span>{formatTime(run.created_at)}</span>
          </div>
          <div class="flex items-center gap-2 mt-1 text-[10px]">
            <span class="text-green-400">{run.passed}P</span>
            <span class="text-red-400">{run.failed}F</span>
            <span class="text-gray-500">{run.skipped}S</span>
          </div>
        </button>
      {/each}
      {#if runs.length === 0}
        <div class="text-center text-gray-600 py-8 text-xs">No test runs yet.</div>
      {/if}
    </div>

    <!-- Right: Run Detail -->
    <div class="col-span-2">
      {#if selectedRun}
        <!-- Status card -->
        <div class="rounded-xl border p-5 {statusBg(selectedRun.status)} mb-4">
          <div class="flex items-center justify-between mb-2">
            <span class="text-sm font-bold uppercase tracking-wider {statusColor(selectedRun.status)}">{selectedRun.status}</span>
            <span class="text-xs text-gray-400">{selectedRun.test_type}</span>
          </div>
          <div class="flex items-center gap-4 text-xs text-gray-400">
            <span>Triggered by: <span class="text-gray-300">{selectedRun.triggered_by}</span></span>
            <span>Duration: <span class="text-gray-300">{formatDuration(selectedRun.duration_seconds)}</span></span>
          </div>
        </div>

        <!-- Pass/Fail/Skip breakdown -->
        <div class="grid grid-cols-3 gap-3 mb-4">
          <div class="rounded-lg bg-gray-900 border border-gray-800 p-3 text-center">
            <div class="text-[10px] text-gray-500 uppercase tracking-wider">Passed</div>
            <div class="text-lg font-semibold text-green-400 mt-0.5">{selectedRun.passed}</div>
          </div>
          <div class="rounded-lg bg-gray-900 border border-gray-800 p-3 text-center">
            <div class="text-[10px] text-gray-500 uppercase tracking-wider">Failed</div>
            <div class="text-lg font-semibold text-red-400 mt-0.5">{selectedRun.failed}</div>
          </div>
          <div class="rounded-lg bg-gray-900 border border-gray-800 p-3 text-center">
            <div class="text-[10px] text-gray-500 uppercase tracking-wider">Skipped</div>
            <div class="text-lg font-semibold text-gray-400 mt-0.5">{selectedRun.skipped}</div>
          </div>
        </div>

        <!-- Failed tests -->
        {#if selectedRun.failed_tests}
          {@const failedList = parseFailed(selectedRun.failed_tests)}
          {#if failedList.length > 0}
            <div class="mb-4">
              <h4 class="text-xs font-semibold text-gray-400 mb-2">Failed Tests</h4>
              <div class="space-y-1">
                {#each failedList as test}
                  <div class="rounded-lg bg-gray-900 border border-red-500/30 px-3 py-2 text-xs text-red-300 font-mono">
                    {test}
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        {/if}

        <!-- Full output -->
        {#if selectedRun.output}
          <div>
            <button
              onclick={() => showOutput = !showOutput}
              class="text-xs text-gray-500 hover:text-gray-300 transition-colors mb-2"
            >
              {showOutput ? 'Hide' : 'Show'} Full Output
            </button>
            {#if showOutput}
              <pre class="bg-gray-950 border border-gray-800 rounded-lg p-4 text-xs text-gray-300 font-mono max-h-96 overflow-auto whitespace-pre-wrap">{selectedRun.output}</pre>
            {/if}
          </div>
        {/if}
      {:else}
        <div class="flex items-center justify-center h-full text-gray-500 text-sm">
          Select a test run to view details
        </div>
      {/if}
    </div>
  </div>
</div>
