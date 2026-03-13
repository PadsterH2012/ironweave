<script lang="ts">
  import {
    mounts,
    proxyConfigs,
    type MountConfig,
    type CreateMountConfig,
    type ProxyConfigResponse,
    type RemoteBrowseResponse,
  } from '../lib/api';

  let mountList: MountConfig[] = $state([]);
  let proxyList: ProxyConfigResponse[] = $state([]);
  let error: string | null = $state(null);

  // Form state
  let showForm: boolean = $state(false);
  let editingId: string | null = $state(null);
  let formName: string = $state('');
  let formType: string = $state('sshfs');
  let formHost: string = $state('');
  let formPort: number = $state(22);
  let formUsername: string = $state('');
  let formPassword: string = $state('');
  let formSshKey: string = $state('');
  let formRemotePath: string = $state('/');
  let formLocalMount: string = $state('');
  let formMountOptions: string = $state('');
  let formAutoMount: boolean = $state(true);
  let formProxyId: string = $state('');
  let formGitRemote: string = $state('');
  let saving: boolean = $state(false);

  // SSH test state
  let sshTestStatus: 'idle' | 'testing' | 'success' | 'error' = $state('idle');
  let sshTestMessage: string = $state('');

  // Remote browser state
  let showBrowser: boolean = $state(false);
  let browserPath: string = $state('/');
  let browserEntries: Array<{ name: string; type: string }> = $state([]);
  let browserGitRemote: string | null = $state(null);
  let browserLoading: boolean = $state(false);
  let browserError: string | null = $state(null);

  async function fetchMounts() {
    try {
      mountList = await mounts.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch mounts';
    }
  }

  async function fetchProxies() {
    try {
      proxyList = await proxyConfigs.list();
    } catch { /* non-blocking */ }
  }

  $effect(() => {
    fetchMounts();
    fetchProxies();
  });

  function resetForm() {
    editingId = null;
    formName = '';
    formType = 'sshfs';
    formHost = '';
    formPort = 22;
    formUsername = '';
    formPassword = '';
    formSshKey = '';
    formRemotePath = '/';
    formLocalMount = '';
    formMountOptions = '';
    formAutoMount = true;
    formProxyId = '';
    formGitRemote = '';
    sshTestStatus = 'idle';
    sshTestMessage = '';
    showBrowser = false;
    browserPath = '/';
    browserEntries = [];
    browserGitRemote = null;
    browserError = null;
  }

  function openCreate() {
    resetForm();
    showForm = true;
  }

  function openEdit(mount: MountConfig) {
    resetForm();
    editingId = mount.id;
    formName = mount.name;
    formType = mount.mount_type;
    formLocalMount = mount.local_mount_point;
    formMountOptions = mount.mount_options || '';
    formAutoMount = mount.auto_mount;
    formProxyId = mount.proxy_config_id || '';
    formGitRemote = mount.git_remote || '';

    // Parse remote_path for sshfs: user@host:/path
    if (mount.mount_type === 'sshfs' && mount.remote_path.includes('@')) {
      const atIdx = mount.remote_path.indexOf('@');
      const colonIdx = mount.remote_path.indexOf(':', atIdx);
      formUsername = mount.remote_path.substring(0, atIdx);
      formHost = mount.remote_path.substring(atIdx + 1, colonIdx > atIdx ? colonIdx : undefined);
      formRemotePath = colonIdx > atIdx ? mount.remote_path.substring(colonIdx + 1) : '/';
    } else {
      formRemotePath = mount.remote_path;
    }

    // Don't populate redacted passwords
    formPassword = '';
    formSshKey = '';
    showForm = true;
  }

  async function handleDuplicate(id: string) {
    try {
      await mounts.duplicate(id);
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to duplicate mount';
    }
  }

  function buildRemotePath(): string {
    if (formType === 'sshfs') {
      return `${formUsername}@${formHost}:${formRemotePath}`;
    }
    return formRemotePath;
  }

  async function handleSave() {
    if (!formName.trim()) return;
    saving = true;
    try {
      const data: CreateMountConfig = {
        name: formName.trim(),
        mount_type: formType as 'nfs' | 'smb' | 'sshfs',
        remote_path: buildRemotePath(),
        local_mount_point: formLocalMount.trim(),
        auto_mount: formAutoMount,
      };
      if (formUsername) data.username = formUsername;
      if (formPassword) data.password = formPassword;
      if (formSshKey) data.ssh_key = formSshKey;
      if (formMountOptions) data.mount_options = formMountOptions;
      if (formProxyId) data.proxy_config_id = formProxyId;
      if (formGitRemote) data.git_remote = formGitRemote;

      if (editingId) {
        await mounts.update(editingId, data);
      } else {
        await mounts.create(data);
      }
      showForm = false;
      resetForm();
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save mount';
    } finally {
      saving = false;
    }
  }

  async function handleTestSsh() {
    sshTestStatus = 'testing';
    sshTestMessage = '';
    try {
      const result = await mounts.testSsh({
        host: formHost,
        port: formPort,
        username: formUsername,
        password: formPassword || undefined,
        ssh_key: formSshKey || undefined,
        proxy_config_id: formProxyId || undefined,
      });
      sshTestStatus = result.success ? 'success' : 'error';
      sshTestMessage = result.message || result.error || '';
    } catch (e) {
      sshTestStatus = 'error';
      sshTestMessage = e instanceof Error ? e.message : 'Test failed';
    }
  }

  async function browseRemoteDir(path: string) {
    browserLoading = true;
    browserError = null;
    try {
      const result = await mounts.browseRemote({
        host: formHost,
        port: formPort,
        username: formUsername,
        password: formPassword || undefined,
        ssh_key: formSshKey || undefined,
        proxy_config_id: formProxyId || undefined,
        path,
      });
      if (result.error) {
        browserError = result.error;
      } else {
        browserPath = result.path;
        browserEntries = result.entries;
        browserGitRemote = result.git_remote;
      }
    } catch (e) {
      browserError = e instanceof Error ? e.message : 'Browse failed';
    } finally {
      browserLoading = false;
    }
  }

  function openBrowser() {
    showBrowser = true;
    browseRemoteDir(formRemotePath || '/');
  }

  function navigateUp() {
    const parent = browserPath === '/' ? '/' : browserPath.replace(/\/[^/]+\/?$/, '') || '/';
    browseRemoteDir(parent);
  }

  function navigateInto(name: string) {
    const next = browserPath === '/' ? `/${name}` : `${browserPath}/${name}`;
    browseRemoteDir(next);
  }

  function selectPath() {
    formRemotePath = browserPath;
    if (browserGitRemote) {
      formGitRemote = browserGitRemote;
    }
    showBrowser = false;
  }

  async function handleMount(id: string) {
    try {
      await mounts.mount(id);
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Mount failed';
    }
  }

  async function handleUnmount(id: string) {
    try {
      await mounts.unmount(id);
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Unmount failed';
    }
  }

  async function handleDelete(id: string) {
    if (!confirm('Delete this mount configuration?')) return;
    try {
      await mounts.delete(id);
      await fetchMounts();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete mount';
    }
  }

  function stateBadge(state: string): string {
    switch (state) {
      case 'mounted': return 'bg-green-600 text-green-100';
      case 'error': return 'bg-red-600 text-red-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  function typeBadge(type: string): string {
    switch (type) {
      case 'nfs': return 'bg-blue-600 text-blue-100';
      case 'smb': return 'bg-orange-600 text-orange-100';
      case 'sshfs': return 'bg-purple-600 text-purple-100';
      default: return 'bg-gray-600 text-gray-100';
    }
  }

  const canTestSsh = $derived(formType === 'sshfs' && formHost && formUsername);
  const canBrowse = $derived(sshTestStatus === 'success');
</script>

<div class="space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-2xl font-bold text-white">Mounts</h1>
      <p class="mt-1 text-sm text-gray-400">Manage remote filesystem mounts.</p>
    </div>
    <button
      onclick={openCreate}
      class="px-4 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
    >
      {showForm ? 'Cancel' : 'New Mount'}
    </button>
  </div>

  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  <!-- Create / Edit form -->
  {#if showForm}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-5">
      <h2 class="text-lg font-semibold text-white">{editingId ? 'Edit Mount' : 'New Mount'}</h2>

      <!-- Row 1: Name, Type, Proxy -->
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div>
          <label for="m-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
          <input id="m-name" type="text" bind:value={formName} placeholder="my-mount"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
        </div>
        <div>
          <label for="m-type" class="block text-sm font-medium text-gray-400 mb-1">Type</label>
          <select id="m-type" bind:value={formType}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500">
            <option value="sshfs">SSHFS</option>
            <option value="nfs">NFS</option>
            <option value="smb">SMB/CIFS</option>
          </select>
        </div>
        <div>
          <label for="m-proxy" class="block text-sm font-medium text-gray-400 mb-1">Proxy Chain</label>
          <select id="m-proxy" bind:value={formProxyId}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500">
            <option value="">None (direct)</option>
            {#each proxyList as pc}
              <option value={pc.id}>{pc.name} ({pc.hops.length} hops)</option>
            {/each}
          </select>
        </div>
      </div>

      <!-- Row 2: SSH connection (sshfs only) -->
      {#if formType === 'sshfs'}
        <div class="grid grid-cols-1 md:grid-cols-4 gap-4">
          <div class="md:col-span-2">
            <label for="m-host" class="block text-sm font-medium text-gray-400 mb-1">Host</label>
            <input id="m-host" type="text" bind:value={formHost} placeholder="10.202.28.75"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label for="m-port" class="block text-sm font-medium text-gray-400 mb-1">Port</label>
            <input id="m-port" type="number" bind:value={formPort} min="1" max="65535"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label for="m-user" class="block text-sm font-medium text-gray-400 mb-1">Username</label>
            <input id="m-user" type="text" bind:value={formUsername} placeholder="root"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label for="m-pass" class="block text-sm font-medium text-gray-400 mb-1">Password</label>
            <input id="m-pass" type="password" bind:value={formPassword} placeholder="Leave blank for key auth"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
          <div>
            <label for="m-key" class="block text-sm font-medium text-gray-400 mb-1">SSH Key Path</label>
            <input id="m-key" type="text" bind:value={formSshKey} placeholder="/home/paddy/.ssh/id_rsa"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
          </div>
        </div>

        <!-- Test SSH + Browse buttons -->
        <div class="flex items-center gap-3">
          <button
            onclick={handleTestSsh}
            disabled={!canTestSsh || sshTestStatus === 'testing'}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-blue-600 hover:bg-blue-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
          >
            {sshTestStatus === 'testing' ? 'Testing...' : 'Test SSH'}
          </button>
          <button
            onclick={openBrowser}
            disabled={!canBrowse}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-cyan-600 hover:bg-cyan-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
          >
            Browse Remote
          </button>
          {#if sshTestStatus === 'success'}
            <span class="text-sm text-green-400">Connected</span>
          {:else if sshTestStatus === 'error'}
            <span class="text-sm text-red-400">{sshTestMessage}</span>
          {/if}
        </div>

        <!-- Remote file browser -->
        {#if showBrowser}
          <div class="rounded-lg bg-gray-800 border border-gray-700 p-4 space-y-3">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-gray-300">Remote:</span>
                <span class="text-sm font-mono text-gray-400">{browserPath}</span>
                {#if browserGitRemote}
                  <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-orange-600 text-orange-100">
                    git: {browserGitRemote}
                  </span>
                {/if}
              </div>
              <div class="flex items-center gap-2">
                <button
                  onclick={selectPath}
                  class="px-3 py-1 text-xs font-medium rounded-lg bg-green-600 hover:bg-green-500 text-white transition-colors"
                >
                  Use this path
                </button>
                <button
                  onclick={() => showBrowser = false}
                  class="px-3 py-1 text-xs rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
                >
                  Close
                </button>
              </div>
            </div>

            {#if browserError}
              <div class="text-xs text-red-400">{browserError}</div>
            {/if}

            {#if browserLoading}
              <div class="text-sm text-gray-500">Loading...</div>
            {:else}
              <div class="max-h-64 overflow-y-auto space-y-0.5">
                {#if browserPath !== '/'}
                  <button
                    onclick={navigateUp}
                    class="w-full text-left px-2 py-1 text-sm text-gray-400 hover:bg-gray-700 rounded transition-colors font-mono"
                  >
                    ../ (up)
                  </button>
                {/if}
                {#each browserEntries as entry}
                  {#if entry.type === 'directory'}
                    <button
                      onclick={() => navigateInto(entry.name)}
                      class="w-full text-left px-2 py-1 text-sm text-blue-400 hover:bg-gray-700 rounded transition-colors font-mono"
                    >
                      {entry.name}/
                    </button>
                  {:else}
                    <div class="px-2 py-1 text-sm text-gray-500 font-mono">{entry.name}</div>
                  {/if}
                {/each}
                {#if browserEntries.length === 0 && !browserError}
                  <div class="text-sm text-gray-600 px-2 py-1">Empty directory</div>
                {/if}
              </div>
            {/if}
          </div>
        {/if}
      {/if}

      <!-- Git Remote (auto-detected) -->
      {#if formGitRemote}
        <div class="flex items-center gap-2 rounded-lg bg-gray-800 border border-orange-700/40 px-3 py-2">
          <span class="text-[10px] font-medium px-2 py-0.5 rounded-full bg-orange-600 text-orange-100">git</span>
          <span class="text-sm text-gray-300 font-mono truncate">{formGitRemote}</span>
        </div>
      {/if}

      <!-- Row 3: Remote path (for non-sshfs) / Local mount -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <label for="m-remote" class="block text-sm font-medium text-gray-400 mb-1">
            {formType === 'sshfs' ? 'Remote Path' : 'Remote Path (full)'}
          </label>
          <input id="m-remote" type="text" bind:value={formRemotePath}
            placeholder={formType === 'sshfs' ? '/home/user/project' : '//server/share'}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
        </div>
        <div>
          <label for="m-local" class="block text-sm font-medium text-gray-400 mb-1">Local Mount Point</label>
          <input id="m-local" type="text" bind:value={formLocalMount}
            placeholder="/home/paddy/ironweave/mounts/my-mount"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
        </div>
      </div>

      <!-- Row 4: Options, Auto-mount -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div>
          <label for="m-opts" class="block text-sm font-medium text-gray-400 mb-1">Mount Options</label>
          <input id="m-opts" type="text" bind:value={formMountOptions} placeholder="Optional extra options"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500" />
        </div>
        <div class="flex items-end pb-1">
          <label class="flex items-center gap-2 text-sm text-gray-400 cursor-pointer">
            <input type="checkbox" bind:checked={formAutoMount}
              class="rounded bg-gray-800 border-gray-600 text-purple-500 focus:ring-purple-500" />
            Auto-mount on startup
          </label>
        </div>
      </div>

      <!-- Save / Cancel -->
      <div class="flex justify-end gap-3">
        <button
          onclick={() => { showForm = false; resetForm(); }}
          class="px-4 py-2 text-sm rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
        >
          Cancel
        </button>
        <button
          onclick={handleSave}
          disabled={saving || !formName.trim()}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
        >
          {saving ? 'Saving...' : (editingId ? 'Update' : 'Create')}
        </button>
      </div>
    </div>
  {/if}

  <!-- Mount list -->
  {#if mountList.length === 0 && !showForm}
    <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
      No mounts configured. Click "New Mount" to add one.
    </div>
  {:else}
    <div class="space-y-3">
      {#each mountList as mount (mount.id)}
        <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-3 group">
          <div class="flex items-start justify-between">
            <div class="space-y-1">
              <div class="flex items-center gap-2">
                <h3 class="text-base font-semibold text-white">{mount.name}</h3>
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {typeBadge(mount.mount_type)}">
                  {mount.mount_type.toUpperCase()}
                </span>
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {stateBadge(mount.state)}">
                  {mount.state}
                </span>
              </div>
              <p class="text-xs text-gray-500 font-mono">{mount.remote_path}</p>
              {#if mount.git_remote}
                <p class="text-xs text-orange-400 font-mono truncate" title={mount.git_remote}>git: {mount.git_remote}</p>
              {/if}
              <p class="text-xs text-gray-600">&#8594; {mount.local_mount_point}</p>
            </div>

            <div class="flex items-center gap-2">
              {#if mount.state === 'mounted'}
                <button
                  onclick={() => handleUnmount(mount.id)}
                  class="px-3 py-1 text-xs rounded-lg bg-yellow-600/20 text-yellow-400 hover:bg-yellow-600/30 transition-colors"
                >
                  Unmount
                </button>
              {:else}
                <button
                  onclick={() => handleMount(mount.id)}
                  class="px-3 py-1 text-xs rounded-lg bg-green-600/20 text-green-400 hover:bg-green-600/30 transition-colors"
                >
                  Mount
                </button>
              {/if}
              <button
                onclick={() => openEdit(mount)}
                class="px-3 py-1 text-xs rounded-lg bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 transition-colors opacity-0 group-hover:opacity-100"
              >
                Edit
              </button>
              <button
                onclick={() => handleDuplicate(mount.id)}
                class="px-3 py-1 text-xs rounded-lg bg-purple-600/20 text-purple-400 hover:bg-purple-600/30 transition-colors opacity-0 group-hover:opacity-100"
              >
                Duplicate
              </button>
              <button
                onclick={() => handleDelete(mount.id)}
                class="px-3 py-1 text-xs rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors opacity-0 group-hover:opacity-100"
              >
                Delete
              </button>
            </div>
          </div>

          {#if mount.state === 'error' && mount.last_error}
            <div class="text-xs text-red-400 bg-red-900/20 rounded-lg px-3 py-2">
              {mount.last_error}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
