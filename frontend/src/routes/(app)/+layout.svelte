<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import AppShell from '$lib/components/app-shell/AppShell.svelte';
  import { me } from '$lib/api/auth';
  import { ApiClientError } from '$lib/api/client';
  import { session } from '$lib/features/auth/session.svelte';
  import DialogHost from '$lib/features/ui/DialogHost.svelte';
  import type { MeResponse } from '$lib/api/types';

  let { children } = $props();
  let ready = $state(false);
  let user = $state<MeResponse | null>(null);
  let errorMessage = $state('');

  onMount(async () => {
    if (!session.accessToken) {
      await goto('/login');
      return;
    }
    try {
      user = (await me()).data;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '登录状态验证失败';
      session.clear();
      await goto('/login');
      return;
    } finally {
      ready = true;
    }
  });
</script>

{#if !ready}
  <main class="session-loading"><div class="loading-card"><span class="loading-dot"></span><strong>正在验证登录状态</strong><p>正在连接 Projecty 服务…</p></div></main>
{:else if user}
  <AppShell {user}>{@render children()}</AppShell>
  <DialogHost />
{:else if errorMessage}
  <main class="session-loading"><div class="loading-card"><strong>{errorMessage}</strong><a class="primary-button" href="/login">返回登录</a></div></main>
{/if}

<style>
  .session-loading { display: grid; place-items: center; min-height: 100vh; padding: 24px; background: var(--color-bg); }
  .loading-card { display: grid; gap: 10px; width: min(100%, 360px); padding: 28px; text-align: center; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-lg); }
  .loading-card p { margin: 0; color: var(--color-text-muted); }
  .loading-card .primary-button { margin: 8px auto 0; }
  .loading-dot { width: 12px; height: 12px; margin: 0 auto 2px; border-radius: 50%; background: var(--color-primary); box-shadow: 0 0 0 7px var(--color-primary-soft); animation: pulse 1.2s ease-in-out infinite; }
  @keyframes pulse { 50% { transform: scale(.75); opacity: .55; } }
</style>
