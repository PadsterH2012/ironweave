<script lang="ts">
  import { loom, type LoomEntry } from '../api';

  let replyingTo: string | null = $state(null);
  let replyContent: string = $state('');
  let submittingReply: boolean = $state(false);

  async function submitReply(questionEntry: LoomEntry) {
    if (!replyContent.trim()) return;
    submittingReply = true;
    try {
      await loom.answer({
        question_id: questionEntry.id,
        content: replyContent,
        team_id: questionEntry.team_id,
        project_id: questionEntry.project_id,
      });
      replyContent = '';
      replyingTo = null;
      // Re-poll to show the new answer
      poll();
    } catch (e) {
      console.error('Failed to post answer:', e);
    } finally {
      submittingReply = false;
    }
  }
  import { timeAgo } from '../utils';

  interface Props {
    projectId?: string;
    teamId?: string;
    entries?: LoomEntry[];
    pollInterval?: number;
  }

  let { projectId, teamId, entries: externalEntries, pollInterval = 5000 }: Props = $props();

  let internalEntries: LoomEntry[] = $state([]);
  let expandedId: string | null = $state(null);
  let scrollContainer: HTMLDivElement | undefined = $state(undefined);
  let userScrolled = $state(false);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  // Use external entries if provided, otherwise internal (self-polling)
  let entries = $derived(externalEntries ?? internalEntries);

  function typeIcon(entryType: string): string {
    switch (entryType) {
      case 'status': return '\u25cb';
      case 'finding': return '\u2609';
      case 'warning': return '\u26a0';
      case 'delegation': return '\u2192';
      case 'escalation': return '\u2191';
      case 'completion': return '\u2713';
      case 'question': return '\u2753';
      case 'answer': return '\uD83D\uDCAC';
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
      case 'question': return 'text-orange-400';
      case 'answer': return 'text-teal-400';
      default: return 'text-gray-400';
    }
  }

  function runtimeColor(rt: string): string {
    switch (rt.toLowerCase()) {
      case 'claude': return 'text-purple-400';
      case 'opencode': return 'text-blue-400';
      case 'gemini': return 'text-green-400';
      default: return 'text-gray-400';
    }
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  function handleScroll() {
    if (!scrollContainer) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollContainer;
    userScrolled = scrollHeight - scrollTop - clientHeight > 40;
  }

  function scrollToBottom() {
    if (scrollContainer && !userScrolled) {
      scrollContainer.scrollTop = scrollContainer.scrollHeight;
    }
  }

  async function poll() {
    try {
      if (projectId) {
        internalEntries = await loom.byProject(projectId, 100);
      } else if (teamId) {
        internalEntries = await loom.byTeam(teamId, 100);
      } else {
        internalEntries = await loom.recent(100);
      }
    } catch (e) { console.warn('Loom poll failed:', e); }
  }

  // Auto-scroll when entries change
  $effect(() => {
    if (entries.length > 0) {
      // Use tick-like delay to scroll after DOM update
      setTimeout(scrollToBottom, 50);
    }
  });

  // Self-polling when no external entries provided
  $effect(() => {
    if (!externalEntries) {
      poll();
      pollTimer = setInterval(poll, pollInterval);
    }
    return () => {
      if (pollTimer) clearInterval(pollTimer);
    };
  });
</script>

<div class="rounded-xl bg-gray-900 border border-gray-800 p-4 h-full flex flex-col">
  <div class="flex items-center justify-between mb-3">
    <h3 class="text-sm font-semibold text-gray-300">Loom</h3>
    {#if entries.length > 0}
      <span class="text-[10px] text-gray-600">{entries.length} entries</span>
    {/if}
  </div>

  {#if entries.length === 0}
    <div class="flex-1 flex items-center justify-center text-gray-500 text-sm">
      No loom entries yet
    </div>
  {:else}
    <div
      class="flex-1 overflow-y-auto space-y-1 max-h-96 pr-1"
      bind:this={scrollContainer}
      onscroll={handleScroll}
    >
      {#each entries as entry (entry.id)}
        <button
          class="w-full text-left flex flex-col gap-0.5 py-1.5 px-1.5 rounded-lg border border-transparent hover:border-gray-800/80 hover:bg-gray-800/30 transition-colors {expandedId === entry.id ? 'bg-gray-800/40 border-gray-700/50' : ''}"
          onclick={() => toggleExpand(entry.id)}
        >
          <div class="flex items-start gap-2">
            <span class="{typeColor(entry.entry_type)} text-sm mt-0.5 w-4 text-center flex-shrink-0">
              {typeIcon(entry.entry_type)}
            </span>
            {#if entry.role || entry.agent_id}
              <span class="text-xs text-purple-400 font-mono whitespace-nowrap mt-0.5 flex-shrink-0" title={entry.agent_id ?? ''}>
                {entry.role ?? entry.agent_id?.slice(0, 8) ?? ''}{#if entry.model}<span class="{runtimeColor(entry.runtime ?? '')} opacity-70"> — {entry.model}</span>{:else if entry.runtime}<span class="{runtimeColor(entry.runtime)} opacity-70"> — {entry.runtime}</span>{/if}
              </span>
            {/if}
            <span class="text-sm text-gray-300 flex-1 min-w-0 {expandedId === entry.id ? '' : 'truncate'}">
              {entry.content}
            </span>
            <span class="text-xs text-gray-500 whitespace-nowrap flex-shrink-0">
              {timeAgo(entry.timestamp)}
            </span>
          </div>

          {#if expandedId === entry.id}
            <div class="ml-6 mt-1.5 space-y-1 text-[11px] text-gray-500">
              <div class="flex gap-4 flex-wrap">
                <span>Type: <span class="{typeColor(entry.entry_type)} font-medium">{entry.entry_type}</span></span>
                {#if entry.model}
                  <span>Model: <span class="{runtimeColor(entry.runtime ?? '')} font-medium">{entry.model}</span></span>
                {:else if entry.runtime}
                  <span>Runtime: <span class="{runtimeColor(entry.runtime)} font-medium">{entry.runtime}</span></span>
                {/if}
                {#if entry.agent_id}
                  <span>Agent: <span class="text-gray-400 font-mono">{entry.agent_id.slice(0, 12)}</span></span>
                {/if}
              </div>
              <div class="text-gray-600 font-mono text-[10px]">
                {new Date(entry.timestamp).toLocaleString()}
              </div>
              {#if entry.workflow_instance_id}
                <div>Workflow: <span class="text-gray-400 font-mono">{entry.workflow_instance_id.slice(0, 8)}</span></div>
              {/if}
            </div>
          {/if}

          {#if entry.entry_type === 'question'}
            <div class="mt-1 ml-6">
              {#if replyingTo === entry.id}
                <div class="flex gap-2 mt-1" onclick={(e) => e.stopPropagation()}>
                  <input
                    type="text"
                    bind:value={replyContent}
                    placeholder="Type your answer..."
                    class="flex-1 text-xs bg-gray-900 border border-gray-700 rounded px-2 py-1 text-gray-200 focus:border-orange-500 focus:outline-none"
                    onkeydown={(e) => { if (e.key === 'Enter') submitReply(entry); }}
                    onclick={(e) => e.stopPropagation()}
                  />
                  <button
                    onclick={(e) => { e.stopPropagation(); submitReply(entry); }}
                    disabled={submittingReply || !replyContent.trim()}
                    class="text-xs px-2 py-1 rounded bg-teal-600 hover:bg-teal-500 text-white disabled:opacity-50"
                  >
                    Reply
                  </button>
                  <button
                    onclick={(e) => { e.stopPropagation(); replyingTo = null; replyContent = ''; }}
                    class="text-xs px-2 py-1 rounded bg-gray-700 text-gray-300"
                  >
                    Cancel
                  </button>
                </div>
              {:else}
                <button
                  onclick={(e) => { e.stopPropagation(); replyingTo = entry.id; }}
                  class="text-[10px] text-orange-400 hover:text-orange-300"
                >
                  Reply to this question
                </button>
              {/if}
            </div>
          {/if}
        </button>
      {/each}
    </div>

    {#if userScrolled}
      <button
        class="mt-1 text-[10px] text-blue-400 hover:text-blue-300 transition-colors text-center"
        onclick={() => { userScrolled = false; scrollToBottom(); }}
      >
        Jump to latest
      </button>
    {/if}
  {/if}
</div>
