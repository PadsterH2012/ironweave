<script lang="ts">
  import { projects, mounts, type Project, type MountConfig, type UpdateProject } from '../api';

  interface Props {
    project: Project;
    onUpdate: () => void;
  }
  let { project, onUpdate }: Props = $props();

  let mountList: MountConfig[] = $state([]);
  let saving: boolean = $state(false);
  let error: string | null = $state(null);
  let success: string | null = $state(null);

  // Form fields
  let formName: string = $state('');
  let formContext: string = $state('');
  let formDescription: string = $state('');
  let formDirectory: string = $state('');
  let formGitRemote: string = $state('');
  let formMountId: string = $state('');
  let formAppUrl: string = $state('');

  let selectedMount = $derived(
    formMountId ? mountList.find(m => m.id === formMountId) ?? null : null
  );

  function resetForm(p: Project) {
    formName = p.name;
    formContext = p.context;
    formDescription = p.description ?? '';
    formDirectory = p.directory;
    formGitRemote = p.git_remote ?? '';
    formMountId = p.mount_id ?? '';
    formAppUrl = p.app_url ?? '';
    error = null;
    success = null;
  }

  function buildChanges(): UpdateProject {
    const changes: UpdateProject = {};
    if (formName !== project.name) changes.name = formName;
    if (formContext !== project.context) changes.context = formContext;
    if (formDescription !== (project.description ?? '')) changes.description = formDescription;
    if (formDirectory !== project.directory) changes.directory = formDirectory;
    if (formGitRemote !== (project.git_remote ?? '')) changes.git_remote = formGitRemote;
    if (formMountId !== (project.mount_id ?? '')) changes.mount_id = formMountId || undefined;
    if (formAppUrl !== (project.app_url ?? '')) changes.app_url = formAppUrl || undefined;
    return changes;
  }

  let hasChanges = $derived(
    formName !== project.name ||
    formContext !== project.context ||
    formDescription !== (project.description ?? '') ||
    formDirectory !== project.directory ||
    formGitRemote !== (project.git_remote ?? '') ||
    formMountId !== (project.mount_id ?? '') ||
    formAppUrl !== (project.app_url ?? '')
  );

  async function fetchMounts() {
    try {
      mountList = await mounts.list();
    } catch (e) {
      console.error('Failed to fetch mounts:', e);
    }
  }

  async function handleSave() {
    const changes = buildChanges();
    if (Object.keys(changes).length === 0) return;

    saving = true;
    error = null;
    success = null;
    try {
      await projects.update(project.id, changes);
      success = 'Project settings saved successfully.';
      onUpdate();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save changes';
    } finally {
      saving = false;
    }
  }

  function mountStateColor(state: string): string {
    switch (state) {
      case 'mounted': return 'text-green-400';
      case 'unmounted': return 'text-yellow-400';
      case 'error': return 'text-red-400';
      default: return 'text-gray-400';
    }
  }

  $effect(() => {
    resetForm(project);
  });

  $effect(() => {
    fetchMounts();
  });

  // Auto-fill git_remote from selected mount when field is empty
  $effect(() => {
    if (selectedMount?.git_remote && !formGitRemote) {
      formGitRemote = selectedMount.git_remote;
    }
  });
</script>

