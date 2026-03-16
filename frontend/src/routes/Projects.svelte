<script lang="ts">
  import { push } from 'svelte-spa-router';
  import { projects, mounts, proxyConfigs, dispatch, testRunner, type Project, type CreateProject, type ProxyConfigResponse } from '../lib/api';
  import DirectoryBrowser from '../lib/components/DirectoryBrowser.svelte';

  let projectList: Project[] = $state([]);
  let error: string | null = $state(null);
  let showCreateForm: boolean = $state(false);

  // Create form fields
  let newName: string = $state('');
  let newDirectory: string = $state('');
  let newContext: string = $state('work');
  let newGitRemote: string = $state('');
  let creating: boolean = $state(false);
  let showBrowser: boolean = $state(false);
  let sourceType: string = $state('local');
  let remotePath: string = $state('');
  let mountUsername: string = $state('');
  let mountPassword: string = $state('');
  let mountSshKey: string = $state('');
  let mountOptions: string = $state('');
  let proxyList: ProxyConfigResponse[] = $state([]);
  let selectedProxy: string = $state('');
  let globalPaused: boolean = $state(false);
  let togglingPause: Record<string, boolean> = $state({});
  let runningTests: Record<string, string> = $state({});

  async function fetchGlobalPause() {
    try {
      const s = await dispatch.status();
      globalPaused = s.paused;
    } catch { /* ignore */ }
  }

  async function handleTogglePause(e: MouseEvent, project: Project) {
    e.stopPropagation();
    togglingPause[project.id] = true;
    try {
      if (project.is_paused) {
        await dispatch.projectResume(project.id);
      } else {
        await dispatch.projectPause(project.id);
      }
      await fetchProjects();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to toggle pause';
    } finally {
      togglingPause[project.id] = false;
    }
  }

  async function handleRunTests(e: MouseEvent, pid: string) {
    e.stopPropagation();
    runningTests[pid] = 'running';
    runningTests = { ...runningTests };
    try {
      const run = await testRunner.trigger(pid, 'e2e');
      const interval = setInterval(async () => {
        try {
          const updated = await testRunner.get(pid, run.id);
          if (updated.status !== 'pending' && updated.status !== 'running') {
            runningTests[pid] = updated.status;
            runningTests = { ...runningTests };
            clearInterval(interval);
            setTimeout(() => { delete runningTests[pid]; runningTests = { ...runningTests }; }, 10000);
          }
        } catch { clearInterval(interval); delete runningTests[pid]; runningTests = { ...runningTests }; }
      }, 3000);
    } catch {
      runningTests[pid] = 'error';
      runningTests = { ...runningTests };
      setTimeout(() => { delete runningTests[pid]; runningTests = { ...runningTests }; }, 5000);
    }
  }

  async function fetchProjects() {
    try {
      projectList = await projects.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch projects';
    }
  }

  async function fetchProxies() {
    try {
      proxyList = await proxyConfigs.list();
    } catch { /* proxies are optional */ }
  }

  $effect(() => {
    fetchProjects();
    fetchProxies();
    fetchGlobalPause();
  });

  async function handleCreate() {
    if (!newName.trim()) return;
    creating = true;
    try {
      let directory = newDirectory.trim();
      let mount_id: string | undefined;

      if (sourceType !== 'local') {
        const mountData: any = {
          name: `${newName.trim()}-mount`,
          mount_type: sourceType,
          remote_path: remotePath.trim(),
          local_mount_point: `/home/paddy/ironweave/mounts/${newName.trim()}`,
        };
        if (mountUsername.trim()) mountData.username = mountUsername.trim();
        if (mountPassword.trim()) mountData.password = mountPassword.trim();
        if (mountSshKey.trim()) mountData.ssh_key = mountSshKey.trim();
        if (mountOptions.trim()) mountData.mount_options = mountOptions.trim();
        if (selectedProxy) mountData.proxy_config_id = selectedProxy;

        const mount = await mounts.create(mountData);
        mount_id = mount.id;
        directory = mount.local_mount_point;
        if (mount.git_remote && !newGitRemote.trim()) {
          newGitRemote = mount.git_remote;
        }
      }

      const data: CreateProject = {
        name: newName.trim(),
        directory,
        context: newContext,
      };
      if (newGitRemote.trim()) data.git_remote = newGitRemote.trim();
      if (mount_id) data.mount_id = mount_id;

      await projects.create(data);

      newName = ''; newDirectory = ''; newContext = 'work'; newGitRemote = '';
      sourceType = 'local'; remotePath = ''; mountUsername = ''; mountPassword = '';
      mountSshKey = ''; mountOptions = ''; selectedProxy = '';
      showCreateForm = false;
      await fetchProjects();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to create project';
    } finally {
      creating = false;
    }
  }

  async function handleDelete(e: MouseEvent, id: string) {
    e.stopPropagation();
    if (!confirm('Delete this project? This cannot be undone.')) return;
    try {
      await projects.delete(id);
      await fetchProjects();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete project';
    }
  }

  function contextBadge(ctx: string): string {
    switch (ctx.toLowerCase()) {
      case 'work': return 'bg-blue-600 text-blue-100';
      case 'homelab': return 'bg-green-600 text-green-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleDateString('en-GB', {
      day: 'numeric',
      month: 'short',
      year: 'numeric',
    });
  }
