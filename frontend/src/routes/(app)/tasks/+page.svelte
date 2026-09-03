<script lang="ts">
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import PriorityPill from '$lib/components/PriorityPill.svelte';
  import TaskTypePill from '$lib/components/TaskTypePill.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { deleteTask, listMyTasks } from '$lib/api/tasks';
  import type { TaskListItem } from '$lib/api/types';
  import { confirmDialog } from '$lib/features/ui/dialog.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';
  import { page as appPage } from '$app/state';

  type Scope = 'assignee' | 'reporter' | 'reviewer' | 'all';

  const scopes: { value: Scope; label: string }[] = [
    { value: 'assignee', label: '我负责的' },
    { value: 'reporter', label: '我创建的' },
    { value: 'reviewer', label: '我评审的' },
    { value: 'all', label: '全部' }
  ];

  // 初始化读 URL 参数:工作台横幅等入口可带参直达(?scope=&overdue=1&due_soon=1&keyword=)。
  function initialScope(value: string | null): Scope {
    return value === 'reporter' || value === 'all' || value === 'reviewer' ? value : 'assignee';
  }

  let scope = $state<Scope>(initialScope(appPage.url.searchParams.get('scope')));
  let keyword = $state(appPage.url.searchParams.get('keyword') ?? '');
  let overdueOnly = $state(appPage.url.searchParams.get('overdue') === '1');
  let dueSoonOnly = $state(appPage.url.searchParams.get('due_soon') === '1');
  let items = $state<TaskListItem[]>([]);
  let page = $state(1);
  let hasMore = $state(false);
  let loading = $state(true);
  let appending = $state(false);
  let errorMessage = $state('');
  let deletingId = $state<string | null>(null);

  const isOverdue = (task: TaskListItem) => Boolean(task.due_at && new Date(task.due_at) < new Date());

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

  async function removeTask(task: TaskListItem) {
    if (
      !(await confirmDialog({
        title: '逻辑删除任务',
        message: `确定删除 ${task.task_key}「${task.title}」吗？删除后可在项目操作日志追溯。`,
        confirmLabel: '删除',
        danger: true
      }))
    ) {
      return;
    }
    deletingId = task.id;
    errorMessage = '';
    try {
      await deleteTask(task.task_key, '用户从全局任务列表删除任务');
      items = items.filter((item) => item.id !== task.id);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务删除失败';
    } finally {
      deletingId = null;
    }
  }

  bindReload(() => void load(1));
</script>

<PageHeader
  title="任务"
  eyebrow="Tasks"
  description="跨项目聚合你负责的、创建的以及可见项目的全部任务。"
/>

