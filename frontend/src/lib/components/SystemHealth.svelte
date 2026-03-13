<script lang="ts">
  import type { SystemHealth } from '../api';

  interface Props {
    health: SystemHealth;
  }

  let { health }: Props = $props();

  function barColor(percent: number): string {
    if (percent >= 80) return 'bg-red-500';
    if (percent >= 60) return 'bg-yellow-500';
    return 'bg-green-500';
  }

  const cpuPercent = $derived(Math.round(health.cpu_usage_percent));
  const memPercent = $derived(
    health.memory_total_mb > 0
      ? Math.round((health.memory_used_mb / health.memory_total_mb) * 100)
      : 0
  );
  const diskPercent = $derived(
    health.disk_total_gb > 0
      ? Math.round((health.disk_used_gb / health.disk_total_gb) * 100)
      : 0
  );
</script>

<div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
  <h3 class="text-sm font-semibold text-gray-300">System Health</h3>

  <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
    <!-- CPU -->
    <div class="space-y-1.5">
      <div class="flex items-center justify-between text-xs">
        <span class="text-gray-400">CPU</span>
        <span class="text-gray-300 font-mono">{cpuPercent}%</span>
      </div>
      <div class="w-full h-2 bg-gray-800 rounded-full overflow-hidden">
        <div class="h-full rounded-full transition-all {barColor(cpuPercent)}" style="width: {cpuPercent}%"></div>
      </div>
    </div>

    <!-- Memory -->
    <div class="space-y-1.5">
      <div class="flex items-center justify-between text-xs">
        <span class="text-gray-400">Memory</span>
        <span class="text-gray-300 font-mono">{health.memory_used_mb} / {health.memory_total_mb} MB</span>
      </div>
      <div class="w-full h-2 bg-gray-800 rounded-full overflow-hidden">
        <div class="h-full rounded-full transition-all {barColor(memPercent)}" style="width: {memPercent}%"></div>
      </div>
    </div>

    <!-- Disk -->
    <div class="space-y-1.5">
      <div class="flex items-center justify-between text-xs">
        <span class="text-gray-400">Disk</span>
        <span class="text-gray-300 font-mono">{health.disk_used_gb} / {health.disk_total_gb} GB</span>
      </div>
      <div class="w-full h-2 bg-gray-800 rounded-full overflow-hidden">
        <div class="h-full rounded-full transition-all {barColor(diskPercent)}" style="width: {diskPercent}%"></div>
      </div>
    </div>
  </div>

  <div class="pt-2 border-t border-gray-800">
    <span class="text-xs text-gray-400">Agent processes: </span>
    <span class="text-sm font-bold text-purple-400">{health.agent_process_count}</span>
  </div>
</div>