</script>

<div class="space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold text-white">Projects</h1>
      <p class="mt-1 text-sm text-gray-400">Manage your Ironweave projects.</p>
    </div>
    <button
      onclick={() => showCreateForm = !showCreateForm}
      class="px-4 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
    >
      {showCreateForm ? 'Cancel' : 'Create Project'}
    </button>
  </div>

  <!-- Error banner -->
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  <!-- Create form -->
  {#if showCreateForm}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <h2 class="text-lg font-semibold text-white">New Project</h2>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <label for="proj-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
          <input
            id="proj-name"
            type="text"
            bind:value={newName}
            placeholder="my-project"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          />
        </div>

        <div>
          <label for="proj-source" class="block text-sm font-medium text-gray-400 mb-1">Source</label>
          <select
            id="proj-source"
            bind:value={sourceType}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          >
            <option value="local">Local</option>
            <option value="nfs">NFS Share</option>
            <option value="smb">SMB Share</option>
            <option value="sshfs">SSH Remote</option>
          </select>
        </div>

        <div>
          <label for="proj-context" class="block text-sm font-medium text-gray-400 mb-1">Context</label>
          <select
            id="proj-context"
            bind:value={newContext}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          >
            <option value="work">Work</option>
            <option value="homelab">Homelab</option>
          </select>
        </div>

        <div>
          <label for="proj-git" class="block text-sm font-medium text-gray-400 mb-1">Git Remote (optional)</label>
          <input
            id="proj-git"
            type="text"
            bind:value={newGitRemote}
            placeholder="https://github.com/..."
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          />
        </div>
      </div>

      {#if sourceType === 'local'}
        <div>
          <label for="proj-dir" class="block text-sm font-medium text-gray-400 mb-1">Directory</label>
          <div class="flex gap-2">
            <input id="proj-dir" type="text" bind:value={newDirectory} placeholder="/path/to/project"
              class="flex-1 rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
            <button type="button" onclick={() => showBrowser = true}
              class="px-3 py-2 text-sm rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors shrink-0">Browse</button>
          </div>
        </div>
      {:else if sourceType === 'nfs'}
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Remote Path</label>
            <input type="text" bind:value={remotePath} placeholder="server:/export/path"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Mount Options (optional)</label>
            <input type="text" bind:value={mountOptions} placeholder="rw,hard,intr"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
        </div>
      {:else if sourceType === 'smb'}
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Remote Path</label>
            <input type="text" bind:value={remotePath} placeholder="//server/share"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Username</label>
            <input type="text" bind:value={mountUsername} placeholder="username"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Password</label>
            <input type="password" bind:value={mountPassword} placeholder="password"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Mount Options (optional)</label>
            <input type="text" bind:value={mountOptions} placeholder="domain=WORKGROUP"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
        </div>
      {:else if sourceType === 'sshfs'}
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Remote Path</label>
            <input type="text" bind:value={remotePath} placeholder="user@host:/path"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">SSH Key Path (optional)</label>
            <input type="text" bind:value={mountSshKey} placeholder="/home/paddy/.ssh/id_rsa"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Mount Options (optional)</label>
            <input type="text" bind:value={mountOptions} placeholder="port=22"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-400 mb-1">Proxy (optional)</label>
            <select bind:value={selectedProxy}
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500">
              <option value="">Direct connection</option>
              {#each proxyList.filter(p => p.is_active) as pc}
                <option value={pc.id}>{pc.name} ({pc.hops.length} hop{pc.hops.length !== 1 ? 's' : ''})</option>
              {/each}
            </select>
          </div>
        </div>
      {/if}

      <div class="flex justify-end">
        <button
          onclick={handleCreate}
          disabled={creating || !newName.trim() || (sourceType === 'local' ? !newDirectory.trim() : !remotePath.trim())}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
        >
          {creating ? 'Creating...' : 'Create'}
        </button>
      </div>
    </div>
  {/if}

  <!-- Project grid -->
  {#if projectList.length === 0}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      No projects yet. Click "Create Project" to get started.
    </div>
  {:else}
    <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
      {#each projectList as project (project.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          onclick={() => push(`/projects/${project.id}`)}
          class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-3 cursor-pointer hover:border-gray-600 transition-colors group"
        >
          <div class="flex items-start justify-between">
            <h3 class="text-base font-semibold text-white group-hover:text-purple-400 transition-colors">
              {project.name}
            </h3>
            <button
              onclick={(e) => handleDelete(e, project.id)}
              class="text-gray-600 hover:text-red-400 text-sm shrink-0 transition-colors opacity-0 group-hover:opacity-100"
              title="Delete project"
            >
              &times;
            </button>
          </div>

          <p class="text-xs text-gray-500 font-mono truncate" title={project.directory}>
            {project.directory}
          </p>

          <div class="flex items-center justify-between">
            <div class="flex items-center gap-1.5">
              <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {contextBadge(project.context)}">
                {project.context}
              </span>
              {#if globalPaused}
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-amber-600/20 text-amber-400 border border-amber-600/30" title="Global pause active">
                  Global Pause
                </span>
              {:else if project.is_paused}
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-red-600/20 text-red-400 border border-red-600/30">
                  Paused
                </span>
              {:else}
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-green-600/20 text-green-400 border border-green-600/30">
                  Active
                </span>
              {/if}
            </div>
            <div class="flex items-center gap-1.5">
              <button
                onclick={(e) => handleRunTests(e, project.id)}
                disabled={runningTests[project.id] === 'running'}
                class="text-[10px] font-medium w-6 h-5 rounded transition-colors flex items-center justify-center {
                  runningTests[project.id] === 'passed' ? 'bg-green-600/20 text-green-400 border border-green-600/30' :
                  runningTests[project.id] === 'failed' ? 'bg-red-600/20 text-red-400 border border-red-600/30' :
                  runningTests[project.id] === 'running' ? 'bg-blue-600/20 text-blue-400 border border-blue-600/30 animate-pulse' :
                  'bg-gray-800 text-gray-400 hover:text-white border border-gray-700'
                } disabled:cursor-not-allowed"
                title="Run E2E tests"
              >
                {#if runningTests[project.id] === 'running'}
                  ⟳
                {:else if runningTests[project.id] === 'passed'}
                  ✓
                {:else if runningTests[project.id] === 'failed'}
                  ✗
                {:else}
                  ▶
                {/if}
              </button>
              <button
                onclick={(e) => handleTogglePause(e, project)}
                disabled={togglingPause[project.id] || globalPaused}
                class="text-[10px] font-medium px-2 py-0.5 rounded-full transition-colors {project.is_paused
                  ? 'bg-green-600/20 text-green-400 hover:bg-green-600/40 border border-green-600/30'
                  : 'bg-red-600/20 text-red-400 hover:bg-red-600/40 border border-red-600/30'} disabled:opacity-40 disabled:cursor-not-allowed"
                title={globalPaused ? 'Global pause is active' : project.is_paused ? 'Resume dispatch' : 'Pause dispatch'}
              >
                {togglingPause[project.id] ? '...' : project.is_paused ? 'Resume' : 'Pause'}
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if showBrowser}
    <DirectoryBrowser
      initialPath={newDirectory || '/home/paddy'}
      onSelect={(path) => { newDirectory = path; showBrowser = false; }}
      onClose={() => showBrowser = false}
    />
  {/if}
</div>
