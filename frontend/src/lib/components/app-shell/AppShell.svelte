<script lang="ts">
  import { goto } from '$app/navigation';
  import { logout } from '$lib/api/auth';
  import { session } from '$lib/features/auth/session.svelte';
  import type { MeResponse } from '$lib/api/types';

  let { children, user } = $props<{ children: import('svelte').Snippet; user: MeResponse }>();
  let loggingOut = $state(false);

  const navGroups = $derived([
    { title: '工作台', links: [{ href: '/', label: '总览', mark: '概' }, { href: '/projects', label: '项目', mark: '项' }, { href: '/departments', label: '部门', mark: '部' }, { href: '/search', label: '搜索', mark: '搜' }] },
    { title: '系统', links: [ ...(user.system_role === 'super_admin' ? [{ href: '/users', label: '用户', mark: '员' }] : []), { href: '/notifications', label: '通知', mark: '通' }, { href: '/settings/profile', label: '个人设置', mark: '设' }, { href: '/settings/system', label: '系统管理', mark: '管' }] }
  ]);

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
      {#each navGroups as group}<section><div class="nav-section">{group.title}</div>{#each group.links as link}<a class="nav-item" href={link.href}><span class="nav-mark">{link.mark}</span><span>{link.label}</span></a>{/each}</section>{/each}
    </nav>
    <div class="sidebar-footer"><a class="user-card" href="/settings/profile"><span class="avatar">{user.display_name.slice(0, 1)}</span><span class="user-info"><strong>{user.display_name}</strong><small>{user.account} · {user.system_role}</small></span></a><button class="logout-button" type="button" onclick={signOut} disabled={loggingOut}>{loggingOut ? '退出中…' : '退出登录'}</button></div>
  </aside>
  <main class="content-shell">
    <div class="topbar"><div><strong>内部项目管理</strong><span>项目 / 任务 / 子任务 / 日志</span></div><a class="primary-button" href="/projects/new">新建项目</a></div>
    <div class="content-scroll">{@render children()}</div>
  </main>
</div>
<style>
  .app-shell{display:flex;min-height:100vh}.sidebar{position:sticky;top:0;display:flex;flex:0 0 var(--sidebar-width);flex-direction:column;height:100vh;padding:24px 16px;background:var(--color-surface);border-right:1px solid var(--color-border);box-shadow:2px 0 12px rgba(0,0,0,.02)}
  .logo{display:flex;align-items:center;gap:10px;padding-bottom:24px;margin-bottom:18px;border-bottom:1px solid #f0f2f6;font-size:22px;font-weight:800}.logo-mark,.avatar{display:inline-grid;place-items:center;color:#fff;background:linear-gradient(135deg,#7c5cfc,#4f7df3)}.logo-mark{width:34px;height:34px;border-radius:12px}
  .nav-section{margin:18px 0 8px 8px;color:var(--color-text-muted);font-size:11px;font-weight:700;letter-spacing:.08em}.nav-item{display:flex;align-items:center;gap:12px;margin-bottom:4px;padding:10px 12px;border-radius:10px;color:var(--color-text-secondary);font-weight:650}.nav-item:hover{color:var(--color-primary);background:var(--color-primary-soft)}.nav-mark{display:inline-grid;place-items:center;width:24px;height:24px;border-radius:8px;color:var(--color-primary);background:#f0f3fa;font-size:12px}
  .sidebar-footer{display:grid;gap:8px;margin-top:auto;padding-top:16px;border-top:1px solid #f0f2f6}.user-card{display:flex;align-items:center;gap:12px;padding:8px;border-radius:12px}.user-card:hover{background:#f0f3fa}.avatar{width:38px;height:38px;border-radius:50%;font-weight:700}.user-info{display:grid;min-width:0}.user-info strong,.user-info small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.user-info small{color:var(--color-text-muted)}.logout-button{padding:8px;border:0;border-radius:8px;color:var(--color-text-muted);background:transparent;text-align:left;cursor:pointer}.logout-button:hover{color:var(--color-danger);background:#fff1f2}.logout-button:disabled{opacity:.6;cursor:wait}
  .content-shell{min-width:0;flex:1;display:flex;flex-direction:column}.topbar{position:sticky;top:0;z-index:5;display:flex;align-items:center;justify-content:space-between;gap:16px;min-height:72px;padding:16px 28px;background:rgba(244,246,249,.86);backdrop-filter:blur(18px);border-bottom:1px solid rgba(230,234,240,.8)}.topbar div{display:grid;gap:3px}.topbar span{color:var(--color-text-muted);font-size:13px}.content-scroll{width:min(100%,1480px);margin:0 auto;padding:24px 28px 44px}
  @media(max-width:1024px){.sidebar{flex-basis:82px;padding-inline:12px}.logo span:last-child,.nav-item span:last-child,.nav-section,.user-info,.logout-button{display:none}.logo,.nav-item{justify-content:center}}
  @media(max-width:768px){.app-shell{display:block}.sidebar{position:static;flex-direction:row;align-items:center;width:100%;height:auto;overflow-x:auto;border-right:0;border-bottom:1px solid var(--color-border)}.sidebar-nav{display:flex;gap:8px}.sidebar-nav section{display:contents}.sidebar-footer{display:none}.topbar{padding-inline:16px}.content-scroll{padding:18px 16px 36px}}
</style>
