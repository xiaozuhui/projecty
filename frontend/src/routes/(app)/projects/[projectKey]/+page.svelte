<script lang="ts">
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import StatusPill from '$lib/components/StatusPill.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { getProject, listStatuses } from '$lib/api/projects';
  import { listTasks } from '$lib/api/tasks';
  import type { ProjectStatus, ProjectView, TaskView } from '$lib/api/types';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';

  type ScheduleRow = { task: TaskView; left: number; width: number; hasStart: boolean; hasEnd: boolean; category: string };
  let project = $state<ProjectView | null>(null);
  let statuses = $state<ProjectStatus[]>([]);
  let tasks = $state<TaskView[]>([]);
  let loading = $state(true);
  let errorMessage = $state('');
  const projectKey = $derived(String(page.params.projectKey ?? ''));
  const dateFormatter = new Intl.DateTimeFormat('zh-CN', { month: 'numeric', day: 'numeric' });
  const dateTimeFormatter = new Intl.DateTimeFormat('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  const statusOf = (task: TaskView) => statuses.find((status) => status.id === task.status_id);
  const categoryOf = (task: TaskView) => statusOf(task)?.category ?? 'todo';
  const toMillis = (value: string | null | undefined) => {
    if (!value) return null;
    const millis = new Date(value).getTime();
    return Number.isNaN(millis) ? null : millis;
  };
  const formatDate = (millis: number) => dateFormatter.format(new Date(millis));

  async function loadAllTasks() {
    const all: TaskView[] = [];
    let currentPage = 1;
    let hasMore = true;
    while (hasMore) {
      const response = await listTasks(projectKey, currentPage, 100);
      all.push(...response.data.items);
      hasMore = response.data.has_more;
      currentPage += 1;
    }
    return all;
  }

  async function load() {
    loading = true;
    errorMessage = '';
    try {
      const [projectResponse, statusResponse, allTasks] = await Promise.all([getProject(projectKey), listStatuses(projectKey), loadAllTasks()]);
      project = projectResponse.data;
      statuses = statusResponse.data;
      tasks = allTasks;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '项目加载失败';
    } finally {
      loading = false;
    }
  }

  const metrics = $derived.by(() => {
    const now = Date.now();
    const done = tasks.filter((task) => categoryOf(task) === 'done').length;
    const active = tasks.filter((task) => ['active', 'review'].includes(categoryOf(task))).length;
    const overdue = tasks.filter((task) => {
      const dueAt = toMillis(task.due_at);
      return dueAt !== null && dueAt < now && categoryOf(task) !== 'done' && categoryOf(task) !== 'canceled';
    }).length;
    const scheduled = tasks.filter((task) => task.start_at || task.due_at).length;
    return { total: tasks.length, done, active, overdue, scheduled };
  });
  const recentTasks = $derived([...tasks].sort((left, right) => toMillis(right.updated_at)! - toMillis(left.updated_at)!).slice(0, 5));

  const schedule = $derived.by(() => {
    const entries = tasks.filter((task) => task.start_at || task.due_at).map((task) => {
      const startAt = toMillis(task.start_at) ?? toMillis(task.created_at)!;
      const dueAt = toMillis(task.due_at) ?? startAt;
      return { task, startAt, endAt: Math.max(startAt, dueAt), hasStart: Boolean(task.start_at), hasEnd: Boolean(task.due_at) };
    }).sort((left, right) => left.startAt - right.startAt || left.endAt - right.endAt);
    if (!entries.length) return { rows: [] as ScheduleRow[], ticks: [] as { position: number; label: string }[], more: 0, todayPosition: null as number | null };
    const day = 24 * 60 * 60 * 1000;
    const rawStart = Math.min(...entries.map((entry) => entry.startAt));
    const rawEnd = Math.max(...entries.map((entry) => entry.endAt), Date.now());
    const padding = Math.max(day, Math.round((rawEnd - rawStart) * 0.06));
    const rangeStart = rawStart - padding;
    const rangeEnd = Math.max(rawEnd + padding, rangeStart + day * 2);
    const range = rangeEnd - rangeStart;
    const position = (millis: number) => Math.min(100, Math.max(0, ((millis - rangeStart) / range) * 100));
    const rows = entries.slice(0, 12).map((entry) => {
      const left = position(entry.startAt);
      return { task: entry.task, left, width: Math.max(2.5, position(entry.endAt) - left), hasStart: entry.hasStart, hasEnd: entry.hasEnd, category: categoryOf(entry.task) };
    });
    const ticks = Array.from({ length: 5 }, (_, index) => {
      const ratio = index / 4;
      return { position: ratio * 100, label: formatDate(rangeStart + range * ratio) };
    });
    return { rows, ticks, more: Math.max(0, entries.length - rows.length), todayPosition: position(Date.now()) };
  });
  bindReload(() => void load());
</script>

{#if loading}
  <div class="workspace-card state-box">正在加载项目概览…</div>
{:else if errorMessage}
  <div class="workspace-card state-box error-state"><strong>{errorMessage}</strong><a class="primary-button" href="/projects">返回项目列表</a></div>
{:else if project}
  <PageHeader title={project.name} eyebrow={project.project_key} description={project.description || '暂无项目描述。'} actionHref={`/projects/${project.project_key}/list`} actionLabel="查看任务" />
  <div class="overview-grid">
    <section class="workspace-card health-card">
      <div class="card-title"><div><h2>项目健康度</h2><p>以当前任务状态、逾期情况和排期完整度衡量推进节奏。</p></div><span class="health-badge" class:archived={Boolean(project.archived_at)}>{project.archived_at ? '已归档' : '运行中'}</span></div>
      <div class="metric-row">
        <div><strong>{metrics.total}</strong><span>全部任务</span></div><div><strong>{metrics.active}</strong><span>进行中 / 评审中</span></div><div><strong>{metrics.done}</strong><span>已完成</span></div><div class:metric-alert={metrics.overdue > 0}><strong>{metrics.overdue}</strong><span>逾期未完成</span></div>
      </div>
      <div class="schedule-summary"><span>已排期 <strong>{metrics.scheduled}</strong> / {metrics.total} 项</span>{#if metrics.total}<span class="schedule-progress"><i style={`width: ${(metrics.scheduled / metrics.total) * 100}%`}></i></span>{/if}</div>
    </section>
    <section class="workspace-card"><h2>项目导航</h2><div class="quick-links"><a href={`/projects/${project.project_key}/list`}>任务列表 <span>→</span></a><a href={`/projects/${project.project_key}/board`}>看板视图 <span>→</span></a><a href={`/projects/${project.project_key}/timeline`}>时间线 <span>→</span></a><a href={`/projects/${project.project_key}/members`}>成员与负责人 <span>→</span></a></div></section>
  </div>
  <section class="workspace-card schedule-card">
    <div class="card-title"><div><h2>项目节奏</h2><p>按任务开始与结束时间展开的排期图。实线代表完整排期，虚线端点表示缺少开始或结束时间。</p></div><a href={`/projects/${project.project_key}/list`}>补充任务排期</a></div>
    {#if schedule.rows.length}
      <div class="schedule-legend"><span class="legend todo">待处理</span><span class="legend active">进行中</span><span class="legend review">评审中</span><span class="legend done">已完成</span></div>
      <div class="schedule-scroll"><div class="schedule-chart">
        <div class="schedule-axis"><span></span><div class="axis-track">{#each schedule.ticks as tick}<span class="axis-tick" style={`left: ${tick.position}%`}>{tick.label}</span>{/each}</div></div>
        {#each schedule.rows as row (row.task.id)}
          <a class="schedule-row" href={`/tasks/${row.task.task_key}`} title={`${row.task.task_key} · ${row.task.title}`}><div class="schedule-task"><span>{row.task.task_key}</span><strong>{row.task.title}</strong></div><div class="timeline-track">{#if schedule.todayPosition !== null}<i class="today-line" style={`left: ${schedule.todayPosition}%`} aria-label="今天"></i>{/if}<span class={`schedule-bar ${row.category}`} class:missing-start={!row.hasStart} class:missing-end={!row.hasEnd} style={`left: ${row.left}%; width: ${row.width}%`}></span></div></a>
        {/each}
      </div></div>
      {#if schedule.more}<p class="more-schedule">为保证图表可读性，当前展示最早排期的 12 项任务；另有 {schedule.more} 项已排期任务。</p>{/if}
    {:else}<div class="schedule-empty"><strong>还没有可展示的任务排期</strong><p>为任务补充开始时间和结束时间后，这里会自动生成项目节奏图。</p><a class="secondary-button" href={`/projects/${project.project_key}/list`}>前往任务列表</a></div>{/if}
  </section>
  <section class="workspace-card"><div class="card-title"><div><h2>最近更新</h2><p>优先关注刚被调整或推进的任务。</p></div><a href={`/projects/${project.project_key}/list`}>查看全部</a></div>
    {#if recentTasks.length}<div class="recent-list">{#each recentTasks as task (task.id)}{@const status = statusOf(task)}<a href={`/tasks/${task.task_key}`}><span class="task-key">{task.task_key}</span><strong>{task.title}</strong><time>{dateTimeFormatter.format(new Date(task.updated_at))}</time>{#if status}<StatusPill {status} />{:else}<span class="status-pill">{task.status_id.slice(0, 8)}</span>{/if}</a>{/each}</div>{:else}<div class="empty-inline">还没有任务，<a href={`/projects/${project.project_key}/list`}>去任务列表创建第一项任务</a>。</div>{/if}
  </section>
{/if}

<style>
  h2, p { margin: 0; } h2 { font-size: 16px; font-weight: 500; } .state-box { display: grid; place-items: center; gap: 12px; min-height: 220px; } .error-state { color: var(--color-danger); }
  .overview-grid { display: grid; grid-template-columns: minmax(0, 1.55fr) minmax(280px, 1fr); gap: 18px; margin-bottom: 18px; } .health-card { display: grid; align-content: start; }
  .card-title { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 20px; } .card-title p { margin-top: 5px; color: var(--color-text-muted); font-size: 13px; line-height: 1.5; } .card-title a { flex: none; color: var(--color-primary); font-size: 13px; font-weight: 500; }
  .health-badge { padding: 3px 9px; border-radius: 999px; color: var(--color-success); background: color-mix(in srgb, var(--color-success) 14%, transparent); font-size: 12px; } .health-badge.archived { color: var(--color-warning); background: color-mix(in srgb, var(--color-warning) 16%, transparent); }
  .metric-row { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; } .metric-row > div { display: grid; gap: 5px; padding: 14px; background: var(--color-surface-sunken); border: 1px solid var(--color-border-weak); border-radius: var(--radius-md); } .metric-row > div.metric-alert { border-color: color-mix(in srgb, var(--color-danger) 45%, var(--color-border)); } .metric-row strong { font-size: 24px; font-weight: 500; } .metric-row span, .empty-inline { color: var(--color-text-muted); font-size: 12px; }
  .schedule-summary { display: grid; grid-template-columns: auto minmax(100px, 1fr); align-items: center; gap: 10px; margin-top: 16px; color: var(--color-text-muted); font-size: 12px; } .schedule-summary strong { color: var(--color-text-primary); } .schedule-progress { display: block; height: 6px; overflow: hidden; border-radius: 999px; background: var(--color-surface-sunken); } .schedule-progress i { display: block; height: 100%; border-radius: inherit; background: var(--color-primary); }
  .quick-links { display: grid; gap: 2px; margin-top: 13px; } .quick-links a { display: flex; justify-content: space-between; padding: 11px 0; border-bottom: 1px solid var(--color-border); color: var(--color-text-secondary); } .quick-links a:last-child { border-bottom: 0; } .quick-links a:hover { color: var(--color-primary); }
  .schedule-card { margin-bottom: 18px; } .schedule-legend { display: flex; flex-wrap: wrap; gap: 8px 14px; margin: -4px 0 16px; color: var(--color-text-muted); font-size: 12px; } .legend { display: inline-flex; align-items: center; gap: 6px; } .legend::before { width: 8px; height: 8px; border-radius: 50%; background: var(--color-text-muted); content: ''; } .legend.active::before { background: var(--color-primary); } .legend.review::before { background: var(--color-warning); } .legend.done::before { background: var(--color-success); }
  .schedule-scroll { overflow-x: auto; padding-bottom: 4px; } .schedule-chart { min-width: 670px; } .schedule-axis, .schedule-row { display: grid; grid-template-columns: minmax(190px, 0.8fr) minmax(430px, 2.2fr); gap: 16px; } .schedule-axis { margin-bottom: 6px; } .axis-track { position: relative; height: 22px; border-bottom: 1px solid var(--color-border); } .axis-tick { position: absolute; bottom: 4px; color: var(--color-text-muted); font-size: 11px; transform: translateX(-50%); white-space: nowrap; } .axis-tick:first-child { transform: none; } .axis-tick:last-child { transform: translateX(-100%); }
  .schedule-row { align-items: center; min-height: 42px; color: var(--color-text-primary); } .schedule-row:hover .schedule-task strong { color: var(--color-primary); } .schedule-task { display: flex; min-width: 0; align-items: center; gap: 8px; } .schedule-task span { flex: none; color: var(--color-primary-strong); font-family: var(--font-mono); font-size: 11px; } .schedule-task strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 500; }
  .timeline-track { position: relative; height: 30px; border-radius: var(--radius-sm); background: repeating-linear-gradient(to right, transparent 0, transparent calc(25% - 1px), var(--color-border-weak) calc(25% - 1px), var(--color-border-weak) 25%); } .today-line { position: absolute; z-index: 2; top: -5px; bottom: -5px; width: 1px; background: color-mix(in srgb, var(--color-danger) 68%, transparent); pointer-events: none; } .schedule-bar { position: absolute; z-index: 1; top: 9px; height: 12px; min-width: 12px; border-radius: 999px; background: var(--color-text-muted); box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-text-primary) 12%, transparent); } .schedule-bar.active { background: var(--color-primary); } .schedule-bar.review { background: var(--color-warning); } .schedule-bar.done { background: var(--color-success); } .schedule-bar.missing-start { border-left: 2px dashed color-mix(in srgb, var(--color-text-primary) 70%, transparent); border-radius: 2px 999px 999px 2px; } .schedule-bar.missing-end { border-right: 2px dashed color-mix(in srgb, var(--color-text-primary) 70%, transparent); border-radius: 999px 2px 2px 999px; } .schedule-bar.missing-start.missing-end { border-radius: 2px; }
  .more-schedule { margin-top: 12px; color: var(--color-text-muted); font-size: 12px; } .schedule-empty { display: grid; justify-items: start; gap: 8px; min-height: 150px; align-content: center; padding: 8px 0; } .schedule-empty p { color: var(--color-text-muted); font-size: 13px; } .schedule-empty a { border: 0; }
  .recent-list { display: grid; } .recent-list a { display: grid; grid-template-columns: 105px minmax(0, 1fr) auto auto; align-items: center; gap: 12px; padding: 12px 0; border-top: 1px solid var(--color-border); } .task-key { color: var(--color-primary-strong); font-family: var(--font-mono); font-size: 12px; } .recent-list strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; } .recent-list time { color: var(--color-text-muted); font-size: 12px; } .empty-inline { padding: 24px 0; } .empty-inline a { color: var(--color-primary); }
  @media (max-width: 920px) { .overview-grid { grid-template-columns: 1fr; } } @media (max-width: 680px) { .metric-row { grid-template-columns: repeat(2, 1fr); } .card-title { display: grid; } .schedule-summary { grid-template-columns: 1fr; } .recent-list a { grid-template-columns: 1fr auto; gap: 5px 10px; } .recent-list strong { grid-column: 1 / -1; } .recent-list time { grid-column: 1; } .recent-list :global(.status-pill) { grid-column: 2; grid-row: 1; } }
</style>
