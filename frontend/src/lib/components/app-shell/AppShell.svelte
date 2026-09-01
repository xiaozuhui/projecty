<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Icon from '$lib/components/icons/Icon.svelte';
  import type { IconName } from '$lib/components/icons/Icon.svelte';
  import { logout } from '$lib/api/auth';
  import { session } from '$lib/features/auth/session.svelte';
  import { initTheme, theme, toggleTheme } from '$lib/features/ui/theme.svelte';
  import type { MeResponse } from '$lib/api/types';

  let { children, user } = $props<{ children: import('svelte').Snippet; user: MeResponse }>();
  let loggingOut = $state(false);

  onMount(initTheme);

  const navGroups = $derived([
    {
      title: '工作台',
      links: [
        { href: '/', label: '总览', icon: 'home' },
        { href: '/tasks', label: '任务', icon: 'tasks' },
        { href: '/projects', label: '项目', icon: 'projects' },
        { href: '/departments', label: '部门', icon: 'departments' },
        { href: '/search', label: '搜索', icon: 'search' },
      ] as { href: string; label: string; icon: IconName }[],
    },
    {
      title: '系统',
      links: [
        ...(user.system_role === 'super_admin' ? [{ href: '/users', label: '用户', icon: 'users' }] : []),
        { href: '/notifications', label: '通知', icon: 'bell' },
        { href: '/settings/profile', label: '个人设置', icon: 'settings' },
        { href: '/settings/system', label: '系统管理', icon: 'settings' },
      ] as { href: string; label: string; icon: IconName }[],
    },
  ]);

  const isActive = (href: string) =>
    href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);

  // 顶栏位置指示:复用侧栏导航定义,避免再维护一份路由→名称映射。
  const currentLocation = $derived.by(() => {
    for (const group of navGroups) {
      const link = group.links.find((item) => isActive(item.href));
      if (link) return `${group.title} · ${link.label}`;
    }
    return '';
  });

  async function signOut() {
    if (loggingOut) return;
    loggingOut = true;
    const refreshToken = session.refreshToken;
    try { if (refreshToken) await logout(refreshToken); } catch { /* 本地清理仍然必须执行 */ }
    session.clear();
    await goto('/login');
  }
</script>

