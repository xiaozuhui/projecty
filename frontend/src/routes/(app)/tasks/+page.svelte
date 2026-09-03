<script lang="ts">
  import Avatar from '$lib/components/Avatar.svelte';
  import PriorityPill from '$lib/components/PriorityPill.svelte';
  import TaskTypePill from '$lib/components/TaskTypePill.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { listMyTasks } from '$lib/api/tasks';
  import type { TaskListItem } from '$lib/api/types';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';
  import { page as appPage } from '$app/state';

  type Scope = 'assignee' | 'reporter' | 'reviewer' | 'all';

  const scopes: { value: Scope; label: string }[] = [
    { value: 'assignee', label: '我负责的' },
    { value: 'reporter', label: '我创建的' },
    { value: 'reviewer', label: '我评审的' },
    { value: 'all', label: '全部' }
  ];
  const scopeLabel: Record<Scope, string> = {
    assignee: '我负责的',
    reporter: '我创建的',
    reviewer: '我评审的',
    all: '可见项目全部'
  };

  // 初始化读 URL 参数:工作台「关注」面板等入口可带参直达(?scope=&overdue=1&due_soon=1&keyword=)。
  function initialScope(value: string | null): Scope {
    return value === 'reporter' || value === 'all' || value === 'reviewer' ? value : 'assignee';
  }

  let scope = $state<Scope>(initialScope(appPage.url.searchParams.get('scope')));
  let keyword = $state(appPage.url.searchParams.get('keyword') ?? '');
  let overdueOnly = $state(appPage.url.searchParams.get('overdue') === '1');
  let dueSoonOnly = $state(appPage.url.searchParams.get('due_soon') === '1');
  let items = $state<TaskListItem[]>([]);
  let total = $state(0);
  let page = $state(1);
  let hasMore = $state(false);
  let loading = $state(true);
  let appending = $state(false);
  let errorMessage = $state('');

  const isOverdue = (task: TaskListItem) => Boolean(task.due_at && task.status_category !== 'done' && new Date(task.due_at) < new Date());

  const groups = $derived.by(() => {
    const map = new Map<string, { projectKey: string; projectName: string; tasks: TaskListItem[] }>();
    for (const item of items) {
      const group = map.get(item.project_key) ?? {
        projectKey: item.project_key,
        projectName: item.project_name || item.project_key,
        tasks: []
      };
      group.tasks.push(item);
      map.set(item.project_key, group);
    }
    return [...map.values()];
  });

  async function load(targetPage = 1, append = false) {
    if (append) appending = true;
    else {
      loading = true;
      errorMessage = '';
    }
    try {
      const response = await listMyTasks(scope, targetPage, 30, {
        keyword: keyword.trim() || undefined,
        overdue: overdueOnly,
        dueSoon: dueSoonOnly
      });
      items = append ? [...items, ...response.data.items] : response.data.items;
      total = response.data.total;
      page = response.data.page;
      hasMore = response.data.has_more;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务加载失败';
    } finally {
      loading = false;
      appending = false;
    }
  }

  function switchScope(next: Scope) {
    if (next === scope) return;
    scope = next;
    items = [];
    void load(1);
  }

  function applySearch(event: SubmitEvent) {
    event.preventDefault();
    items = [];
    void load(1);
  }

  function toggleOverdue() {
    overdueOnly = !overdueOnly;
    if (overdueOnly) dueSoonOnly = false;
    items = [];
    void load(1);
  }

  function toggleDueSoon() {
    dueSoonOnly = !dueSoonOnly;
    if (dueSoonOnly) overdueOnly = false;
    items = [];
    void load(1);
  }

  bindReload(() => void load(1));
</script>

