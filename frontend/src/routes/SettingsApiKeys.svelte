<script lang="ts">
  import { settings, type Setting } from '../lib/api';
  import SettingsLayout from './Settings.svelte';

  let apiKeys: Setting[] = $state([]);
  let error: string | null = $state(null);
  let showForm: boolean = $state(false);
  let saving: boolean = $state(false);

  let newKeyName: string = $state('');
  let newKeyValue: string = $state('');

  async function fetchKeys() {
    try {
      const all = await settings.list();
      apiKeys = all.filter((s) => s.category === 'api_keys');
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch API keys';
    }
  }

  $effect(() => { fetchKeys(); });

  async function handleCreate() {
    if (!newKeyName.trim() || !newKeyValue.trim()) return;
    saving = true;
    try {
      const key = `apikey_${newKeyName.trim().toLowerCase().replace(/[^a-z0-9_]/g, '_')}`;
      await settings.upsert(key, { value: newKeyValue.trim(), category: 'api_keys' });
      newKeyName = '';
      newKeyValue = '';
      showForm = false;
      await fetchKeys();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save API key';
    } finally {
      saving = false;
    }
  }

  async function handleDelete(key: string) {
    if (!confirm('Delete this API key?')) return;
    try {
      await settings.delete(key);
      await fetchKeys();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete API key';
    }
  }

  function displayName(key: string): string {
    return key.replace(/^apikey_/, '').replace(/_/g, ' ');
  }
</script>

<SettingsLayout>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-lg font-semibold text-white">API Keys</h2>
        <p class="mt-1 text-sm text-gray-400">Store API keys for agent runtimes and integrations.</p>
      </div>
      <button
        onclick={() => showForm = !showForm}
        class="px-3 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
      >
        {showForm ? 'Cancel' : 'Add Key'}
      </button>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
    {/if}

    {#if showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label for="key-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
            <input
              id="key-name"
              type="text"
              bind:value={newKeyName}
              placeholder="anthropic_api"
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
            />
          </div>
          <div>
            <label for="key-value" class="block text-sm font-medium text-gray-400 mb-1">Value</label>
            <input
              id="key-value"
              type="password"
              bind:value={newKeyValue}
              placeholder="sk-..."
              class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
            />
          </div>
        </div>
        <div class="flex justify-end">
          <button
            onclick={handleCreate}
            disabled={saving || !newKeyName.trim() || !newKeyValue.trim()}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
          >
            {saving ? 'Saving...' : 'Save'}
          </button>
        </div>
      </div>
    {/if}

    {#if apiKeys.length === 0 && !showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
        No API keys stored yet.
      </div>
    {:else}
      <div class="space-y-2">
        {#each apiKeys as key (key.key)}
          <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 flex items-center justify-between">
            <div>
              <span class="text-sm font-medium text-white">{displayName(key.key)}</span>
              <span class="ml-2 text-xs text-gray-500 font-mono">{key.value}</span>
            </div>
            <button
              onclick={() => handleDelete(key.key)}
              class="px-2 py-1 text-xs rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors"
            >
              Delete
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</SettingsLayout>
