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

  // ── Data types for breakdown ──────────────────────────────────
  interface SuiteResult {
    file: string;
    category: string;
    passed: number;
    failed: number;
    skipped: number;
    total: number;
    failedNames: string[];
  }

  // ── API functions ─────────────────────────────────────────────
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

  // ── Helpers ───────────────────────────────────────────────────
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

  function formatShortTime(iso: string): string {
    const d = new Date(iso);
    return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
  }

  function parseFailed(json: string): string[] {
    try { return JSON.parse(json); } catch { return []; }
  }

  // ── Categorize test files ─────────────────────────────────────
  function categorizeFile(filename: string): string {
    if (filename.startsWith('api-')) return 'API';
    if (filename.startsWith('interact-')) return 'Interactions';
    if (filename.startsWith('killswitch')) return 'Killswitch';
    if (filename.startsWith('data-views') || filename.startsWith('costs-quality')) return 'Data Views';
    if (filename.startsWith('tests-tab')) return 'Test Runner';
    if (filename === 'navigation.spec.ts') return 'Navigation';
    if (filename === 'dashboard.spec.ts') return 'Dashboard';
    return 'Smoke Tests';
  }

  function prettifyFile(filename: string): string {
    return filename
      .replace('.spec.ts', '')
      .replace(/^interact-/, '')
      .replace(/^api-/, '')
      .replace(/-/g, ' ')
      .replace(/\b\w/g, c => c.toUpperCase());
  }

  // ── Parse output JSON into suite results ──────────────────────
  function parseSuiteResults(output: string | null): SuiteResult[] {
    if (!output) return [];
    try {
      const json = JSON.parse(output);
      const results: SuiteResult[] = [];
      for (const suite of json.suites || []) {
        const file = suite.file || '';
        let passed = 0, failed = 0, skipped = 0;
        const failedNames: string[] = [];

        function walkSuite(s: any) {
          for (const spec of s.specs || []) {
            for (const test of spec.tests || []) {
              for (const r of test.results || []) {
                if (r.status === 'passed') passed++;
                else if (r.status === 'failed') { failed++; failedNames.push(spec.title); }
                else if (r.status === 'skipped') skipped++;
              }
            }
          }
          for (const child of s.suites || []) walkSuite(child);
        }
        walkSuite(suite);

        results.push({
          file,
          category: categorizeFile(file),
          passed, failed, skipped,
          total: passed + failed + skipped,
          failedNames,
        });
      }
      return results;
    } catch { return []; }
  }

  // ── Group suites by category ──────────────────────────────────
  interface CategoryGroup {
    name: string;
    passed: number;
    failed: number;
    skipped: number;
    total: number;
    status: 'passed' | 'failed';
    suites: SuiteResult[];
  }

  function groupByCategory(suites: SuiteResult[]): CategoryGroup[] {
    const groups: Record<string, CategoryGroup> = {};
    const order = ['API', 'Navigation', 'Dashboard', 'Smoke Tests', 'Interactions', 'Killswitch', 'Data Views', 'Test Runner'];

    for (const s of suites) {
      if (!groups[s.category]) {
        groups[s.category] = { name: s.category, passed: 0, failed: 0, skipped: 0, total: 0, status: 'passed', suites: [] };
      }
      const g = groups[s.category];
      g.passed += s.passed;
      g.failed += s.failed;
      g.skipped += s.skipped;
      g.total += s.total;
      if (s.failed > 0) g.status = 'failed';
      g.suites.push(s);
    }

    return order.filter(n => groups[n]).map(n => groups[n])
      .concat(Object.keys(groups).filter(n => !order.includes(n)).map(n => groups[n]));
  }

  // ── Derived state ─────────────────────────────────────────────
  let latestRun = $derived(runs.length > 0 ? runs[0] : null);
  let selectedSuites = $derived(selectedRun ? parseSuiteResults(selectedRun.output) : []);
  let selectedCategories = $derived(groupByCategory(selectedSuites));

  // Chart data: last 20 runs in chronological order
  let chartRuns = $derived(runs.slice(0, 20).reverse());

  let expandedCategories: Record<string, boolean> = $state({});

  // Chart dimensions and computed values
  let chartPad = { top: 10, right: 10, bottom: 24, left: 36 };
  let chartW = 600;
  let chartH = 160;
  let plotW = $derived(chartW - chartPad.left - chartPad.right);
  let plotH = $derived(chartH - chartPad.top - chartPad.bottom);
  let maxTests = $derived(Math.max(...chartRuns.map(r => r.passed + r.failed + r.skipped), 1));
  let hasFailures = $derived(chartRuns.some(r => r.failed > 0));

  function chartX(i: number): number {
    return chartPad.left + (chartRuns.length > 1 ? (i / (chartRuns.length - 1)) * plotW : plotW / 2);
  }
  function chartY(val: number): number {
    return chartPad.top + plotH - (val / maxTests) * plotH;
  }

  let passLine = $derived(chartRuns.map((r, i) => `${chartX(i)},${chartY(r.passed)}`).join(' '));
  let failLine = $derived(chartRuns.map((r, i) => `${chartX(i)},${chartY(r.failed)}`).join(' '));
  let passArea = $derived(
    chartRuns.map((r, i) => `${chartX(i)},${chartY(r.passed)}`).join(' ') +
    ` ${chartX(chartRuns.length - 1)},${chartPad.top + plotH} ${chartX(0)},${chartPad.top + plotH}`
  );

  // Y-axis tick values
  let yTicks = $derived(() => {
    const ticks = [];
    const step = Math.ceil(maxTests / 4);
    for (let v = 0; v <= maxTests; v += step) ticks.push(v);
    if (ticks[ticks.length - 1] < maxTests) ticks.push(maxTests);
    return ticks;
  });

  // ── Feature Coverage Matrix ───────────────────────────────────
  // Maps each feature area to what's tested and what's missing
  interface CoverageItem {
    feature: string;
    tested: string[];
    missing: string[];
  }

  const featureCoverage: CoverageItem[] = [
    { feature: 'Routes & Navigation', tested: ['All 11 routes render', 'Sidebar nav links', 'Backend health indicator'], missing: [] },
    { feature: 'Dashboard', tested: ['Stat cards', 'KillSwitch widget', 'System health panel'], missing: ['Metrics chart interaction', 'Agent util chart', 'Merge health chart'] },
    { feature: 'Projects', tested: ['List renders', 'Create project', 'Navigate to detail', 'Delete via API'], missing: ['Edit project inline', 'Mount selection'] },
    { feature: 'Teams', tested: ['List renders', 'Create team', 'Mode selection', 'Delete via API'], missing: ['Agent slot CRUD', 'Activate/deactivate', 'Clone template'] },
    { feature: 'Issues', tested: ['Board columns', 'Create issue', 'Delete via API'], missing: ['Drag between columns', 'Edit priority/role', 'Attachments upload', 'Parent/child hierarchy'] },
    { feature: 'Workflows', tested: ['Tab renders', 'Create definition via API', 'Create instance via API'], missing: ['DAG visualization', 'Gate approvals', 'Pause/resume/cancel instance'] },
    { feature: 'Merge Queue', tested: ['Tab renders'], missing: ['Approve/reject merge', 'Diff viewer', 'Conflict resolution'] },
    { feature: 'Loom', tested: ['Feed renders'], missing: ['Entry type filtering', 'Auto-scroll behavior'] },
    { feature: 'Mounts', tested: ['List renders', 'Create form fields', 'Cancel form'], missing: ['Mount/unmount action', 'SSH test connection', 'Remote browse'] },
    { feature: 'Settings', tested: ['General form fields', 'Proxies tab', 'API Keys tab'], missing: ['Save settings', 'Proxy CRUD', 'Test connection'] },
    { feature: 'Killswitch', tested: ['Dashboard toggle', 'Per-project pause/resume', 'Schedule visibility'], missing: ['Schedule CRUD', 'Cron expression validation'] },
    { feature: 'Quality & Routing', tested: ['Quality tab renders', 'Routing tab renders', 'Detect patterns', 'Create override via API'], missing: ['Slider interaction', 'Accept/reject in UI', 'Team tier overrides'] },
    { feature: 'Costs', tested: ['Cost dashboard renders'], missing: ['Daily spend chart', 'Aggregate trigger', 'Role/model breakdown'] },
    { feature: 'Coordinator', tested: ['Panel renders', 'Wake/sleep toggle'], missing: [] },
    { feature: 'Prompts', tested: ['Editor renders', 'Create template', 'Delete via API'], missing: ['Role assignment', 'Build prompt preview'] },
    { feature: 'Test Runner', tested: ['Tab renders', 'Trigger run', 'Run detail panel', 'Quick-trigger button', 'History list'], missing: [] },
    { feature: 'Agents', tested: ['Page renders', 'Empty state'], missing: ['Spawn agent', 'Stop agent', 'WebSocket terminal'] },
    { feature: 'API Contracts', tested: ['Health', 'All list endpoints', 'Project-scoped endpoints', '404 error handling'], missing: ['POST/PUT validation', 'Auth 401 (disabled)'] },
    { feature: 'Files & Sync', tested: ['Files tab renders', 'Sync status bar', 'Breadcrumb nav', 'Directory navigation', 'Open file viewer', 'Sync trigger via API', 'History API', 'Status API'], missing: ['Diff viewer', 'Restore snapshot'] },
    { feature: 'App Preview', tested: [], missing: ['Start/stop app', 'Status polling', 'Port display'] },
  ];

  let totalTested = $derived(featureCoverage.reduce((n, c) => n + c.tested.length, 0));
  let totalMissing = $derived(featureCoverage.reduce((n, c) => n + c.missing.length, 0));
  let coveragePct = $derived(Math.round((totalTested / (totalTested + totalMissing)) * 100));
  let showCoverageDetail: string | false = $state(false);

  function toggleCategory(name: string) {
    expandedCategories[name] = !expandedCategories[name];
    expandedCategories = { ...expandedCategories };
  }

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

  {#if error}
    <div class="bg-red-500/10 border border-red-500/30 rounded-lg p-3 text-sm text-red-300">{error}</div>
  {/if}

  <!-- Chart + Coverage Row -->
  <div class="grid grid-cols-3 gap-4">
    <!-- Pass/Fail Trend Chart -->
    {#if chartRuns.length > 1}
      <div class="col-span-2 rounded-xl bg-gray-900 border border-gray-800 p-4">
        <div class="flex items-center justify-between mb-3">
          <h3 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Pass / Fail Trend</h3>
          <div class="flex items-center gap-4 text-[10px] text-gray-500">
            <span class="flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-full bg-green-500 inline-block"></span> Passed</span>
            {#if hasFailures}
              <span class="flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-full bg-red-500 inline-block"></span> Failed</span>
            {/if}
          </div>
        </div>
        <svg viewBox="0 0 {chartW} {chartH}" class="w-full" style="height: 160px;">
          {#each yTicks() as val}
            <line x1={chartPad.left} y1={chartY(val)} x2={chartW - chartPad.right} y2={chartY(val)} stroke="rgb(55,65,81)" stroke-width="0.5" />
            <text x={chartPad.left - 6} y={chartY(val) + 3.5} fill="rgb(107,114,128)" font-size="9" text-anchor="end">{val}</text>
          {/each}
          <polygon points={passArea} fill="rgb(74,222,128)" fill-opacity="0.08" />
          <polyline fill="none" stroke="rgb(74,222,128)" stroke-width="2" stroke-linejoin="round" points={passLine} />
          {#if hasFailures}
            <polyline fill="none" stroke="rgb(248,113,113)" stroke-width="2" stroke-linejoin="round" stroke-dasharray="6,3" points={failLine} />
          {/if}
          {#each chartRuns as r, i}
            <circle cx={chartX(i)} cy={chartY(r.passed)} r="3.5" fill={r.failed > 0 ? 'rgb(248,113,113)' : 'rgb(74,222,128)'} stroke="rgb(17,24,39)" stroke-width="1.5" />
          {/each}
          {#each chartRuns as r, i}
            {#if chartRuns.length <= 6 || i === 0 || i === chartRuns.length - 1 || i % Math.max(1, Math.floor(chartRuns.length / 4)) === 0}
              <text x={chartX(i)} y={chartH - 4} fill="rgb(107,114,128)" font-size="9" text-anchor={i === 0 ? 'start' : i === chartRuns.length - 1 ? 'end' : 'middle'}>{formatShortTime(r.created_at)}</text>
            {/if}
          {/each}
        </svg>
      </div>
    {/if}

    <!-- Feature Coverage -->
    <div class="{chartRuns.length > 1 ? 'col-span-1' : 'col-span-3'} rounded-xl bg-gray-900 border border-gray-800 p-4">
      <div class="flex items-center justify-between mb-3">
        <h3 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Feature Coverage</h3>
        <span class="text-xs font-bold {coveragePct >= 80 ? 'text-green-400' : coveragePct >= 50 ? 'text-yellow-400' : 'text-red-400'}">{coveragePct}%</span>
      </div>

      <!-- Coverage bar -->
      <div class="w-full h-2 rounded-full bg-gray-800 mb-3">
        <div
          class="h-full rounded-full transition-all {coveragePct >= 80 ? 'bg-green-500' : coveragePct >= 50 ? 'bg-yellow-500' : 'bg-red-500'}"
          style="width: {coveragePct}%"
        ></div>
      </div>

      <div class="text-[10px] text-gray-500 mb-3">
        {totalTested} tested · {totalMissing} missing
      </div>

      <!-- Feature list -->
      <div class="space-y-1 max-h-[140px] overflow-y-auto">
        {#each featureCoverage as item}
          {@const pct = item.tested.length + item.missing.length > 0 ? Math.round((item.tested.length / (item.tested.length + item.missing.length)) * 100) : 0}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="flex items-center justify-between py-1 px-2 rounded hover:bg-gray-800/60 cursor-pointer text-[10px]"
            onclick={() => showCoverageDetail = showCoverageDetail === item.feature ? false : item.feature}
          >
            <span class="text-gray-300 truncate">{item.feature}</span>
            <span class="shrink-0 ml-2 font-bold {item.missing.length === 0 ? 'text-green-400' : pct >= 50 ? 'text-yellow-400' : 'text-red-400'}">
              {pct === 100 ? '✓' : `${pct}%`}
            </span>
          </div>
          {#if showCoverageDetail === item.feature}
            <div class="pl-4 pb-1 space-y-0.5">
              {#each item.tested as t}
                <div class="text-[9px] text-green-400/70">✓ {t}</div>
              {/each}
              {#each item.missing as m}
                <div class="text-[9px] text-red-400/70">✗ {m}</div>
              {/each}
            </div>
          {/if}
        {/each}
      </div>
    </div>
  </div>

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

        <!-- Pass/Fail/Skip totals -->
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

        <!-- Categorized breakdown -->
        {#if selectedCategories.length > 0}
          <div class="space-y-2 mb-4">
            <h4 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Test Breakdown</h4>
            {#each selectedCategories as cat}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="rounded-lg bg-gray-900 border border-gray-800 overflow-hidden"
                onclick={() => toggleCategory(cat.name)}
              >
                <div class="flex items-center justify-between px-4 py-2.5 cursor-pointer hover:bg-gray-800/60 transition-colors">
                  <div class="flex items-center gap-3">
                    <span class="text-[10px] text-gray-600">{expandedCategories[cat.name] ? '▾' : '▸'}</span>
                    <span class="text-sm font-medium text-gray-200">{cat.name}</span>
                    <span class="text-[10px] text-gray-500">({cat.total})</span>
                  </div>
                  <span class="text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded {cat.status === 'passed' ? 'bg-green-600/20 text-green-400' : 'bg-red-600/20 text-red-400'}">
                    {cat.status === 'passed' ? 'PASSED' : `${cat.failed} FAILED`}
                  </span>
                </div>
                {#if expandedCategories[cat.name]}
                  <div class="border-t border-gray-800 px-4 py-2 space-y-1.5">
                    {#each cat.suites as suite}
                      <div class="flex items-center justify-between text-xs">
                        <div class="flex items-center gap-2">
                          <span class={suite.failed > 0 ? 'text-red-400' : 'text-green-400'}>{suite.failed > 0 ? '✗' : '✓'}</span>
                          <span class="text-gray-300">{prettifyFile(suite.file)}</span>
                        </div>
                        <div class="flex items-center gap-2 text-[10px]">
                          <span class="text-green-400">{suite.passed}</span>
                          {#if suite.failed > 0}
                            <span class="text-red-400">{suite.failed}</span>
                          {/if}
                          {#if suite.skipped > 0}
                            <span class="text-gray-500">{suite.skipped}S</span>
                          {/if}
                        </div>
                      </div>
                      {#if suite.failedNames.length > 0}
                        {#each suite.failedNames as name}
                          <div class="ml-6 text-[10px] text-red-300 font-mono">↳ {name}</div>
                        {/each}
                      {/if}
                    {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
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