<div class="scope-tabs" role="tablist" aria-label="任务范围">
  {#each scopes as item}
    <button
      class="scope-tab"
      class:active={scope === item.value}
      role="tab"
      aria-selected={scope === item.value}
      type="button"
      onclick={() => switchScope(item.value)}
    >
      {item.label}
    </button>
  {/each}
  <form class="scope-tools" onsubmit={applySearch}>
    <input
      class="search-input"
      bind:value={keyword}
      placeholder="搜索任务标题或编号"
      aria-label="搜索任务"
    />
    <button class="secondary-button" type="submit" disabled={loading}>搜索</button>
    <label class="overdue-toggle">
      <input type="checkbox" checked={overdueOnly} onchange={toggleOverdue} />
      仅看逾期
    </label>
    <label class="overdue-toggle">
      <input type="checkbox" checked={dueSoonOnly} onchange={toggleDueSoon} />
      7 天内到期
    </label>
  </form>
</div>

{#if errorMessage}
  <div class="error-message" role="alert">{errorMessage}</div>
{/if}

{#if loading}
  <div class="workspace-card state-box">正在加载任务…</div>
{:else if !groups.length}
  <div class="workspace-card state-box">
    <strong>没有任务</strong>
    <p>切换范围,或到项目看板创建第一个任务。</p>
    <a class="secondary-button" href="/projects">去项目列表</a>
  </div>
{:else}
  {#each groups as group (group.projectKey)}
    <section class="workspace-card project-group">
      <header>
        <a class="project-link" href={`/projects/${group.projectKey}/board`}>
          <strong>{group.projectName}</strong>
          <code>{group.projectKey}</code>
        </a>
        <span>{group.tasks.length} 项</span>
      </header>
      <div class="task-rows">
        {#each group.tasks as task (task.id)}
          <div class="task-row">
            <a class="task-main" href={`/tasks/${task.task_key}`}>
              <span class="task-key">{task.task_key}</span>
              <span class="task-title">{task.title}</span>
              <span class="status-pill">{task.status_name}</span>
              <TaskTypePill taskType={task.task_type} />
              <PriorityPill priority={task.priority} />
              <span class="assignee">
                {#if task.assignee_name}<Avatar name={task.assignee_name} size={18} />{task.assignee_name}{:else}未分配{/if}
              </span>
              {#if task.due_at}
                <span class="due" class:danger={isOverdue(task)}>
                  {new Date(task.due_at).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })} 截止
                </span>
              {:else}
                <span class="due"></span>
              {/if}
              <time>{new Date(task.updated_at).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })}</time>
            </a>
            <button
              class="row-delete"
              type="button"
              disabled={deletingId === task.id}
              onclick={() => removeTask(task)}
            >
              {deletingId === task.id ? '删除中…' : '删除'}
            </button>
          </div>
        {/each}
      </div>
    </section>
  {/each}
  {#if hasMore}
    <div class="pager">
      <button class="secondary-button" type="button" disabled={appending} onclick={() => void load(page + 1, true)}>
        {appending ? '加载中…' : '加载更多'}
      </button>
    </div>
  {/if}
{/if}

<style>
  .scope-tabs {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-bottom: 18px;
    border-bottom: 1px solid var(--color-border);
  }
  .scope-tools { display: flex; align-items: center; gap: 10px; margin-left: auto; padding-bottom: 8px; }
  .search-input {
    width: 220px;
    padding: 7px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface);
    color: var(--color-text);
    font-size: 13px;
  }
  .search-input:focus-visible { outline: none; box-shadow: var(--color-focus-ring); }
  .scope-tools .secondary-button { padding: 7px 12px; font-size: 12px; }
  .overdue-toggle { display: inline-flex; align-items: center; gap: 5px; color: var(--color-text-muted); font-size: 12px; cursor: pointer; user-select: none; }
  .overdue-toggle input { accent-color: var(--color-primary); }
  .scope-tab {
    padding: 8px 2px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 13px;
    cursor: pointer;
    transition: color var(--transition-fast), border-color var(--transition-fast);
  }
  .scope-tab:hover { color: var(--color-text-secondary); }
  .scope-tab.active { color: var(--color-text); border-bottom-color: var(--color-text); font-weight: 500; }
  .scope-tab:focus-visible { outline: none; box-shadow: var(--color-focus-ring); border-radius: var(--radius-sm); }

  .error-message {
    margin-bottom: 14px;
    padding: 8px 12px;
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-md);
    color: var(--color-danger);
    font-size: 13px;
  }

  .project-group { margin-bottom: 16px; }
  .project-group header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--color-border);
  }
  .project-link { display: flex; align-items: baseline; gap: 8px; text-decoration: none; }
  .project-link strong { color: var(--color-text); font-size: 15px; font-weight: 500; }
  .project-link strong:hover { color: var(--color-primary); }
  .project-link code { color: var(--color-text-muted); font-family: var(--font-mono); font-size: 11px; }
  .project-group header > span { color: var(--color-text-muted); font-size: 12px; }

  .task-rows { display: grid; }
  .task-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 2px;
    border-bottom: 1px solid var(--color-border-weak);
    font-size: 13px;
    transition: background-color var(--transition-fast);
  }
  .task-row:last-child { border-bottom: 0; }
  .task-row:hover { background: var(--color-hover); }
  .task-main {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr) auto auto minmax(90px, auto) minmax(70px, auto) auto;
    align-items: center;
    gap: 14px;
    flex: 1;
    min-width: 0;
    color: var(--color-text);
    text-decoration: none;
  }
  .due { color: var(--color-text-muted); font-size: 12px; white-space: nowrap; }
  .due.danger { color: var(--color-danger); font-weight: 500; }
  .row-delete {
    flex: none;
    border: 0;
    padding: 4px 8px;
    background: transparent;
    color: var(--color-danger);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    border-radius: var(--radius-sm);
    opacity: 0;
    transition: opacity var(--transition-fast), background-color var(--transition-fast);
  }
  .task-row:hover .row-delete, .row-delete:focus-visible { opacity: 1; }
  .row-delete:disabled { cursor: not-allowed; opacity: 0.45; }
  .task-key { color: var(--color-text-muted); font-family: var(--font-mono); font-size: 12px; }
  .task-main:hover .task-key { color: var(--color-primary-strong); }
  .task-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  .assignee { display: inline-flex; align-items: center; gap: 5px; color: var(--color-text-muted); }
  .task-main time { color: var(--color-text-muted); font-size: 12px; }

  .state-box { display: grid; place-items: center; gap: 8px; min-height: 200px; color: var(--color-text-muted); }
  .state-box strong { color: var(--color-text-secondary); font-size: 14px; font-weight: 500; }
  .state-box p { font-size: 13px; }
  .state-box a { margin-top: 6px; }

  .pager { display: flex; justify-content: center; padding: 6px 0 14px; }

  @media (max-width: 900px) {
    .scope-tabs { flex-wrap: wrap; row-gap: 10px; }
    .scope-tools { margin-left: 0; width: 100%; }
  }
  @media (max-width: 760px) {
    .task-main { grid-template-columns: minmax(0, 1fr) auto; row-gap: 6px; }
    .task-main :global(.priority-pill), .assignee, .due { display: none; }
    .row-delete { opacity: 1; }
  }
</style>
