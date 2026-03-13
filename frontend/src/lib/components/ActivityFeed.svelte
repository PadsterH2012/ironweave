<script lang="ts">
  import type { ActivityLogEntry } from '../api';

  interface Props {
    entries: ActivityLogEntry[];
  }

  let { entries }: Props = $props();

  function timeAgo(iso: string): string {
    const diff = Date.now() - new Date(iso).getTime();
    const secs = Math.floor(diff / 1000);
    if (secs < 60) return `${secs}s ago`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    const days = Math.floor(hrs / 24);
    return `${days}d ago`;
  }

  function badgeClass(eventType: string): string {
    switch (eventType) {
      case 'agent_spawned': return 'bg-blue-600/30 text-blue-300 border-blue-600/50';
      case 'agent_completed': return 'bg-green-600/30 text-green-300 border-green-600/50';
      case 'issue_claimed': return 'bg-purple-600/30 text-purple-300 border-purple-600/50';
      case 'merge_success': return 'bg-green-600/30 text-green-300 border-green-600/50';
      case 'merge_conflict': return 'bg-red-600/30 text-red-300 border-red-600/50';
      case 'merge_failed': return 'bg-red-600/30 text-red-300 border-red-600/50';
      case 'workflow_started': return 'bg-blue-600/30 text-blue-300 border-blue-600/50';
      case 'workflow_completed': return 'bg-green-600/30 text-green-300 border-green-600/50';
      default: return 'bg-gray-600/30 text-gray-300 border-gray-600/50';
    }
  }

  function formatEventType(eventType: string): string {
    return eventType.replace(/_/g, ' ');
  }
</script>

<div class="rounded-xl bg-gray-900 border border-gray-800 p-4 h-full flex flex-col">
  <h3 class="text-sm font-semibold text-gray-300 mb-3">Activity Feed</h3>

  {#if entries.length === 0}
    <div class="flex-1 flex items-center justify-center text-gray-500 text-sm">
      No activity yet
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto space-y-2 max-h-96 pr-1">
      {#each entries as entry (entry.id)}
        <div class="flex items-start gap-3 py-2 border-b border-gray-800/50 last:border-0">
          <span class="text-xs text-gray-500 whitespace-nowrap mt-0.5 min-w-[4rem]">
            {timeAgo(entry.created_at)}
          </span>
          <span class="text-xs font-medium px-2 py-0.5 rounded border whitespace-nowrap {badgeClass(entry.event_type)}">
            {formatEventType(entry.event_type)}
          </span>
          <span class="text-sm text-gray-300 truncate">
            {entry.message}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>
