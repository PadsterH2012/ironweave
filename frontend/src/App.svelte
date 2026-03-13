<script lang="ts">
  import Router, { link, push, location } from 'svelte-spa-router';
  import Dashboard from './routes/Dashboard.svelte';
  import Projects from './routes/Projects.svelte';
  import ProjectDetail from './routes/ProjectDetail.svelte';
  import WorkflowView from './routes/WorkflowView.svelte';
  import Mounts from './routes/Mounts.svelte';
  import Agents from './routes/Agents.svelte';
  import SettingsGeneral from './routes/SettingsGeneral.svelte';
  import SettingsProxies from './routes/SettingsProxies.svelte';
  import SettingsApiKeys from './routes/SettingsApiKeys.svelte';
  import Login from './routes/Login.svelte';
  import { auth } from './lib/api';

  const routes = {
    '/login': Login,
    '/': Dashboard,
    '/projects': Projects,
    '/projects/:id': ProjectDetail,
    '/projects/:id/workflows/:wid': WorkflowView,
    '/mounts': Mounts,
    '/agents': Agents,
    '/settings': SettingsGeneral,
    '/settings/general': SettingsGeneral,
    '/settings/proxies': SettingsProxies,
    '/settings/api-keys': SettingsApiKeys,
  };

  let backendStatus = $state('checking...');
  let authEnabled = $state<boolean | null>(null);

  async function checkHealth() {
    try {
      const res = await fetch('/api/health');
      backendStatus = res.ok ? 'connected' : 'error';
    } catch {
      backendStatus = 'unreachable';
    }
  }

  async function checkAuth() {
    // Try fetching a protected endpoint to see if auth is enabled
    try {
      const res = await fetch('/api/dashboard');
      if (res.status === 401) {
        authEnabled = true;
        if (!auth.isAuthenticated()) {
          push('/login');
        }
      } else {
        authEnabled = false;
      }
    } catch {
      authEnabled = false;
    }
  }

  $effect(() => {
    checkHealth();
    checkAuth();
  });

  function handleLogout() {
    auth.logout();
  }

  const navItems = [
    { href: '/', label: 'Dashboard' },
    { href: '/projects', label: 'Projects' },
    { href: '/mounts', label: 'Mounts' },
    { href: '/agents', label: 'Agents' },
    { href: '/settings', label: 'Settings' },
  ];

  let currentLocation = $state('');

  // Track location reactively
  $effect(() => {
    const unsubscribe = location.subscribe((val) => {
      currentLocation = val ?? '';
    });
    return unsubscribe;
  });

  let isLoginPage = $derived(currentLocation === '/login');
</script>

{#if isLoginPage}
  <Router {routes} />
{:else}
  <div class="flex h-screen bg-gray-950 text-gray-100">
    <!-- Sidebar -->
    <aside class="w-60 shrink-0 bg-gray-900 border-r border-gray-800 flex flex-col">
      <div class="p-4 border-b border-gray-800">
        <h1 class="text-lg font-bold tracking-tight">Ironweave</h1>
      </div>
      <nav class="flex-1 p-2 space-y-1">
        {#each navItems as item}
          <a
            href={item.href}
            use:link
            class="block px-3 py-2 rounded text-sm text-gray-300 hover:bg-gray-800 hover:text-white transition-colors"
          >
            {item.label}
          </a>
        {/each}
      </nav>
      <div class="p-4 border-t border-gray-800 text-xs text-gray-500 space-y-2">
        <div>
          Backend:
          <span class={backendStatus === 'connected' ? 'text-green-400' : 'text-red-400'}>
            {backendStatus}
          </span>
        </div>
        {#if authEnabled && auth.isAuthenticated()}
          <button
            onclick={handleLogout}
            class="text-gray-400 hover:text-white text-xs transition-colors"
          >
            Sign out
          </button>
        {/if}
      </div>
    </aside>

    <!-- Main content -->
    <main class="flex-1 overflow-auto p-6">
      <Router {routes} />
    </main>
  </div>
{/if}
