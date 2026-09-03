<script lang="ts">
  import { onMount } from 'svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import { listProjects } from '$lib/api/projects';
  import { listMyTasks, type MyTaskScope } from '$lib/api/tasks';
  import type { ProjectView, TaskListItem } from '$lib/api/types';
  import { meStore } from '$lib/features/auth/me.svelte';

  type Scope = Extract<MyTaskScope, 'assignee' | 'reviewer' | 'reporter'>;

  const scopes: { value: Scope; label: string }[] = [
    { value: 'assignee', label: '我负责的' },
    { value: 'reviewer', label: '我评审的' },
    { value: 'reporter', label: '我创建的' }
  ];

  let projects = $state<ProjectView[]>([]);
  let projectCountLabel = $state('0');
  let scope = $state<Scope>('assignee');
  let scoped = $state<TaskListItem[]>([]);
  let totals = $state<Record<Scope, number>>({ assignee: 0, reviewer: 0, reporter: 0 });
  let overdueTotal = $state(0);
  let dueSoonTotal = $state(0);
  let listLoading = $state(true);
  let loading = $state(true);
  let errorMessage = $state('');

  const weekdays = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];
  const today = new Date();
  const todayLabel = `${today.getMonth() + 1}月${today.getDate()}日 ${weekdays[today.getDay()]}`;

  const isOverdue = (task: TaskListItem) => Boolean(task.due_at && task.status_category !== 'done' && new Date(task.due_at) < new Date());
  const dueLabel = (task: TaskListItem) =>
    task.due_at ? new Date(task.due_at).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' }) : '';

  async function loadScope(next: Scope) {
    listLoading = true;
    try {
      const response = await listMyTasks(next, 1, 10);
      scoped = response.data.items;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '任务加载失败';
    } finally {
      listLoading = false;
    }
  }

  function switchScope(next: Scope) {
    if (next === scope) return;
    scope = next;
    void loadScope(next);
  }

  onMount(async () => {
    try {
      const [projectResponse, assignedResponse, reviewingResponse, reporterResponse, overdueResponse, dueSoonResponse] = await Promise.all([
        listProjects(1, 6),
        listMyTasks('assignee', 1, 10),
        listMyTasks('reviewer', 1, 1),
        listMyTasks('reporter', 1, 1),
        listMyTasks('assignee', 1, 1, { overdue: true }),
        listMyTasks('assignee', 1, 1, { dueSoon: true })
      ]);
      projects = projectResponse.data.items;
      // 列表接口没有 total,首页只取 6 条,has_more 时用 "6+" 表达还有更多。
      projectCountLabel = projectResponse.data.has_more ? `${projectResponse.data.items.length}+` : `${projectResponse.data.items.length}`;
      scoped = assignedResponse.data.items;
      totals = { assignee: assignedResponse.data.total, reviewer: reviewingResponse.data.total, reporter: reporterResponse.data.total };
      overdueTotal = overdueResponse.data.total;
      dueSoonTotal = dueSoonResponse.data.total;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '工作台加载失败';
    } finally {
      loading = false;
      listLoading = false;
    }
  });
</script>

