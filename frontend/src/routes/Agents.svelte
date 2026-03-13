<script lang="ts">
  import { agents, projects, type AgentInfo, type SpawnAgentRequest, type Project } from '../lib/api';
  import Terminal from '../lib/components/Terminal.svelte';

  let agentList: AgentInfo[] = $state([]);
  let projectList: Project[] = $state([]);
  let error: string | null = $state(null);
  let expandedId: string | null = $state(null);
  let showSpawnForm: boolean = $state(false);

  // Spawn form fields
  let spawnRuntime: string = $state('claude');
  let spawnPrompt: string = $state('');
  let spawnDir: string = $state('/home/paddy');
  let selectedProjectId: string = $state('');
  let spawning: boolean = $state(false);

  async function fetchAgents() {
    try {
      agentList = await agents.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch agents';
    }
  }

  async function fetchProjects() {
    try {
      projectList = await projects.list();
    } catch {
      // Non-critical — project dropdown just stays empty
    }
  }

  $effect(() => {
    fetchAgents();
    fetchProjects();
    const interval = setInterval(fetchAgents, 5000);
    return () => clearInterval(interval);
  });

  function handleProjectChange() {
    if (selectedProjectId) {
      const proj = projectList.find((p) => p.id === selectedProjectId);
      if (proj) {
        spawnDir = proj.directory;
      }
    }
  }

  function truncateId(id: string): string {
    return id.length > 8 ? id.slice(0, 8) : id;
  }

  function runtimeColor(runtime: string): string {
    switch (runtime.toLowerCase()) {
      case 'claude': return 'bg-purple-600 text-purple-100';
      case 'opencode': return 'bg-blue-600 text-blue-100';
      case 'gemini': return 'bg-green-600 text-green-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function stateColor(state: string): string {
    switch (state.toLowerCase()) {
      case 'running': return 'bg-green-400 animate-pulse';
      case 'exited': return 'bg-gray-400';
      case 'crashed': return 'bg-red-500';
      default: return 'bg-gray-400';
    }
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  async function handleSpawn() {
    if (!spawnPrompt.trim()) return;
    spawning = true;
    try {
      const data: SpawnAgentRequest = {
        runtime: spawnRuntime,
        working_directory: spawnDir,
        prompt: spawnPrompt,
      };
      await agents.spawn(data);
      showSpawnForm = false;
      spawnPrompt = '';
      await fetchAgents();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to spawn agent';
    } finally {
      spawning = false;
    }
  }

  async function handleStop(id: string) {
    try {
      await agents.stop(id);
      await fetchAgents();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to stop agent';
    }
  }
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold text-white">Agents</h1>
      <p class="mt-1 text-sm text-gray-400">Monitor and manage active agent sessions.</p>
    </div>
    <button
      onclick={() => showSpawnForm = !showSpawnForm}
      class="px-4 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
    >
      {showSpawnForm ? 'Cancel' : 'Spawn Agent'}
    </button>
  </div>

  <!-- Error banner -->
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  <!-- Spawn form -->
  {#if showSpawnForm}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <h2 class="text-lg font-semibold text-white">Spawn New Agent</h2>

      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div>
          <label for="spawn-runtime" class="block text-sm font-medium text-gray-400 mb-1">Runtime</label>
          <select
            id="spawn-runtime"
            bind:value={spawnRuntime}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          >
            <option value="claude">Claude</option>
            <option value="opencode">OpenCode</option>
            <option value="gemini">Gemini</option>
          </select>
        </div>

        <div>
          <label for="spawn-project" class="block text-sm font-medium text-gray-400 mb-1">Project (optional)</label>
          <select
            id="spawn-project"
            bind:value={selectedProjectId}
            onchange={handleProjectChange}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          >
            <option value="">-- None --</option>
            {#each projectList as proj (proj.id)}
              <option value={proj.id}>{proj.name}</option>
            {/each}
          </select>
        </div>

        <div>
          <label for="spawn-dir" class="block text-sm font-medium text-gray-400 mb-1">Working Directory</label>
          <input
            id="spawn-dir"
            type="text"
            bind:value={spawnDir}
            placeholder="/home/paddy"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          />
        </div>
      </div>

      <div>
        <label for="spawn-prompt" class="block text-sm font-medium text-gray-400 mb-1">Prompt</label>
        <textarea
          id="spawn-prompt"
          bind:value={spawnPrompt}
          rows="3"
          placeholder="Enter the task prompt for the agent..."
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500 resize-none"
        ></textarea>
      </div>

      <div class="flex justify-end">
        <button
          onclick={handleSpawn}
          disabled={spawning || !spawnPrompt.trim()}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
        >
          {spawning ? 'Spawning...' : 'Spawn'}
        </button>
      </div>
    </div>
  {/if}

  <!-- Agent panels -->
  {#if agentList.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      No active agent sessions. Click "Spawn Agent" to start one.
    </div>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {#each agentList as agent (agent.id)}
        <div
          class="rounded-xl bg-gray-900 border border-gray-800 overflow-hidden flex flex-col transition-all duration-200"
          class:col-span-full={expandedId === agent.id}
          class:md:col-span-full={expandedId === agent.id}
          class:lg:col-span-full={expandedId === agent.id}
        >
          <!-- Header bar -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            onclick={() => toggleExpand(agent.id)}
            class="flex items-center justify-between px-4 py-2.5 bg-gray-800/60 border-b border-gray-800 hover:bg-gray-800 transition-colors w-full text-left cursor-pointer"
            role="button"
            tabindex="0"
          >
            <div class="flex items-center gap-3 min-w-0">
              <!-- State dot -->
              <span class="inline-block h-2.5 w-2.5 rounded-full shrink-0 {stateColor(agent.state)}"></span>
              <!-- Agent ID -->
              <span class="font-mono text-sm text-gray-200 truncate" title={agent.id}>
                {truncateId(agent.id)}
              </span>
              <!-- Runtime badge -->
              <span class="text-xs font-medium px-2 py-0.5 rounded-full shrink-0 {runtimeColor(agent.runtime)}">
                {agent.runtime}
              </span>
            </div>
            <div class="flex items-center gap-2 shrink-0 ml-2">
              <span class="text-xs text-gray-500 capitalize">{agent.state}</span>
              <button
                onclick={(e: MouseEvent) => { e.stopPropagation(); handleStop(agent.id); }}
                class="text-xs px-2 py-1 rounded bg-red-900/40 hover:bg-red-800/60 text-red-400 hover:text-red-300 transition-colors"
                title="Stop agent"
              >
                Stop
              </button>
            </div>
          </div>

          <!-- Terminal area -->
          <div
            class="bg-gray-950"
            style="height: {expandedId === agent.id ? '600px' : '300px'}"
          >
            <Terminal agentId={agent.id} />
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
