<script lang="ts">
  import { proxyConfigs, type ProxyConfigResponse, type ProxyHop, type TestConnectionResult } from '../lib/api';
  import SettingsLayout from './Settings.svelte';

  let configList: ProxyConfigResponse[] = $state([]);
  let error: string | null = $state(null);
  let showForm: boolean = $state(false);
  let editing: string | null = $state(null);
  let saving: boolean = $state(false);
  let testing: string | null = $state(null);
  let testResult: TestConnectionResult | null = $state(null);

  let formName: string = $state('');
  let formHops: ProxyHop[] = $state([]);

  async function fetchConfigs() {
    try {
      configList = await proxyConfigs.list();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to fetch proxy configs';
    }
  }

  $effect(() => { fetchConfigs(); });

  function resetForm() {
    formName = '';
    formHops = [];
    editing = null;
    showForm = false;
  }

  function addHop() {
    formHops = [...formHops, { host: '', port: 22, username: '', auth_type: 'key', credential: null }];
  }

  function removeHop(index: number) {
    formHops = formHops.filter((_, i) => i !== index);
  }

  function startEdit(pc: ProxyConfigResponse) {
    formName = pc.name;
    formHops = pc.hops.map((h) => ({ ...h, credential: h.credential === '***' ? null : h.credential }));
    editing = pc.id;
    showForm = true;
  }

  async function handleSave() {
    if (!formName.trim()) return;
    saving = true;
    try {
      if (editing) {
        await proxyConfigs.update(editing, { name: formName.trim(), hops: formHops });
      } else {
        await proxyConfigs.create({ name: formName.trim(), hops: formHops });
      }
      resetForm();
      await fetchConfigs();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to save proxy config';
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    if (!confirm('Delete this proxy configuration?')) return;
    try {
      await proxyConfigs.delete(id);
      await fetchConfigs();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to delete proxy config';
    }
  }

  async function handleTest(id: string) {
    testing = id;
    testResult = null;
    try {
      testResult = await proxyConfigs.test(id);
    } catch (e) {
      testResult = { success: false, error: e instanceof Error ? e.message : 'Test failed' };
    } finally {
      testing = null;
    }
  }

  async function handleToggle(pc: ProxyConfigResponse) {
    try {
      await proxyConfigs.update(pc.id, { is_active: !pc.is_active });
      await fetchConfigs();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to update proxy config';
    }
  }
</script>

<SettingsLayout>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h2 class="text-lg font-semibold text-white">Proxy Configurations</h2>
        <p class="mt-1 text-sm text-gray-400">SSH proxy chains for reaching remote hosts through tunnels.</p>
      </div>
      <button
        onclick={() => { if (showForm) resetForm(); else { showForm = true; addHop(); } }}
        class="px-3 py-1.5 text-sm font-medium rounded-lg bg-purple-600 hover:bg-purple-500 text-white transition-colors"
      >
        {showForm ? 'Cancel' : 'Add Proxy'}
      </button>
    </div>

    {#if error}
      <div class="rounded-lg bg-red-900/40 border border-red-700 px-4 py-3 text-red-300 text-sm">{error}</div>
    {/if}

    {#if testResult}
      <div class="rounded-lg px-4 py-3 text-sm {testResult.success ? 'bg-green-900/40 border border-green-700 text-green-300' : 'bg-red-900/40 border border-red-700 text-red-300'}">
        {testResult.success ? testResult.message : testResult.error}
      </div>
    {/if}

    {#if showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-5 space-y-4">
        <h3 class="text-sm font-semibold text-white">{editing ? 'Edit Proxy' : 'New Proxy'}</h3>

        <div>
          <label for="proxy-name" class="block text-sm font-medium text-gray-400 mb-1">Name</label>
          <input
            id="proxy-name"
            type="text"
            bind:value={formName}
            placeholder="cuk-proxy-chain"
            class="w-full rounded-lg bg-gray-800 border border-gray-700 text-gray-200 px-3 py-2 text-sm focus:outline-none focus:border-purple-500"
          />
        </div>

        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-sm font-medium text-gray-400">Hops</span>
            <button
              onclick={addHop}
              class="text-xs text-purple-400 hover:text-purple-300 transition-colors"
            >
              + Add Hop
            </button>
          </div>

          {#each formHops as hop, i}
            <div class="rounded-lg bg-gray-800 border border-gray-700 p-3 space-y-2">
              <div class="flex items-center justify-between">
                <span class="text-xs text-gray-500">Hop {i + 1}</span>
                <button onclick={() => removeHop(i)} class="text-xs text-red-400 hover:text-red-300">&times;</button>
              </div>
              <div class="grid grid-cols-2 md:grid-cols-4 gap-2">
                <input type="text" bind:value={hop.host} placeholder="10.0.0.1"
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
                <input type="number" bind:value={hop.port} min="1" max="65535"
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
                <input type="text" bind:value={hop.username} placeholder="username"
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
                <select bind:value={hop.auth_type}
                  class="rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500">
                  <option value="key">SSH Key</option>
                  <option value="password">Password</option>
                </select>
              </div>
              {#if hop.auth_type === 'password'}
                <input type="password" bind:value={hop.credential} placeholder="Password"
                  class="w-full rounded bg-gray-900 border border-gray-700 text-gray-200 px-2 py-1.5 text-sm focus:outline-none focus:border-purple-500" />
              {/if}
            </div>
          {/each}
        </div>

        <div class="flex justify-end">
          <button
            onclick={handleSave}
            disabled={saving || !formName.trim() || formHops.length === 0}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 text-white transition-colors"
          >
            {saving ? 'Saving...' : editing ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    {/if}

    {#if configList.length === 0 && !showForm}
      <div class="rounded-xl bg-gray-900 border border-gray-800 p-8 text-center text-gray-500">
        No proxy configurations yet.
      </div>
    {:else}
      <div class="space-y-3">
        {#each configList as pc (pc.id)}
          <div class="rounded-xl bg-gray-900 border border-gray-800 p-4 space-y-3">
            <div class="flex items-start justify-between">
              <div class="flex items-center gap-2">
                <h3 class="text-sm font-semibold text-white">{pc.name}</h3>
                <span class="text-[10px] font-medium px-2 py-0.5 rounded-full {pc.is_active ? 'bg-green-600 text-green-100' : 'bg-gray-600 text-gray-100'}">
                  {pc.is_active ? 'Active' : 'Inactive'}
                </span>
                <span class="text-xs text-gray-500">{pc.hops.length} hop{pc.hops.length !== 1 ? 's' : ''}</span>
              </div>
              <div class="flex items-center gap-2">
                <button
                  onclick={() => handleToggle(pc)}
                  class="px-2 py-1 text-xs rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
                >
                  {pc.is_active ? 'Disable' : 'Enable'}
                </button>
                <button
                  onclick={() => handleTest(pc.id)}
                  disabled={testing === pc.id}
                  class="px-2 py-1 text-xs rounded-lg bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 transition-colors"
                >
                  {testing === pc.id ? 'Testing...' : 'Test'}
                </button>
                <button
                  onclick={() => startEdit(pc)}
                  class="px-2 py-1 text-xs rounded-lg bg-gray-700 text-gray-300 hover:bg-gray-600 transition-colors"
                >
                  Edit
                </button>
                <button
                  onclick={() => handleDelete(pc.id)}
                  class="px-2 py-1 text-xs rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 transition-colors"
                >
                  Delete
                </button>
              </div>
            </div>

            <div class="flex items-center gap-1 text-xs text-gray-500 font-mono">
              {#each pc.hops as hop, i}
                {#if i > 0}<span class="text-gray-600">&rarr;</span>{/if}
                <span>{hop.username}@{hop.host}:{hop.port}</span>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</SettingsLayout>
