<script lang="ts">
  import type { LoomEntry } from '../api';

  interface Props {
    entries: LoomEntry[];
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

  function typeIcon(entryType: string): string {
    switch (entryType) {
      case 'status': return '\u25cb';
      case 'finding': return '\u2609';
      case 'warning': return '\u26a0';
      case 'delegation': return '\u2192';
      case 'escalation': return '\u2191';
      case 'completion': return '\u2713';
      default: return '\u00b7';
    }
  }

  function typeColor(entryType: string): string {
    switch (entryType) {
      case 'status': return 'text-blue-400';
      case 'finding': return 'text-cyan-400';
      case 'warning': return 'text-yellow-400';
      case 'delegation': return 'text-purple-400';
      case 'escalation': return 'text-red-400';
      case 'completion': return 'text-green-400';
      default: return 'text-gray-400';
    }
  }
</script>

<div class="rounded-xl bg-gray-900 border border-gray-800 p-4 h-full flex flex-col">
  <h3 class="text-sm font-semibold text-gray-300 mb-3">Loom</h3>

  {#if entries.length === 0}
    <div class="flex-1 flex items-center justify-center text-gray-500 text-sm">
      No loom entries yet
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto space-y-1.5 max-h-96 pr-1">
      {#each entries as entry (entry.id)}
        <div class="flex items-start gap-2 py-1.5 border-b border-gray-800/50 last:border-0">
          <span class="{typeColor(entry.entry_type)} text-sm mt-0.5 w-4 text-center flex-shrink-0">
            {typeIcon(entry.entry_type)}
          </span>
          {#if entry.role || entry.agent_id}
            <span class="text-xs text-purple-400 font-mono whitespace-nowrap mt-0.5 flex-shrink-0" title={entry.agent_id ?? ''}>
              {entry.role ?? entry.agent_id?.slice(0, 8) ?? ''}
            </span>
          {/if}
          <span class="text-sm text-gray-300 flex-1 min-w-0 truncate">
            {entry.content}
          </span>
          <span class="text-xs text-gray-500 whitespace-nowrap flex-shrink-0">
            {timeAgo(entry.timestamp)}
          </span>
        </div>
      {/each}
    </div>
  {/if}
</div>