<div class="app-shell">
  <aside class="sidebar">
    <a class="logo" href="/" aria-label="Projecty 首页"><span class="logo-mark">P</span><span>Projecty</span></a>
    <nav class="sidebar-nav" aria-label="主导航">
      {#each navGroups as group}
        <section>
          <div class="nav-section">{group.title}</div>
          {#each group.links as link}
            <a class="nav-item" href={link.href} class:active={isActive(link.href)} aria-current={isActive(link.href) ? 'page' : undefined}>
              <Icon name={link.icon} />
              <span>{link.label}</span>
            </a>
          {/each}
        </section>
      {/each}
    </nav>
    <div class="sidebar-footer">
      <a class="user-card" href="/settings/profile">
        <span class="user-avatar">{user.display_name.slice(0, 1)}</span>
        <span class="user-info"><strong>{user.display_name}</strong><small>{user.account}</small></span>
      </a>
      <button class="ghost-button logout-button" type="button" onclick={signOut} disabled={loggingOut}>
        <Icon name="logout" />{loggingOut ? '退出中…' : '退出登录'}
      </button>
    </div>
  </aside>
  <main class="content-shell">
    <div class="topbar">
      <div><strong>内部项目管理</strong>{#if currentLocation}<span>{currentLocation}</span>{/if}</div>
      <div class="topbar-actions">
        <button class="icon-button" type="button" onclick={toggleTheme} aria-label={theme.current === 'dark' ? '切换到浅色主题' : '切换到暗色主题'} title={theme.current === 'dark' ? '切换到浅色主题' : '切换到暗色主题'}>
          <Icon name={theme.current === 'dark' ? 'sun' : 'moon'} size={15} />
        </button>
      </div>
    </div>
    <div class="content-scroll">{@render children()}</div>
  </main>
</div>

<style>
  .app-shell { display: flex; min-height: 100vh; }

  .sidebar { position: sticky; top: 0; display: flex; flex: 0 0 var(--sidebar-width); flex-direction: column; height: 100vh; padding: 16px 12px; background: var(--color-surface-sunken); border-right: 1px solid var(--color-border); }
  .logo { display: flex; align-items: center; gap: 8px; padding: 4px 8px 20px; margin-bottom: 12px; border-bottom: 1px solid var(--color-border-weak); font-size: 14px; font-weight: 600; letter-spacing: -0.01em; color: var(--color-text); }
  .logo-mark { display: inline-grid; place-items: center; width: 24px; height: 24px; border-radius: var(--radius-md); background: var(--color-primary-soft); color: var(--color-primary-strong); font-size: 13px; font-weight: 600; }

  .nav-section { margin: 16px 0 6px 8px; color: var(--color-text-muted); font-size: 11px; letter-spacing: 0.06em; }
  .nav-item { display: flex; align-items: center; gap: 8px; margin-bottom: 1px; padding: 6px 8px; border-radius: var(--radius-md); color: var(--color-text-secondary); font-size: 13px; transition: background-color var(--transition-fast), color var(--transition-fast); }
  .nav-item:hover { color: var(--color-text); background: var(--color-hover); }
  .nav-item.active { color: var(--color-text); background: var(--color-hover); }
  .nav-item :global(svg) { flex: none; color: var(--color-text-muted); }
  .nav-item:hover :global(svg), .nav-item.active :global(svg) { color: currentColor; }

  .sidebar-footer { display: grid; gap: 4px; margin-top: auto; padding-top: 12px; border-top: 1px solid var(--color-border-weak); }
  .user-card { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: var(--radius-md); transition: background-color var(--transition-fast); }
  .user-card:hover { background: var(--color-hover); }
  .user-avatar { display: inline-grid; place-items: center; width: 28px; height: 28px; flex: none; border-radius: 50%; border: 1px solid var(--color-border); background: var(--color-hover); color: var(--color-text-secondary); font-size: 12px; font-weight: 500; }
  .user-info { display: grid; min-width: 0; }
  .user-info strong { font-size: 13px; font-weight: 500; }
  .user-info strong, .user-info small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .user-info small { color: var(--color-text-muted); font-size: 11px; }
  .logout-button { justify-content: flex-start; width: 100%; }

  .content-shell { min-width: 0; flex: 1; display: flex; flex-direction: column; }
  .topbar { position: sticky; top: 0; z-index: 5; display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 56px; padding: 10px 28px; background: color-mix(in srgb, var(--color-bg) 86%, transparent); backdrop-filter: blur(12px); border-bottom: 1px solid var(--color-border); }
  .topbar div:first-child { display: grid; gap: 2px; }
  .topbar strong { font-size: 13px; font-weight: 500; }
  .topbar span { color: var(--color-text-muted); font-size: 12px; }
  .topbar-actions { display: flex; align-items: center; gap: 8px; }
  .icon-button { display: inline-grid; place-items: center; width: 30px; height: 30px; padding: 0; border: 0; border-radius: var(--radius-md); background: transparent; color: var(--color-text-muted); cursor: pointer; transition: background-color var(--transition-fast), color var(--transition-fast), transform var(--transition-fast); }
  .icon-button:hover { background: var(--color-hover); color: var(--color-text); }
  .icon-button:active { transform: scale(0.97); }

  .content-scroll { width: min(100%, 1480px); margin: 0 auto; padding: 24px 28px 44px; }

  @media (max-width: 1024px) {
    .sidebar { flex-basis: 64px; padding-inline: 10px; }
    .logo { justify-content: center; padding-bottom: 16px; }
    .logo span:last-child, .nav-item span, .nav-section, .user-info, .logout-button { display: none; }
    .nav-item { justify-content: center; }
    .logout-button { display: none; }
    .user-card { justify-content: center; }
  }
  @media (max-width: 768px) {
    .app-shell { display: block; }
    .sidebar { position: static; flex-direction: row; align-items: center; width: 100%; height: auto; overflow-x: auto; border-right: 0; border-bottom: 1px solid var(--color-border); }
    .sidebar-nav { display: flex; gap: 4px; }
    .sidebar-nav section { display: contents; }
    .nav-section { display: none; }
    .sidebar-footer { display: none; }
    .topbar { padding-inline: 16px; }
    .content-scroll { padding: 18px 16px 36px; }
  }
</style>
