<script lang="ts">
  import { push } from 'svelte-spa-router';
  import { auth } from '../lib/api';

  let username = $state('');
  let password = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleLogin(e: Event) {
    e.preventDefault();
    error = '';
    loading = true;
    try {
      await auth.login(username, password);
      push('/');
    } catch (err: any) {
      error = err.message || 'Login failed';
    } finally {
      loading = false;
    }
  }
</script>

<div class="min-h-screen bg-gray-950 flex items-center justify-center">
  <form onsubmit={handleLogin} class="bg-gray-900 border border-gray-800 rounded-lg p-8 w-full max-w-sm space-y-6">
    <div class="text-center">
      <h1 class="text-2xl font-bold text-gray-100 tracking-tight">Ironweave</h1>
      <p class="text-sm text-gray-500 mt-1">Sign in to continue</p>
    </div>

    {#if error}
      <div class="bg-red-900/30 border border-red-800 text-red-300 text-sm rounded px-3 py-2">
        {error}
      </div>
    {/if}

    <div class="space-y-4">
      <div>
        <label for="username" class="block text-sm font-medium text-gray-400 mb-1">Username</label>
        <input
          id="username"
          type="text"
          bind:value={username}
          class="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-gray-100 text-sm focus:outline-none focus:border-blue-500"
          required
        />
      </div>
      <div>
        <label for="password" class="block text-sm font-medium text-gray-400 mb-1">Password</label>
        <input
          id="password"
          type="password"
          bind:value={password}
          class="w-full px-3 py-2 bg-gray-800 border border-gray-700 rounded text-gray-100 text-sm focus:outline-none focus:border-blue-500"
          required
        />
      </div>
    </div>

    <button
      type="submit"
      disabled={loading}
      class="w-full py-2 px-4 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white text-sm font-medium rounded transition-colors"
    >
      {loading ? 'Signing in...' : 'Sign in'}
    </button>
  </form>
</div>