<div class="page-head">
  <h1>任务</h1>
  <div class="meta-row">
    <span class="meta-item">范围:<b>{scopeLabel[scope]}</b></span><span class="sep">·</span>
    <span class="meta-item">共 {total} 项</span>
    {#if overdueOnly}<span class="sep">·</span><span class="meta-item danger">仅看逾期</span>{/if}
    {#if dueSoonOnly}<span class="sep">·</span><span class="meta-item warn">7 天内到期</span>{/if}
  </div>
</div>

<div class="toolbar">
  <div class="segmented" role="tablist" aria-label="任务范围">
    {#each scopes as item}
      <button
        class:active={scope === item.value}
        role="tab"
        aria-selected={scope === item.value}
        type="button"
        onclick={() => switchScope(item.value)}
      >
        {item.label}
      </button>
    {/each}
  </div>
  <span class="flex-fill"></span>
  <button class="filter-chip" class:active={overdueOnly} type="button" onclick={toggleOverdue}>仅看逾期</button>
  <button class="filter-chip" class:active={dueSoonOnly} type="button" onclick={toggleDueSoon}>7 天内到期</button>
  <form onsubmit={applySearch}>
    <input class="search-input" bind:value={keyword} placeholder="搜索任务标题或编号" aria-label="搜索任务" />
  </form>
</div>

{#if errorMessage}
  <div class="error-message" role="alert">{errorMessage}</div>
{/if}

{#if loading}
  <div class="state-box">正在加载任务…</div>
{:else if !groups.length}
  <div class="empty-panel">
    <strong>没有任务</strong>
    <p>切换范围或筛选,或到项目看板创建第一个任务。</p>
    <a class="secondary-button" href="/projects">去项目列表</a>
  </div>
{:else}
  <section class="list-panel">
    {#each groups as group (group.projectKey)}
      <div class="group-bar">
        <code>{group.projectKey}</code>
        <span>{group.projectName}</span>
        <span class="group-count">{group.tasks.length} 项</span>
      </div>
      {#each group.tasks as task (task.id)}
        <a class="task-row" href={`/tasks/${task.task_key}`}>
          <span class="task-key">{task.task_key}</span>
          <span class="task-title">{task.title}</span>
          <StatusBadge name={task.status_name} category={task.status_category} />
          <span class="col-type"><TaskTypePill taskType={task.task_type} /></span>
          <span class="col-priority"><PriorityPill priority={task.priority} /></span>
          <span class="col-assignee">
            {#if task.assignee_name}<Avatar name={task.assignee_name} size={18} />{:else}<span class="unassigned">未分配</span>{/if}
          </span>
          <span class="due" class:danger={isOverdue(task)}>
            {#if task.due_at}{new Date(task.due_at).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })}{:else}—{/if}
          </span>
        </a>
      {/each}
    {/each}
  </section>
  {#if hasMore}
    <div class="pager">
      <button class="secondary-button" type="button" disabled={appending} onclick={() => void load(page + 1, true)}>
        {appending ? '加载中…' : '加载更多'}
      </button>
    </div>
  {/if}
{/if}

<style>
  .page-head { margin-bottom: 18px; }
  .page-head h1 { margin: 0; font-size: 22px; font-weight: 600; line-height: 1.35; }
  .meta-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; margin-top: 8px; font-size: 13px; color: var(--color-text-muted); }
  .meta-item { display: inline-flex; align-items: center; gap: 6px; }
  .meta-item b { color: var(--color-text-secondary); font-weight: 500; }
  .meta-item.danger { color: var(--color-danger); }
  .meta-item.warn { color: var(--color-warning); }
  .sep { color: var(--color-border); }

  .toolbar { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-bottom: 16px; }
  .flex-fill { flex: 1; }
  .segmented { display: inline-flex; gap: 2px; padding: 2px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .segmented button { padding: 5px 12px; border-radius: calc(var(--radius-md) - 2px); background: transparent; color: var(--color-text-muted); font-size: 12px; cursor: pointer; transition: background-color var(--transition-fast), color var(--transition-fast); }
  .segmented button:hover { color: var(--color-text-secondary); }
  .segmented button.active { background: var(--color-surface-raised); color: var(--color-text); font-weight: 500; box-shadow: 0 0 0 1px var(--color-border-weak); }

  .filter-chip { display: inline-flex; align-items: center; gap: 5px; padding: 5px 12px; border: 1px solid var(--color-border); border-radius: 999px; background: transparent; color: var(--color-text-muted); font-size: 12px; cursor: pointer; transition: color var(--transition-fast), border-color var(--transition-fast), background-color var(--transition-fast); }
  .filter-chip:hover { color: var(--color-text-secondary); border-color: var(--color-border-strong); }
  .filter-chip.active { background: var(--color-primary-soft); border-color: var(--color-primary); color: var(--color-primary-strong); font-weight: 500; }

  .search-input { width: 200px; padding: 6px 10px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface); color: var(--color-text); font-size: 13px; }
  .search-input:focus-visible { outline: none; border-color: var(--color-primary); box-shadow: var(--color-focus-ring); }

  .list-panel { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-lg); overflow: hidden; }
  .group-bar { display: flex; align-items: center; gap: 8px; padding: 10px 14px 6px; font-size: 12px; color: var(--color-text-muted); }
  .group-bar code { color: var(--color-primary-strong); font-family: var(--font-mono); }
  .group-bar + .task-row { border-top: 1px solid var(--color-border-weak); }
  .group-count { margin-left: auto; font-family: var(--font-mono); font-size: 11px; color: var(--color-text-muted); }

  .task-row {
    display: grid;
    grid-template-columns: 104px minmax(0, 1fr) auto auto auto minmax(60px, auto) 56px;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    color: var(--color-text);
    text-decoration: none;
    font-size: 13px;
    transition: background-color var(--transition-fast);
  }
  .task-row:hover { background: var(--color-hover); }
  .task-key { color: var(--color-text-muted); font-family: var(--font-mono); font-size: 12px; }
  .task-row:hover .task-key { color: var(--color-primary-strong); }
  .task-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  .col-type, .col-priority, .col-assignee { display: inline-flex; align-items: center; justify-content: center; }
  .col-assignee { color: var(--color-text-muted); }
  .unassigned { font-size: 12px; color: var(--color-text-muted); }
  .due { text-align: right; font-size: 12px; color: var(--color-text-muted); font-family: var(--font-mono); }
  .due.danger { color: var(--color-danger); font-weight: 500; }

  .error-message { margin-bottom: 14px; padding: 8px 12px; border: 1px solid var(--color-danger); border-radius: var(--radius-md); color: var(--color-danger); font-size: 13px; }

  .state-box { display: grid; place-items: center; min-height: 220px; color: var(--color-text-muted); }
  .empty-panel { display: grid; place-items: center; gap: 8px; min-height: 220px; padding: 24px; border: 1px solid var(--color-border); border-radius: var(--radius-lg); color: var(--color-text-muted); }
  .empty-panel strong { color: var(--color-text-secondary); font-size: 14px; font-weight: 500; }
  .empty-panel p { font-size: 13px; }
  .empty-panel a { margin-top: 6px; }

  .pager { display: flex; justify-content: center; padding: 14px 0; }

  @media (max-width: 900px) {
    .toolbar { align-items: stretch; flex-direction: column; }
    .search-input { width: 100%; }
  }
  @media (max-width: 760px) {
    .task-row { grid-template-columns: minmax(0, 1fr) auto; row-gap: 6px; }
    .task-key, .col-type, .col-priority, .due { display: none; }
  }
</style>
