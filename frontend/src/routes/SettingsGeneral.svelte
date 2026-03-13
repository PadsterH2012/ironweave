<script lang="ts">
  import { settings, type Setting } from '../lib/api';
  import SettingsLayout from './Settings.svelte';

  let settingsList: Setting[] = $state([]);
  let error: string | null = $state(null);
  let success: string | null = $state(null);
  let saving: boolean = $state(false);

  let browseRoots: string = $state('');
  let mountBase: string = $state('');
  let idleMinutes: string = $state('');

  async function fetchSettings() {
    try {
      settingsList = await settings.list();
      const general = settingsList.filter((s) => s.category === 'general');
      for (const s of general) {
        if (s.key === 'browse_roots') {
          try { browseRoots = JSON.parse(s.value).join(', '); } catch { browseRoots = s.value; }
        }
        if (s.key === 'mount_base') mountBase = s.value;
        if (s.key === 'idle_unmount_minutes') idleMinutes = s.value;
      }
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch settings';
    }
  }

  $effect(() => { fetchSettings(); });

  async function handleSave() {
    saving = true;
    success = null;
    try {
      const roots = browseRoots.split(',').map((r) => r.trim()).filter(Boolean);
      await settings.upsert('browse_roots', { value: JSON.stringify(roots), category: 'general' });
      if (mountBase.trim()) {
        await settings.upsert('mount_base', { value: mountBase.trim(), category: 'general' });
      }
      if (idleMinutes.trim()) {
        await settings.upsert('idle_unmount_minutes', { value: idleMinutes.trim(), category: 'general' });
      }
      success = 'Settings saved.';
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save settings';
    } finally {
      saving = false;
    }
  }
</script>

<SettingsLayout>
  <div class="space-y-6">
    <div>
      <h2 class="text-lg font-semibold text-white">General</h2>
      <p class="mt-1 text-sm text-gray-400">Filesystem and mount configuration.</p>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
    {/if}
    {#if success}
      <div class="rounded-lg bg-green-900/40 border border-green-700 px-4 py-3 text-green-300 text-sm">{success}</div>
    {/if}

    <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
      <div>
        <label for="browse-roots" class="block text-sm font-medium text-gray-400 mb-1">Browse Roots</label>
        <input
          id="browse-roots"
          type="text"
          bind:value={browseRoots}
          placeholder="/home/paddy, /opt"
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
        />
        <p class="mt-1 text-xs text-gray-500">Comma-separated list of directories the file browser can access.</p>
      </div>

      <div>
        <label for="mount-base" class="block text-sm font-medium text-gray-400 mb-1">Mount Base Directory</label>
        <input
          id="mount-base"
          type="text"
          bind:value={mountBase}
          placeholder="/home/paddy/ironweave/mounts"
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
        />
      </div>

      <div>
        <label for="idle-minutes" class="block text-sm font-medium text-gray-400 mb-1">Idle Unmount (minutes)</label>
        <input
          id="idle-minutes"
          type="number"
          min="0"
          bind:value={idleMinutes}
          placeholder="30"
          class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
        />
        <p class="mt-1 text-xs text-gray-500">Automatically unmount after this many minutes of no active agent sessions. 0 to disable.</p>
      </div>

      <div class="flex justify-end">
        <button
          onclick={handleSave}
          disabled={saving}
          class="px-4 py-2 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
        >
          {saving ? 'Saving...' : 'Save'}
        </button>
      </div>
    </div>
  </div>
</SettingsLayout>
