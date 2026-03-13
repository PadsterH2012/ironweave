<script lang="ts">
  import { push, location } from 'svelte-spa-router';
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();

  const sections = [
    { key: 'general', label: 'General', href: '/settings/general' },
    { key: 'proxies', label: 'Proxies', href: '/settings/proxies' },
    { key: 'api-keys', label: 'API Keys', href: '/settings/api-keys' },
  ];

  let currentLocation = $state('');

  $effect(() => {
    const unsubscribe = location.subscribe((val) => {
      currentLocation = val ?? '';
    });
    return unsubscribe;
  });

  function isActive(href: string): boolean {
    return currentLocation === href;
  }
</script>

<div class="flex gap-6">
  <!-- Settings sidebar -->
  <nav class="w-48 shrink-0 space-y-1">
    <h1 class="text-2xl font-bold text-white mb-4">Settings</h1>
    {#each sections as section}
      <button
        onclick={() => push(section.href)}
        class="block w-full text-left px-3 py-2 rounded-lg text-sm transition-colors {isActive(section.href)
          ? 'bg-purple-600/20 text-purple-400 font-medium'
          : 'text-gray-400 hover:bg-gray-800 hover:text-gray-200'}"
      >
        {section.label}
      </button>
    {/each}
  </nav>

  <!-- Content area -->
  <div class="flex-1 min-w-0">
    {@render children()}
  </div>
</div>