{#if errorMessage}
  <section class="workspace-card error-state">{errorMessage}</section>
{:else if loading}
  <section class="workspace-card state-box">正在加载工作台…</section>
{:else}
  <div class="page-head">
    <h1>工作台</h1>
    <div class="meta-row">
      <span class="meta-item">你好,{meStore.current?.display_name ?? '同学'}</span><span class="sep">·</span>
      <span class="meta-item">{todayLabel}</span><span class="sep">·</span>
      <span class="meta-item">{projectCountLabel} 个近期项目</span>
    </div>
  </div>

  <div class="layout">
    <main>
      <section class="block">
        <div class="block-head">
          <h2>我的任务</h2>
          <div class="segmented" role="tablist" aria-label="任务范围">
            {#each scopes as item (item.value)}
              <button
                class:active={scope === item.value}
                role="tab"
                aria-selected={scope === item.value}
                type="button"
                onclick={() => switchScope(item.value)}
              >
                {item.label} <span class="n">{totals[item.value]}</span>
              </button>
            {/each}
          </div>
          <span class="spacer"></span>
          <a class="more" href={`/tasks?scope=${scope}`}>查看全部 →</a>
        </div>
        {#if listLoading}
          <p class="block-hint">正在加载…</p>
        {:else}
          <div class="task-rows">
            {#each scoped as task (task.id)}
              <a class="task-row" href={`/tasks/${task.task_key}`}>
                <span class="task-key">{task.task_key}</span>
                <span class="task-title">{task.title}</span>
                <StatusBadge name={task.status_name} category={task.status_category} />
                {#if task.due_at}
                  <span class="due" class:danger={isOverdue(task)}>{dueLabel(task)} 截止</span>
                {:else}
                  <span class="due"></span>
                {/if}
              </a>
            {:else}
              <p class="block-hint">这个范围还没有任务。</p>
            {/each}
          </div>
        {/if}
      </section>

      <section class="block">
        <div class="block-head">
          <h2>近期项目</h2>
          <span class="spacer"></span>
          <a class="more" href="/projects">全部项目 →</a>
        </div>
        <div class="project-grid">
          {#each projects as project (project.id)}
            <a class="project-card" href={`/projects/${project.project_key}`}>
              <span class="project-top">
                <code>{project.project_key}</code>
                <StatusBadge name={project.archived_at ? '已归档' : '活跃'} category={project.archived_at ? 'todo' : 'in_progress'} />
              </span>
              <strong>{project.name}</strong>
              <span class="project-enter">进入 →</span>
            </a>
          {:else}
            <p class="block-hint">当前账号还没有可见项目。</p>
          {/each}
        </div>
      </section>
    </main>

    <aside>
      <div class="panel">
        <div class="panel-title">
          关注
          <span class="info tooltip" data-tip="统计范围:分配给你且未完成的任务。点击数字直达对应过滤视图。">i</span>
        </div>
        <a class="stat-row" href="/tasks?scope=assignee&overdue=1">
          <i class="stat-dot danger"></i>
          已逾期
          <strong class="num danger">{overdueTotal}</strong>
        </a>
        <a class="stat-row" href="/tasks?scope=assignee&due_soon=1">
          <i class="stat-dot warn"></i>
          7 天内到期
          <strong class="num warn">{dueSoonTotal}</strong>
        </a>
        <a class="stat-row" href="/tasks?scope=reviewer">
          <i class="stat-dot"></i>
          待我评审
          <strong class="num">{totals.reviewer}</strong>
        </a>
        <a class="stat-row" href="/tasks?scope=reporter">
          <i class="stat-dot"></i>
          我创建的
          <strong class="num">{totals.reporter}</strong>
        </a>
      </div>
      <div class="panel">
        <div class="panel-title">快捷入口</div>
        <div class="quick-links">
          <a class="ghost" href="/projects/new">＋ 新建项目</a>
          <a class="ghost" href="/search">去全局搜索</a>
        </div>
      </div>
    </aside>
  </div>
{/if}

<style>
  .page-head { margin-bottom: 22px; }
  .page-head h1 { margin: 0; font-size: 22px; font-weight: 600; line-height: 1.35; }
  .meta-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; margin-top: 8px; font-size: 13px; color: var(--color-text-muted); }
  .meta-item { display: inline-flex; align-items: center; gap: 6px; }
  .sep { color: var(--color-border); }

  .layout { display: grid; grid-template-columns: minmax(0, 1fr) 280px; gap: 28px; align-items: start; }

  .block { padding: 18px 0; border-top: 1px solid var(--color-border-weak); }
  .block:first-child { border-top: 0; padding-top: 0; }
  .block-head { display: flex; align-items: center; gap: 10px; margin-bottom: 12px; flex-wrap: wrap; }
  .block-head h2 { margin: 0; font-size: 13px; font-weight: 600; color: var(--color-text-secondary); letter-spacing: 0.02em; }
  .block-head .spacer { flex: 1; }
  .block-head .more { font-size: 12px; color: var(--color-text-muted); }
  .block-head .more:hover { color: var(--color-primary); }
  .block-hint { color: var(--color-text-muted); font-size: 13px; }

  .segmented { display: inline-flex; gap: 2px; padding: 2px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .segmented button { padding: 4px 10px; border-radius: calc(var(--radius-md) - 2px); background: transparent; color: var(--color-text-muted); font-size: 12px; cursor: pointer; transition: background-color var(--transition-fast), color var(--transition-fast); }
  .segmented button:hover { color: var(--color-text-secondary); }
  .segmented button.active { background: var(--color-surface-raised); color: var(--color-text); font-weight: 500; box-shadow: 0 0 0 1px var(--color-border-weak); }
  .segmented .n { font-size: 11px; color: var(--color-text-muted); font-family: var(--font-mono); margin-left: 3px; }

  .task-rows { display: grid; }
  .task-row { display: grid; grid-template-columns: 100px minmax(0, 1fr) auto auto; align-items: center; gap: 12px; padding: 9px 8px; margin: 0 -8px; border-radius: var(--radius-sm); color: var(--color-text); text-decoration: none; font-size: 13px; }
  .task-row:hover { background: var(--color-hover); }
  .task-key { color: var(--color-text-muted); font-family: var(--font-mono); font-size: 12px; }
  .task-row:hover .task-key { color: var(--color-primary-strong); }
  .task-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  .due { color: var(--color-text-muted); font-size: 12px; }
  .due.danger { color: var(--color-danger); font-weight: 500; }

  .project-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 12px; }
  .project-card { display: grid; gap: 6px; padding: 14px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface); color: var(--color-text); text-decoration: none; transition: border-color var(--transition-fast); }
  .project-card:hover { border-color: var(--color-border-strong); }
  .project-top { display: flex; align-items: center; gap: 8px; }
  .project-top code { color: var(--color-primary-strong); font-family: var(--font-mono); font-size: 12px; }
  .project-top :global(.status-badge) { margin-left: auto; }
  .project-card strong { font-size: 14px; font-weight: 500; }
  .project-enter { font-size: 12px; color: var(--color-text-muted); }
  .project-card:hover .project-enter { color: var(--color-primary); }

  .panel { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-lg); overflow: hidden; }
  .panel + .panel { margin-top: 14px; }
  .panel-title { display: flex; align-items: center; gap: 6px; padding: 12px 14px 10px; border-bottom: 1px solid var(--color-border-weak); font-size: 12px; font-weight: 600; color: var(--color-text-secondary); letter-spacing: 0.04em; }
  .panel-title .info { margin-left: auto; cursor: help; color: var(--color-text-muted); font-style: italic; font-family: var(--font-mono); }
  .tooltip { position: relative; display: inline-flex; }
  .tooltip::after {
    content: attr(data-tip); position: absolute; right: 0; top: calc(100% + 6px); z-index: 5;
    width: 220px; padding: 8px 10px; border-radius: var(--radius-sm);
    background: var(--color-surface-sunken); border: 1px solid var(--color-border);
    color: var(--color-text-muted); font-size: 12px; font-style: normal; font-weight: 400; letter-spacing: 0;
    line-height: 1.5; text-align: left; white-space: normal;
    opacity: 0; visibility: hidden; transition: opacity var(--transition-fast); pointer-events: none;
  }
  .tooltip:hover::after { opacity: 1; visibility: visible; }
  .stat-row { display: flex; align-items: center; gap: 10px; padding: 10px 14px; font-size: 13px; color: var(--color-text-secondary); text-decoration: none; transition: background-color var(--transition-fast); }
  .stat-row:hover { background: var(--color-hover); }
  .stat-row + .stat-row { border-top: 1px solid var(--color-border-weak); }
  .stat-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--color-text-muted); flex: none; }
  .stat-dot.danger { background: var(--color-danger); }
  .stat-dot.warn { background: var(--color-warning); }
  .stat-row .num { margin-left: auto; font-size: 18px; font-family: var(--font-mono); color: var(--color-text-secondary); }
  .stat-row .num.danger { color: var(--color-danger); }
  .stat-row .num.warn { color: var(--color-warning); }

  .quick-links { display: grid; gap: 8px; padding: 12px 14px; }
  .ghost {
    display: inline-flex; align-items: center; justify-content: center; gap: 6px;
    padding: 6px 10px; border: 1px dashed var(--color-border); border-radius: var(--radius-sm);
    color: var(--color-text-muted); font-size: 12px; text-decoration: none;
    transition: color var(--transition-fast), border-color var(--transition-fast);
  }
  .ghost:hover { color: var(--color-primary-strong); border-color: var(--color-primary); }

  .error-state { color: var(--color-danger); }
  .state-box { display: grid; place-items: center; min-height: 220px; color: var(--color-text-muted); }
  @media (max-width: 900px) { .layout { grid-template-columns: 1fr; } }
  @media (max-width: 640px) { .task-row { grid-template-columns: minmax(0, 1fr) auto; row-gap: 4px; } .task-key { grid-column: 1; } }
</style>
