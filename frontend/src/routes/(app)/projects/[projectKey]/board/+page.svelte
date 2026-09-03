<script lang="ts">
  import { page } from '$app/state';
  import Board from '$lib/features/board/Board.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';

  const projectKey = $derived(String(page.params.projectKey ?? ''));
  let board = $state<Board | undefined>(undefined);
  const stats = $derived(
    board?.getStats() ?? { total: 0, inProgress: 0, done: 0, overdue: 0 }
  );

  const views = [
    { path: 'board', label: '看板' },
    { path: 'list', label: '列表' },
    { path: 'timeline', label: '时间线' }
  ];

  bindReload(() => void board?.reload());
</script>

<header class="page-head">
  <nav class="breadcrumb" aria-label="项目路径">
    <a href="/projects">项目</a><span>/</span>
    <a href={`/projects/${projectKey}`}>{projectKey}</a><span>/</span>
    <span>看板</span>
  </nav>
  <h1>看板</h1>
  <div class="meta-row">
    <span class="meta-item">{stats.total} 项任务</span><span class="sep">·</span>
    <span class="meta-item">{stats.inProgress} 项进行中</span><span class="sep">·</span>
    <span class="meta-item">{stats.done} 项已完成</span><span class="sep">·</span>
    <span class="meta-item danger">{stats.overdue} 项逾期</span>
  </div>
  <div class="segmented" role="tablist" aria-label="项目视图">
    {#each views as view (view.path)}
      <a role="tab" aria-selected={view.path === 'board'} href={`/projects/${projectKey}/${view.path}`}>{view.label}</a>
    {/each}
  </div>
</header>

<Board bind:this={board} {projectKey} />

<style>
  h1 { margin: 0; }
  .page-head { margin-bottom: 18px; display: grid; gap: 8px; }
  .page-head h1 { font-size: 22px; font-weight: 600; line-height: 1.35; }
  .breadcrumb { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--color-text-muted); }
  .breadcrumb a { color: var(--color-text-muted); }
  .breadcrumb a:hover { color: var(--color-text); text-decoration: none; }
  .meta-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; font-size: 13px; color: var(--color-text-muted); }
  .meta-item.danger { color: var(--color-danger); }
  .sep { color: var(--color-border); }
  .segmented { display: inline-flex; gap: 2px; padding: 2px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-sunken); width: fit-content; }
  .segmented a { padding: 4px 12px; border-radius: calc(var(--radius-md) - 2px); color: var(--color-text-muted); font-size: 12px; text-decoration: none; transition: background-color var(--transition-fast), color var(--transition-fast); }
  .segmented a:hover { color: var(--color-text-secondary); }
  .segmented a[aria-selected='true'] { background: var(--color-surface-raised); color: var(--color-text); font-weight: 500; box-shadow: 0 0 0 1px var(--color-border-weak); }
</style>
