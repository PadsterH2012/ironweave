<script lang="ts">
  import { push } from 'svelte-spa-router';
  import {
    projects,
    projectApps,
    teams,
    workflows,
    mounts,
    type AppStatus,
    type Project,
    type Team,
    type CreateTeam,
    type TeamAgentSlot,
    type CreateTeamAgentSlot,
    type UpdateTeamAgentSlot,
    type TeamStatus,
    type MountConfig,
    type WorkflowDefinition,
    RUNTIME_MODELS,
    PREDEFINED_ROLES,
    sync,
  } from '../lib/api';
  import IssueBoard from '../lib/components/IssueBoard.svelte';
  import IntakeChat from '../lib/components/IntakeChat.svelte';
  import ProjectFiles from '../lib/components/ProjectFiles.svelte';
  import ProjectHistory from '../lib/components/ProjectHistory.svelte';
  import ProjectSettings from '../lib/components/ProjectSettings.svelte';
  import MergeQueue from '../lib/components/MergeQueue.svelte';

  interface Props {
    params: { id: string };
  }
  let { params }: Props = $props();

  let project: Project | null = $state(null);
  let teamList: Team[] = $state([]);
  let workflowDefs: WorkflowDefinition[] = $state([]);
  let error: string | null = $state(null);
  let activeTab: string = $state('teams');

  let showIntakeChat: boolean = $state(false);

  // Create team form
  let showTeamForm: boolean = $state(false);
  let teamName: string = $state('');
  let teamMode: string = $state('pipeline');
  let teamMaxAgents: number = $state(3);
  let creatingTeam: boolean = $state(false);

  // Slot management
  let expandedTeamId: string | null = $state(null);
  let teamSlots: Record<string, TeamAgentSlot[]> = $state({});
  let showSlotForm: string | null = $state(null);
  let editingSlotId: string | null = $state(null);

  // Slot form fields
  let slotRole: string = $state('');
  let slotRuntime: string = $state('claude');
  let slotModel: string = $state('');
  let slotOrder: number = $state(0);
  let creatingSlot: boolean = $state(false);

  // Mount state
  let mountState: 'mounted' | 'unmounted' | 'error' | null = $state(null);
  let togglingMount: boolean = $state(false);

  // App preview state
  let appStatus: AppStatus | null = $state(null);
  let togglingApp: boolean = $state(false);

  // Team activation state
  let teamStatuses: Record<string, TeamStatus> = $state({});
  let activatingTeamId: string | null = $state(null);
  let autoPickupSelections: Record<string, string[]> = $state({});

  const PICKUP_TYPES = ['task', 'bug', 'feature'];

  const tabs = $derived([
    { key: 'teams', label: 'Teams' },
    { key: 'issues', label: 'Issues' },
    { key: 'workflows', label: 'Workflows' },
    { key: 'merge-queue', label: 'Merge Queue' },
    { key: 'files', label: 'Files' },
    ...(project?.mount_id ? [{ key: 'history', label: 'History' }] : []),
    { key: 'settings', label: 'Settings' },
  ]);

  async function fetchProject() {
    try {
      project = await projects.get(params.id);
      error = null;
      if (project.mount_id) {
        await fetchMountState(project.mount_id);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch project';
    }
  }

  async function fetchMountState(mountId: string) {
    try {
      const mount = await mounts.get(mountId);
      mountState = mount.state;
    } catch {
      mountState = null;
    }
  }

  async function loadAppStatus() {
    if (project) {
      try {
        appStatus = await projectApps.status(project.id);
      } catch {
        appStatus = null;
      }
    }
  }

  async function handleToggleApp() {
    if (!project || togglingApp) return;
    togglingApp = true;
    try {
      if (appStatus?.state === 'running') {
        await projectApps.stop(project.id);
      } else {
        await projectApps.start(project.id);
      }
      await loadAppStatus();
    } catch (e: any) {
      error = e.message;
    } finally {
      togglingApp = false;
    }
  }

  async function handleToggleMount() {
    if (!project?.mount_id) return;
    togglingMount = true;
    try {
      if (mountState === 'mounted') {
        await mounts.unmount(project.mount_id);
      } else {
        await mounts.mount(project.mount_id);
      }
      await fetchMountState(project.mount_id);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Mount toggle failed';
    } finally {
      togglingMount = false;
    }
  }

  async function fetchTeams() {
    try {
      teamList = await teams.list(params.id);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch teams';
    }
  }

  async function fetchWorkflows() {
    try {
      workflowDefs = await workflows.definitions.list(params.id);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch workflows';
    }
  }

  async function fetchSlots(teamId: string) {
    try {
      const slots = await teams.slots.list(teamId);
      teamSlots = { ...teamSlots, [teamId]: slots };
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch slots';
    }
  }

  function toggleTeamExpand(teamId: string) {
    if (expandedTeamId === teamId) {
      expandedTeamId = null;
    } else {
      expandedTeamId = teamId;
      fetchSlots(teamId);
      // Fetch status for active teams
      const team = teamList.find(t => t.id === teamId);
      if (team?.is_active) {
        fetchTeamStatus(teamId);
      }
    }
    // Reset slot form state
    showSlotForm = null;
    editingSlotId = null;
  }

  function resetSlotForm() {
    slotRole = '';
    slotRuntime = 'claude';
    slotModel = '';
    slotOrder = 0;
  }

  function startEditSlot(slot: TeamAgentSlot) {
    showSlotForm = null;
    editingSlotId = slot.id;
    slotRole = slot.role;
    slotRuntime = slot.runtime;
    slotModel = slot.model ?? '';
    slotOrder = slot.slot_order;
  }

  async function handleCreateSlot(teamId: string) {
    if (!slotRole.trim()) return;
    creatingSlot = true;
    try {
      const data: CreateTeamAgentSlot = {
        role: slotRole.trim(),
        runtime: slotRuntime,
        model: slotModel.trim() || undefined,
        slot_order: slotOrder,
      };
      await teams.slots.create(teamId, data);
      resetSlotForm();
      showSlotForm = null;
      await fetchSlots(teamId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to create slot';
    } finally {
      creatingSlot = false;
    }
  }

  async function handleUpdateSlot(teamId: string, slotId: string) {
    try {
      const data: UpdateTeamAgentSlot = {
        role: slotRole.trim() || undefined,
        runtime: slotRuntime,
        model: slotModel.trim() || null,
        slot_order: slotOrder,
      };
      await teams.slots.update(teamId, slotId, data);
      editingSlotId = null;
      resetSlotForm();
      await fetchSlots(teamId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to update slot';
    }
  }

  async function handleDeleteSlot(teamId: string, slotId: string) {
    if (!confirm('Delete this slot?')) return;
    try {
      await teams.slots.delete(teamId, slotId);
      await fetchSlots(teamId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete slot';
    }
  }

  function runtimeBadge(runtime: string): string {
    switch (runtime.toLowerCase()) {
      case 'claude': return 'bg-purple-600 text-purple-100';
      case 'opencode': return 'bg-blue-600 text-blue-100';
      case 'gemini': return 'bg-green-600 text-green-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  async function autoSync() {
    if (project?.mount_id) {
      try {
        await sync.trigger(params.id);
      } catch { /* non-blocking */ }
    }
  }

  $effect(() => {
    const pid = params.id;
    if (pid) {
      fetchProject().then(() => { autoSync(); loadAppStatus(); });
      fetchTeams().then(() => {
        // Fetch slots for active teams so roles are available for issue creation
        for (const team of teamList) {
          if (team.is_active) {
            fetchSlots(team.id);
          }
        }
      });
      fetchWorkflows();
    }
  });

  async function handleCreateTeam() {
    if (!teamName.trim()) return;
    creatingTeam = true;
    try {
      const data: CreateTeam = {
        name: teamName.trim(),
        project_id: params.id,
        coordination_mode: teamMode,
        max_agents: teamMaxAgents,
      };
      await teams.create(params.id, data);
      teamName = '';
      teamMode = 'pipeline';
      teamMaxAgents = 3;
      showTeamForm = false;
      await fetchTeams();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to create team';
    } finally {
      creatingTeam = false;
    }
  }

  async function handleDeleteTeam(id: string) {
    if (!confirm('Delete this team?')) return;
    try {
      await teams.delete(params.id, id);
      if (expandedTeamId === id) expandedTeamId = null;
      await fetchTeams();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete team';
    }
  }

  async function handleToggleActivation(team: Team) {
    activatingTeamId = team.id;
    try {
      if (team.is_active) {
        await teams.deactivate(params.id, team.id);
      } else {
        await teams.activate(params.id, team.id);
      }
      await fetchTeams();
      // Refresh status if now active
      const updated = teamList.find(t => t.id === team.id);
      if (updated?.is_active) {
        await fetchTeamStatus(team.id);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to toggle team activation';
    } finally {
      activatingTeamId = null;
    }
  }

  async function fetchTeamStatus(teamId: string) {
    try {
      const status = await teams.status(params.id, teamId);
      teamStatuses = { ...teamStatuses, [teamId]: status };
      // Sync auto-pickup selections from status
      autoPickupSelections = { ...autoPickupSelections, [teamId]: status.auto_pickup_types };
    } catch (e) {
      // Non-blocking — status may not be available yet
      console.error('Failed to fetch team status:', e);
    }
  }

  async function handleAutoPickupChange(teamId: string, type: string, checked: boolean) {
    const current = autoPickupSelections[teamId] ?? [];
    let updated: string[];
    if (checked) {
      updated = [...current, type];
    } else {
      updated = current.filter(t => t !== type);
    }
    autoPickupSelections = { ...autoPickupSelections, [teamId]: updated };
    try {
      await teams.updateConfig(params.id, teamId, updated);
      await fetchTeamStatus(teamId);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to update auto-pickup config';
    }
  }

  function contextBadge(ctx: string): string {
    switch (ctx.toLowerCase()) {
      case 'work': return 'bg-blue-600 text-blue-100';
      case 'homelab': return 'bg-green-600 text-green-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function modeBadge(mode: string): string {
    switch (mode.toLowerCase()) {
      case 'pipeline': return 'bg-blue-600 text-blue-100';
      case 'swarm': return 'bg-purple-600 text-purple-100';
      case 'collaborative': return 'bg-green-600 text-green-100';
      case 'hierarchical': return 'bg-orange-600 text-orange-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  const availableModels = $derived(RUNTIME_MODELS[slotRuntime] ?? []);
</script>

<div class="space-y-6">
  <!-- Back link -->
  <button
    onclick={() => push('/projects')}
    class="text-sm text-gray-400 hover:text-white transition-colors"
  >
    &larr; Back to Projects
  </button>

  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  {#if project}
    <!-- Project header -->
    <div class="flex items-center gap-4">
      <div>
        <h1 class="text-2xl font-bold text-white">{project.name}</h1>
        <p class="mt-1 text-sm text-gray-400 font-mono">{project.directory}</p>
        {#if project.description}
          <p class="mt-1 text-sm text-gray-400 line-clamp-2">{project.description}</p>
        {/if}
      </div>
      <span class="text-xs font-medium px-2.5 py-1 rounded-full {contextBadge(project.context)}">
        {project.context}
      </span>
      {#if project.mount_id}
        <button
          onclick={handleToggleMount}
          disabled={togglingMount}
          class="flex items-center gap-2 px-3 py-1.5 text-xs font-medium rounded-full transition-colors {mountState === 'mounted'
            ? 'bg-green-600/20 border border-green-600 text-green-400 hover:bg-red-600/20 hover:border-red-600 hover:text-red-400'
            : 'bg-gray-800 border border-gray-700 text-gray-400 hover:bg-green-600/20 hover:border-green-600 hover:text-green-400'}"
        >
          <span class="w-2 h-2 rounded-full {mountState === 'mounted' ? 'bg-green-400' : mountState === 'error' ? 'bg-red-400' : 'bg-gray-500'}"></span>
          {#if togglingMount}
            ...
          {:else if mountState === 'mounted'}
            Online
          {:else if mountState === 'error'}
            Error
          {:else}
            Offline
          {/if}
        </button>
      {/if}
      <!-- App preview controls -->
      {#if project.app_url}
        <a
          href={project.app_url}
          target="_blank"
          rel="noopener noreferrer"
          class="flex items-center gap-2 px-3 py-1.5 text-xs font-medium rounded-full bg-blue-600/20 border border-blue-600 text-blue-400 hover:bg-blue-600/30 transition-colors"
        >
          <span class="w-2 h-2 rounded-full bg-blue-400"></span>
          Open App ↗
        </a>
      {:else if !project.mount_id}
        <button
          onclick={handleToggleApp}
          disabled={togglingApp}
          class="flex items-center gap-2 px-3 py-1.5 text-xs font-medium rounded-full transition-colors {appStatus?.state === 'running'
            ? 'bg-green-600/20 border border-green-600 text-green-400 hover:bg-red-600/20 hover:border-red-600 hover:text-red-400'
            : appStatus?.state === 'error'
              ? 'bg-red-600/20 border border-red-600 text-red-400 hover:bg-green-600/20 hover:border-green-600 hover:text-green-400'
              : 'bg-gray-800 border border-gray-700 text-gray-400 hover:bg-green-600/20 hover:border-green-600 hover:text-green-400'}"
          title={appStatus?.last_error || ''}
        >
          <span class="w-2 h-2 rounded-full {appStatus?.state === 'running' ? 'bg-green-400' : appStatus?.state === 'error' ? 'bg-red-400' : 'bg-gray-500'}"></span>
          {#if togglingApp}
            ...
          {:else if appStatus?.state === 'running'}
            Stop App
          {:else if appStatus?.state === 'error'}
            Retry
          {:else}
            Start App
          {/if}
        </button>
        {#if appStatus?.state === 'running' && appStatus?.url}
          <a
            href={appStatus.url}
            target="_blank"
            rel="noopener noreferrer"
            class="flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded-full bg-blue-600/20 border border-blue-600 text-blue-400 hover:bg-blue-600/30 transition-colors"
          >
            Open ↗
          </a>
        {/if}
      {/if}
      <button
        onclick={() => showIntakeChat = true}
        class="ml-auto px-4 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
      >
        Submit Request
      </button>
    </div>

    <!-- Tab bar -->
    <div class="border-b border-gray-800">
      <nav class="flex gap-6">
        {#each tabs as tab}
          <button
            onclick={() => activeTab = tab.key}
            class="pb-3 text-sm font-medium border-b-2 transition-colors {activeTab === tab.key
              ? 'border-purple-500 text-white'
              : 'border-transparent text-gray-400 hover:text-gray-200'}"
          >
            {tab.label}
          </button>
        {/each}
      </nav>
    </div>

    <!-- Tab content -->
    {#if activeTab === 'teams'}
      <div class="space-y-4">
        <div class="flex items-center justify-between">
          <h2 class="text-lg font-semibold text-white">Teams</h2>
          <button
            onclick={() => { showTeamForm = !showTeamForm; }}
            class="px-3 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
          >
            {showTeamForm ? 'Cancel' : 'New Team'}
          </button>
        </div>

        {#if showTeamForm}
              <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                  <div>
                    <label for="team-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
                    <input
                      id="team-name"
                      type="text"
                      bind:value={teamName}
                      placeholder="backend-team"
                      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
                    />
                  </div>
                  <div>
                    <label for="team-mode" class="block text-sm font-medium text-gray-400 mb-1">Coordination Mode</label>
                    <select
                      id="team-mode"
                      bind:value={teamMode}
                      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
                    >
                      <option value="pipeline">Pipeline</option>
                      <option value="swarm">Swarm</option>
                      <option value="collaborative">Collaborative</option>
                      <option value="hierarchical">Hierarchical</option>
                    </select>
                  </div>
                  <div>
                    <label for="team-agents" class="block text-sm font-medium text-gray-400 mb-1">Max Agents</label>
                    <input
                      id="team-agents"
                      type="number"
                      min="1"
                      max="20"
                      bind:value={teamMaxAgents}
                      class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
                    />
                  </div>
                </div>
                <div class="flex justify-end">
                  <button
                    onclick={handleCreateTeam}
                    disabled={creatingTeam || !teamName.trim()}
                    class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
                  >
                    {creatingTeam ? 'Creating...' : 'Create'}
                  </button>
                </div>
              </div>
        {/if}

        {#if teamList.length === 0}
          <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
            No teams yet. Create one to get started.
          </div>
        {:else}
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {#each teamList as team (team.id)}
              <div class="rounded-xl bg-gray-900 border border-gray-800 space-y-0 group {expandedTeamId === team.id ? 'col-span-1 sm:col-span-2 lg:col-span-3 border-purple-800' : ''}">
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="p-4 space-y-3 cursor-pointer"
                  onclick={() => toggleTeamExpand(team.id)}
                >
                  <div class="flex items-start justify-between">
                    <div class="flex items-center gap-2">
                      <h3 class="text-sm font-semibold text-white">{team.name}</h3>
                      {#if team.is_active}
                        <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-green-600 text-green-100">Active</span>
                      {:else}
                        <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-gray-600 text-gray-300">Inactive</span>
                      {/if}
                    </div>
                    <button
                      onclick={(e) => { e.stopPropagation(); handleDeleteTeam(team.id); }}
                      class="text-gray-600 hover:text-red-400 text-sm shrink-0 transition-colors opacity-0 group-hover:opacity-100"
                      title="Delete team"
                    >
                      &times;
                    </button>
                  </div>
                  <div class="flex items-center gap-2">
                    <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {modeBadge(team.coordination_mode)}">
                      {team.coordination_mode}
                    </span>
                    <span class="text-xs text-gray-500">
                      {teamSlots[team.id]?.length ?? team.max_agents} {teamSlots[team.id] ? 'slot' : 'max agent'}{(teamSlots[team.id]?.length ?? team.max_agents) !== 1 ? 's' : ''}
                    </span>
                    {#if expandedTeamId === team.id}
                      <span class="text-xs text-gray-600 ml-auto">click to collapse</span>
                    {/if}
                  </div>
                </div>

                {#if expandedTeamId === team.id}
                  <div class="border-t border-gray-800 p-4 space-y-3">
                    <div class="flex items-center justify-between">
                      <h4 class="text-xs font-medium text-gray-400 uppercase tracking-wider">Agent Slots</h4>
                      <button
                        onclick={() => { showSlotForm = showSlotForm === team.id ? null : team.id; editingSlotId = null; resetSlotForm(); }}
                        class="text-xs px-2 py-1 rounded bg-purple-600 hover:bg-purple-500 text-white transition-colors"
                      >
                        {showSlotForm === team.id ? 'Cancel' : 'Add Slot'}
                      </button>
                    </div>

                    {#if !teamSlots[team.id] || teamSlots[team.id].length === 0}
                      <p class="text-xs text-gray-600">No slots configured.</p>
                    {:else}
                      <div class="space-y-2">
                        {#each teamSlots[team.id] as slot (slot.id)}
                          {#if editingSlotId === slot.id}
                            <!-- Inline edit form -->
                            <div class="rounded-lg bg-gray-800 border border-purple-700 p-3 space-y-2">
                              <div class="grid grid-cols-1 sm:grid-cols-4 gap-2">
                                <div>
                                  <label class="block text-[10px] text-gray-500 mb-0.5">Role</label>
                                  <select
                                    bind:value={slotRole}
                                    class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                                  >
                                    <option value="">Select role</option>
                                    {#each PREDEFINED_ROLES as role}
                                      <option value={role}>{role}</option>
                                    {/each}
                                  </select>
                                </div>
                                <div>
                                  <label class="block text-[10px] text-gray-500 mb-0.5">Runtime</label>
                                  <select
                                    bind:value={slotRuntime}
                                    onchange={() => { slotModel = ''; }}
                                    class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                                  >
                                    <option value="claude">Claude</option>
                                    <option value="opencode">OpenCode</option>
                                    <option value="gemini">Gemini</option>
                                  </select>
                                </div>
                                <div>
                                  <label class="block text-[10px] text-gray-500 mb-0.5">Model</label>
                                  {#if availableModels.length > 0}
                                    <select
                                      bind:value={slotModel}
                                      class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                                    >
                                      <option value="">Default</option>
                                      {#each availableModels as m}
                                        <option value={m}>{m}</option>
                                      {/each}
                                    </select>
                                  {:else}
                                    <input
                                      type="text"
                                      bind:value={slotModel}
                                      placeholder="model name"
                                      class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                                    />
                                  {/if}
                                </div>
                                <div class="flex items-end gap-1">
                                  <button
                                    onclick={() => handleUpdateSlot(team.id, slot.id)}
                                    class="px-2 py-1 text-xs rounded bg-green-600 hover:bg-green-500 text-white transition-colors"
                                  >Save</button>
                                  <button
                                    onclick={() => { editingSlotId = null; resetSlotForm(); }}
                                    class="px-2 py-1 text-xs rounded bg-gray-700 hover:bg-gray-600 text-gray-300 transition-colors"
                                  >Cancel</button>
                                </div>
                              </div>
                            </div>
                          {:else}
                            <!-- Slot display row -->
                            <div class="rounded-lg bg-gray-800 px-3 py-2 flex items-center gap-2 group/slot">
                              <span class="text-xs text-gray-200 font-medium min-w-[80px]">{slot.role}</span>
                              <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {runtimeBadge(slot.runtime)}">
                                {slot.runtime}
                              </span>
                              {#if slot.model}
                                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-gray-700 text-gray-300">
                                  {slot.model}
                                </span>
                              {/if}
                              <span class="text-[10px] text-gray-600 ml-auto mr-2">#{slot.slot_order}</span>
                              <button
                                onclick={() => startEditSlot(slot)}
                                class="text-gray-600 hover:text-purple-400 text-xs transition-colors opacity-0 group-hover/slot:opacity-100"
                                title="Edit slot"
                              >edit</button>
                              <button
                                onclick={() => handleDeleteSlot(team.id, slot.id)}
                                class="text-gray-600 hover:text-red-400 text-xs transition-colors opacity-0 group-hover/slot:opacity-100"
                                title="Delete slot"
                              >&times;</button>
                            </div>
                          {/if}
                        {/each}
                      </div>
                    {/if}

                    {#if showSlotForm === team.id}
                      <div class="rounded-lg bg-gray-800 border border-gray-700 p-3 space-y-2">
                        <div class="grid grid-cols-1 sm:grid-cols-4 gap-2">
                          <div>
                            <label class="block text-[10px] text-gray-500 mb-0.5">Role</label>
                            <select
                              bind:value={slotRole}
                              class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                            >
                              <option value="">Select role</option>
                              {#each PREDEFINED_ROLES as role}
                                <option value={role}>{role}</option>
                              {/each}
                            </select>
                          </div>
                          <div>
                            <label class="block text-[10px] text-gray-500 mb-0.5">Runtime</label>
                            <select
                              bind:value={slotRuntime}
                              onchange={() => { slotModel = ''; }}
                              class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                            >
                              <option value="claude">Claude</option>
                              <option value="opencode">OpenCode</option>
                              <option value="gemini">Gemini</option>
                            </select>
                          </div>
                          <div>
                            <label class="block text-[10px] text-gray-500 mb-0.5">Model</label>
                            {#if availableModels.length > 0}
                              <select
                                bind:value={slotModel}
                                class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                              >
                                <option value="">Default</option>
                                {#each availableModels as m}
                                  <option value={m}>{m}</option>
                                {/each}
                              </select>
                            {:else}
                              <input
                                type="text"
                                bind:value={slotModel}
                                placeholder="model name"
                                class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1 text-xs focus:outline-none focus:border-purple-500"
                              />
                            {/if}
                          </div>
                          <div class="flex items-end">
                            <button
                              onclick={() => handleCreateSlot(team.id)}
                              disabled={creatingSlot || !slotRole.trim()}
                              class="px-3 py-1 text-xs rounded bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
                            >
                              {creatingSlot ? 'Adding...' : 'Add'}
                            </button>
                          </div>
                        </div>
                      </div>
                    {/if}

                    <!-- Activation toggle -->
                    <div class="border-t border-gray-800 pt-3 flex items-center gap-3">
                      <button
                        onclick={(e) => { e.stopPropagation(); handleToggleActivation(team); }}
                        disabled={activatingTeamId === team.id}
                        class="px-3 py-1.5 text-xs font-medium rounded-lg transition-colors {team.is_active
                          ? 'bg-red-600 hover:bg-red-500 text-white'
                          : 'bg-green-600 hover:bg-green-500 text-white'} disabled:opacity-50"
                      >
                        {#if activatingTeamId === team.id}
                          ...
                        {:else if team.is_active}
                          Deactivate
                        {:else}
                          Activate
                        {/if}
                      </button>
                    </div>

                    <!-- Auto-pickup config (only when active) -->
                    {#if team.is_active}
                      <div class="border-t border-gray-800 pt-3 space-y-2">
                        <h4 class="text-xs font-medium text-gray-400 uppercase tracking-wider">Auto-Pickup Types</h4>
                        <div class="flex items-center gap-4">
                          {#each PICKUP_TYPES as ptype}
                            <label class="flex items-center gap-1.5 text-xs text-gray-300 cursor-pointer">
                              <input
                                type="checkbox"
                                checked={(autoPickupSelections[team.id] ?? []).includes(ptype)}
                                onchange={(e) => handleAutoPickupChange(team.id, ptype, (e.target as HTMLInputElement).checked)}
                                class="rounded border-gray-600 bg-gray-800 text-purple-500 focus:ring-purple-500 focus:ring-offset-0"
                              />
                              <span class="capitalize">{ptype}s</span>
                            </label>
                          {/each}
                        </div>
                      </div>

                      <!-- Team status display -->
                      {#if teamStatuses[team.id]}
                        <div class="border-t border-gray-800 pt-3 space-y-2">
                          <h4 class="text-xs font-medium text-gray-400 uppercase tracking-wider">Live Status</h4>
                          <div class="space-y-1">
                            {#each teamStatuses[team.id].roles as role}
                              <div class="flex items-center gap-2 text-xs">
                                <span class="text-gray-200 font-medium min-w-[100px] capitalize">{role.role}</span>
                                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {runtimeBadge(role.runtime)}">
                                  {role.runtime}
                                </span>
                                <span class="{role.running > 0 ? 'text-green-400' : 'text-gray-500'}">
                                  {role.running}/{role.slot_count} running
                                </span>
                                {#if role.model}
                                  <span class="text-[10px] text-gray-500">{role.model}</span>
                                {/if}
                              </div>
                            {/each}
                          </div>
                        </div>
                      {/if}
                    {/if}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {:else if activeTab === 'issues'}
      <IssueBoard projectId={params.id} />
    {:else if activeTab === 'workflows'}
      <div class="space-y-4">
        <h2 class="text-lg font-semibold text-white">Workflow Definitions</h2>

        {#if workflowDefs.length === 0}
          <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
            No workflow definitions yet.
          </div>
        {:else}
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {#each workflowDefs as wf (wf.id)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                onclick={() => push(`/projects/${params.id}/workflows/${wf.id}`)}
                class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-2 cursor-pointer hover:border-gray-600 transition-colors group"
              >
                <h3 class="text-sm font-semibold text-white group-hover:text-purple-400 transition-colors">
                  {wf.name}
                </h3>
                <div class="flex items-center gap-3 text-xs text-gray-500">
                  <span>v{wf.version}</span>
                  {#if wf.git_sha}
                    <span class="font-mono">{wf.git_sha.slice(0, 7)}</span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {:else if activeTab === 'merge-queue'}
      <MergeQueue projectId={params.id} />
    {:else if activeTab === 'files'}
      <ProjectFiles projectId={params.id} />
    {:else if activeTab === 'history'}
      <ProjectHistory projectId={params.id} />
    {:else if activeTab === 'settings'}
      <ProjectSettings {project} onUpdate={fetchProject} />
    {/if}
  {:else if !error}
    <div class="text-gray-500">Loading project...</div>
  {/if}
</div>

{#if showIntakeChat}
  <IntakeChat
    projectId={params.id}
    onClose={() => showIntakeChat = false}
    onSubmitted={() => { activeTab = 'issues'; }}
  />
{/if}
