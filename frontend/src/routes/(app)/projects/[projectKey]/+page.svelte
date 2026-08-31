<script lang="ts">
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import StatusPill from '$lib/components/StatusPill.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { getProject, listStatuses } from '$lib/api/projects';
  import { listTasks } from '$lib/api/tasks';
  import type { ProjectStatus, ProjectView, TaskListResponse } from '$lib/api/types';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';

  let project = $state<ProjectView | null>(null);
  let statuses = $state<ProjectStatus[]>([]);
  let tasks = $state<TaskListResponse | null>(null);
  let loading = $state(true);
  let errorMessage = $state('');
  const projectKey = $derived(String(page.params.projectKey ?? ''));

  async function load() {
    loading = true;
    errorMessage = '';
    try {
      const [projectResponse, statusResponse, taskResponse] = await Promise.all([
        getProject(projectKey),
        listStatuses(projectKey),
        listTasks(projectKey, 1, 5)
      ]);
      project = projectResponse.data;
      statuses = statusResponse.data;
      tasks = taskResponse.data;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '项目加载失败';
    } finally {
      loading = false;
    }
  }

  bindReload(() => void load());
</script>

{#if loading}
  <div class="workspace-card state-box">正在加载项目概览…</div>
{:else if errorMessage}
  <div class="workspace-card state-box error-state">
    <strong>{errorMessage}</strong>
    <a class="primary-button" href="/projects">返回项目列表</a>
  </div>
{:else if project}
  <PageHeader
    title={project.name}
    eyebrow={project.project_key}
    description={project.description || '暂无项目描述。'}
    actionHref={`/projects/${project.project_key}/list`}
    actionLabel="查看任务"
  />
  <div class="overview-grid">
    <section class="workspace-card">
      <div class="card-title">
        <h2>项目健康度</h2>
        <span class="health-badge" class:archived={Boolean(project.archived_at)}>{project.archived_at ? '已归档' : '运行中'}</span>
      </div>
      <div class="metric-row">
        <div><strong>{project.task_number_seed}</strong><span>已分配任务编号</span></div>
        <div><strong>{statuses.length}</strong><span>任务状态</span></div>
        <div><strong>{tasks?.items.length ?? 0}</strong><span>近期任务</span></div>
      </div>
    </section>
    <section class="workspace-card">
      <h2>项目导航</h2>
      <div class="quick-links">
        <a href={`/projects/${project.project_key}/list`}>任务列表 <span>→</span></a>
        <a href={`/projects/${project.project_key}/board`}>看板视图 <span>→</span></a>
        <a href={`/projects/${project.project_key}/members`}>成员与负责人 <span>→</span></a>
        <a href={`/projects/${project.project_key}/logs`}>操作日志 <span>→</span></a>
      </div>
    </section>
  </div>
  <section class="workspace-card">
    <div class="card-title">
      <h2>最近任务</h2>
      <a href={`/projects/${project.project_key}/list`}>查看全部</a>
    </div>
    {#if tasks?.items.length}
      <div class="recent-list">
        {#each tasks.items as task (task.id)}
          {@const status = statuses.find((item) => item.id === task.status_id)}
          <a href={`/tasks/${task.task_key}`}>
            <span class="task-key">{task.task_key}</span>
            <strong>{task.title}</strong>
            {#if status}
              <StatusPill {status} />
            {:else}
              <span class="status-pill">{task.status_id.slice(0, 8)}</span>
            {/if}
          </a>
        {/each}
      </div>
    {:else}
      <div class="empty-inline">
        还没有任务,<a href={`/projects/${project.project_key}/board`}>去看板</a>拖拽创建,或到<a href={`/projects/${project.project_key}/list`}>任务列表</a>添加。
      </div>
    {/if}
  </section>
{/if}

<style>
  h2 { margin: 0; font-size: 16px; font-weight: 500; }
  .state-box { display: grid; place-items: center; gap: 12px; min-height: 220px; }
  .error-state { color: var(--color-danger); }
  .overview-grid { display: grid; grid-template-columns: minmax(0, 1.5fr) minmax(280px, 1fr); gap: 18px; margin-bottom: 18px; }
  .card-title { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 20px; }
  .card-title a { color: var(--color-primary); font-size: 13px; font-weight: 500; }
  .health-badge { padding: 3px 9px; border-radius: 999px; color: var(--color-success); background: color-mix(in srgb, var(--color-success) 14%, transparent); font-size: 12px; }
  .health-badge.archived { background: color-mix(in srgb, var(--color-warning) 16%, transparent); }
  .health-badge.archived { color: var(--color-warning); }
  .metric-row { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
  .metric-row div { display: grid; gap: 5px; padding: 14px; background: var(--color-surface-sunken); border: 1px solid var(--color-border-weak); border-radius: var(--radius-md); }
  .metric-row strong { font-size: 24px; font-weight: 500; }
  .metric-row span, .empty-inline { color: var(--color-text-muted); font-size: 13px; }
  .quick-links { display: grid; gap: 2px; }
  .quick-links a { display: flex; justify-content: space-between; padding: 11px 0; border-bottom: 1px solid var(--color-border); color: var(--color-text-secondary); }
  .quick-links a:last-child { border-bottom: 0; }
  .quick-links a:hover { color: var(--color-primary); }
  .recent-list { display: grid; }
  .recent-list a { display: grid; grid-template-columns: 105px minmax(0, 1fr) auto; align-items: center; gap: 12px; padding: 12px 0; border-top: 1px solid var(--color-border); }
  .task-key { color: var(--color-primary-strong); font-family: var(--font-mono); font-size: 12px; }
  .recent-list strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  .empty-inline { padding: 24px 0; }
  .empty-inline a { color: var(--color-primary); }
  @media (max-width: 820px) { .overview-grid { grid-template-columns: 1fr; } }
  @media (max-width: 560px) {
    .metric-row { grid-template-columns: 1fr; }
    .recent-list a { grid-template-columns: 1fr; gap: 6px; }
  }
</style>