<div class="max-w-2xl space-y-6">
  {#if error}
    <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">
      {error}
    </div>
  {/if}

  {#if success}
    <div class="rounded-lg bg-green-900/40 border border-green-700 px-4 py-3 text-green-300 text-sm">
      {success}
    </div>
  {/if}

  <div class="rounded-xl bg-gray-900 border border-gray-800 p-6 space-y-5">
    <!-- Name -->
    <div>
      <label for="settings-name" class="block text-sm font-medium text-gray-300 mb-1">Name</label>
      <input
        id="settings-name"
        type="text"
        bind:value={formName}
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500 transition-colors"
      />
    </div>

    <!-- Context -->
    <div>
      <label for="settings-context" class="block text-sm font-medium text-gray-300 mb-1">Context</label>
      <select
        id="settings-context"
        bind:value={formContext}
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500 transition-colors"
      >
        <option value="work">Work</option>
        <option value="homelab">Homelab</option>
      </select>
    </div>

    <!-- Description -->
    <div>
      <label for="settings-description" class="block text-sm font-medium text-gray-300 mb-1">Description</label>
      <textarea
        id="settings-description"
        bind:value={formDescription}
        rows="3"
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500 transition-colors resize-none"
      ></textarea>
    </div>

    <!-- Mount -->
    <div>
      <label for="settings-mount" class="block text-sm font-medium text-gray-300 mb-1">Mount</label>
      <select
        id="settings-mount"
        bind:value={formMountId}
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500 transition-colors"
      >
        <option value="">None (local only)</option>
        {#each mountList as mount (mount.id)}
          <option value={mount.id}>{mount.name} — {mount.remote_path}</option>
        {/each}
      </select>
      {#if selectedMount}
        <div class="mt-2 text-xs flex items-center gap-2">
          <span class="text-gray-500">State:</span>
          <span class="font-medium {mountStateColor(selectedMount.state)}">{selectedMount.state}</span>
          <span class="text-gray-600">|</span>
          <span class="text-gray-500">Type:</span>
          <span class="text-gray-400">{selectedMount.mount_type}</span>
        </div>
      {/if}
    </div>

    {#if selectedMount}
      <!-- Remote details (from mount) -->
      <div class="rounded-lg bg-gray-800/50 border border-gray-700/50 p-4 space-y-3">
        <h3 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Remote</h3>
        <div>
          <label class="block text-xs text-gray-500 mb-1">Remote Path</label>
          <div class="text-sm text-gray-300 font-mono">{selectedMount.remote_path}</div>
        </div>
        <div>
          <label for="settings-git-remote" class="block text-xs text-gray-500 mb-1">Git Remote</label>
          <input
            id="settings-git-remote"
            type="text"
            bind:value={formGitRemote}
            placeholder="Not detected — set manually or re-browse in Mounts"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500 transition-colors placeholder:text-gray-600"
          />
        </div>
      </div>

      <!-- Local details -->
      <div class="rounded-lg bg-gray-800/50 border border-gray-700/50 p-4 space-y-3">
        <h3 class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Local</h3>
        <div>
          <label for="settings-directory" class="block text-xs text-gray-500 mb-1">Local Directory (mount point)</label>
          <input
            id="settings-directory"
            type="text"
            bind:value={formDirectory}
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500 transition-colors"
          />
        </div>
      </div>
    {:else}
      <!-- No mount — simple directory + git remote -->
      <div>
        <label for="settings-directory" class="block text-sm font-medium text-gray-300 mb-1">Directory</label>
        <input
          id="settings-directory"
          type="text"
          bind:value={formDirectory}
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500 transition-colors"
        />
      </div>
      <div>
        <label for="settings-git-remote" class="block text-sm font-medium text-gray-300 mb-1">Git Remote</label>
        <input
          id="settings-git-remote"
          type="text"
          bind:value={formGitRemote}
          placeholder="https://github.com/..."
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500 transition-colors placeholder:text-gray-600"
        />
      </div>
    {/if}

    <!-- App URL (for remote/hosted projects) -->
    <div>
      <label for="settings-app-url" class="block text-sm font-medium text-gray-300 mb-1">App URL</label>
      <input
        id="settings-app-url"
        type="text"
        bind:value={formAppUrl}
        placeholder="https://myapp.example.com"
        class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm font-mono focus:outline-none focus:border-purple-500 transition-colors placeholder:text-gray-600"
      />
      <p class="mt-1 text-xs text-gray-500">Set a URL for remote/hosted apps. When set, an "Open App" link appears in the project header instead of Start/Stop controls.</p>
    </div>

    <!-- Save button -->
    <div class="pt-2 flex items-center justify-between">
      <button
        onclick={() => resetForm(project)}
        disabled={!hasChanges}
        class="px-4 py-2 text-sm rounded-lg bg-gray-800 text-gray-300 hover:bg-gray-700 disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
      >
        Reset
      </button>
      <button
        onclick={handleSave}
        disabled={saving || !hasChanges}
        class="px-6 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
      >
        {saving ? 'Saving...' : 'Save Changes'}
      </button>
    </div>
  </div>
</div>
