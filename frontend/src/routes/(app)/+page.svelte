<script lang="ts">
  import { onMount } from 'svelte';
  import MetricCard from '$lib/components/MetricCard.svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { listProjects } from '$lib/api/projects';
  import { listMyTasks } from '$lib/api/tasks';
  import type { ProjectView, TaskListItem } from '$lib/api/types';

  let projects = $state<ProjectView[]>([]);
  let assigned = $state<TaskListItem[]>([]);
  let reviewing = $state<TaskListItem[]>([]);
  let assignedTotal = $state(0);
  let reviewingTotal = $state(0);
  let createdTotal = $state(0);
  let overdueTotal = $state(0);
  let loading = $state(true);
  let errorMessage = $state('');

  const isOverdue = (task: TaskListItem) => Boolean(task.due_at && new Date(task.due_at) < new Date());
  const dueLabel = (task: TaskListItem) =>
    task.due_at ? new Date(task.due_at).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' }) : '';

  onMount(async () => {
    try {
      const [projectResponse, assignedResponse, reviewingResponse, createdResponse, overdueResponse] = await Promise.all([
        listProjects(1, 6),
        listMyTasks('assignee', 1, 10),
        listMyTasks('reviewer', 1, 10),
        listMyTasks('reporter', 1, 1),
        listMyTasks('assignee', 1, 1, { overdue: true })
      ]);
      projects = projectResponse.data.items;
      assigned = assignedResponse.data.items;
      assignedTotal = assignedResponse.data.total;
      reviewing = reviewingResponse.data.items;
      reviewingTotal = reviewingResponse.data.total;
      createdTotal = createdResponse.data.total;
      overdueTotal = overdueResponse.data.total;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '工作台加载失败';
    } finally {
      loading = false;
    }
  });
</script>

<PageHeader
  title="工作台"
  eyebrow="Workspace"
  description="汇总你负责的、待评审的与逾期的任务,快速进入近期项目。"
  actionHref="/projects/new"
  actionLabel="新建项目"
/>

{#if errorMessage}
  <section class="workspace-card error-state">{errorMessage}</section>
{:else if loading}
  <section class="workspace-card state-box">正在加载工作台…</section>
{:else}
  <div class="dashboard-grid">
    <MetricCard label="我负责的" value={String(assignedTotal)} hint="分配给你的全部任务" />
    <MetricCard label="待我评审" value={String(reviewingTotal)} hint="等待你评审定稿" />
    <MetricCard label="我创建的" value={String(createdTotal)} hint="由你发起的任务" />
    <MetricCard label="已逾期" value={String(overdueTotal)} hint="你负责且超过截止时间" />
  </div>

  <div class="workbench-grid">
    <section class="workspace-card">
      <header class="card-head">
        <h2>我负责的任务</h2>
        <a href="/tasks">查看全部</a>
      </header>
      {#each assigned as task (task.id)}
        <a class="task-row" class:overdue={isOverdue(task)} href={`/tasks/${task.task_key}`}>
          <span class="task-key">{task.task_key}</span>
          <span class="task-title">{task.title}</span>
          <span class="status-pill">{task.status_name}</span>
          {#if task.due_at}<span class="due" class:danger={isOverdue(task)}>{dueLabel(task)} 截止</span>{/if}
        </a>
      {:else}
        <p class="empty-inline">还没有分配给你的任务。</p>
      {/each}
    </section>
    <section class="workspace-card">
      <header class="card-head">
        <h2>待我评审的任务</h2>
        <a href="/tasks">查看全部</a>
      </header>
      {#each reviewing as task (task.id)}
        <a class="task-row" class:overdue={isOverdue(task)} href={`/tasks/${task.task_key}`}>
          <span class="task-key">{task.task_key}</span>
          <span class="task-title">{task.title}</span>
          <span class="status-pill">{task.status_name}</span>
          {#if task.due_at}<span class="due" class:danger={isOverdue(task)}>{dueLabel(task)} 截止</span>{/if}
        </a>
      {:else}
        <p class="empty-inline">没有等待你评审的任务。</p>
      {/each}
    </section>
  </div>

  <section class="workspace-card">
    <header class="card-head">
      <h2>近期项目</h2>
      <a href="/projects">全部项目</a>
    </header>
    {#each projects as project (project.id)}
      <div class="project-row">
        <strong>{project.project_key}</strong>
        <span>{project.name} · {project.archived_at ? '已归档' : '活跃'}</span>
        <a href={`/projects/${project.project_key}`}>进入</a>
      </div>
    {:else}
      <p class="empty-inline">当前账号还没有可见项目。</p>
    {/each}
  </section>
{/if}

<style>
  .dashboard-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 16px; margin-bottom: 18px; }
  .workbench-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; margin-bottom: 18px; }
  .card-head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; margin-bottom: 4px; }
  .card-head h2 { margin: 0; font-size: 15px; font-weight: 500; }
  .card-head a { color: var(--color-primary); font-size: 13px; font-weight: 500; }
  .task-row { display: grid; grid-template-columns: 96px minmax(0, 1fr) auto auto; align-items: center; gap: 12px; padding: 10px 0; border-top: 1px solid var(--color-border); color: var(--color-text); text-decoration: none; font-size: 13px; }
  .task-row:hover .task-title { color: var(--color-primary); }
  .task-key { color: var(--color-text-muted); font-family: var(--font-mono); font-size: 12px; }
  .task-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  .due { color: var(--color-text-muted); font-size: 12px; }
  .due.danger { color: var(--color-danger); }
  .project-row { display: grid; grid-template-columns: 150px 1fr auto; gap: 12px; padding: 12px 0; border-top: 1px solid var(--color-border); }
  .project-row span { color: var(--color-text-muted); }
  .project-row a { color: var(--color-primary); font-weight: 500; }
  .empty-inline { padding: 12px 0; color: var(--color-text-muted); font-size: 13px; }
  .error-state { color: var(--color-danger); }
  .state-box { text-align: center; color: var(--color-text-muted); }
  @media (max-width: 1024px) { .dashboard-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @media (max-width: 860px) { .workbench-grid { grid-template-columns: 1fr; } }
  @media (max-width: 640px) { .dashboard-grid, .project-row { grid-template-columns: 1fr; } }
</style>
