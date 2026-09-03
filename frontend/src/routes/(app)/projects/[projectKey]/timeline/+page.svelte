<script lang="ts">
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { listStatuses } from '$lib/api/projects';
  import { listTasks, listProjectDependencies } from '$lib/api/tasks';
  import { listMilestones } from '$lib/api/milestones';
  import type { Milestone, ProjectDependencyEdge, ProjectStatus, TaskView } from '$lib/api/types';

  const projectKey = $derived(String(page.params.projectKey ?? ''));
  const DAY_MS = 24 * 60 * 60 * 1000;
  const DAY_W = 32;
  const ROW_H = 40;
  const LABEL_W = 260;

  let tasks = $state<TaskView[]>([]);
  let statuses = $state<ProjectStatus[]>([]);
  let milestones = $state<Milestone[]>([]);
  let edges = $state<ProjectDependencyEdge[]>([]);
  let loading = $state(true);
  let errorMessage = $state('');

  const startOfDay = (value: number) => Math.floor(value / DAY_MS) * DAY_MS;
  const statusOf = (task: TaskView) => statuses.find((status) => status.id === task.status_id);
  const isDone = (task: TaskView) => statusOf(task)?.category === 'done';
  const isOverdue = (task: TaskView) => Boolean(task.due_at && !isDone(task) && new Date(task.due_at) < new Date());

  // 有排期(开始或截止)的任务进入甘特行,按开始/截止排序;其余沉底为普通清单。
  const scheduled = $derived(
    tasks
      .filter((task) => task.start_at || task.due_at)
      .sort((left, right) => {
        const lv = new Date(left.start_at ?? left.due_at ?? 0).getTime();
        const rv = new Date(right.start_at ?? right.due_at ?? 0).getTime();
        return lv - rv;
      })
  );
  const unscheduled = $derived(tasks.filter((task) => !task.start_at && !task.due_at));

  // 日期窗口:覆盖全部排期与里程碑,尾部留 7 天余量。
  const range = $derived.by(() => {
    const points: number[] = [];
    for (const task of scheduled) {
      points.push(new Date(task.start_at ?? task.due_at!).getTime());
      points.push(new Date(task.due_at ?? task.start_at!).getTime());
    }
    for (const milestone of milestones) {
      if (milestone.due_date) points.push(new Date(`${milestone.due_date}T00:00:00Z`).getTime());
    }
    if (!points.length) return null;
    const min = startOfDay(Math.min(...points));
    const max = startOfDay(Math.max(...points)) + DAY_MS;
    const totalDays = Math.max(1, Math.round((max - min) / DAY_MS)) + 7;
    return { start: min, totalDays };
  });

  const xOf = (iso: string) => {
    if (!range) return 0;
    return ((startOfDay(new Date(iso).getTime()) - range.start) / DAY_MS) * DAY_W;
  };
  const barLeft = (task: TaskView) => xOf(task.start_at ?? task.due_at!);
  const barWidth = (task: TaskView) => {
    if (task.start_at && task.due_at) {
      return Math.max(DAY_W, xOf(task.due_at) + DAY_W - xOf(task.start_at));
    }
    return DAY_W;
  };
  const chartWidth = $derived((range?.totalDays ?? 1) * DAY_W);
  const chartHeight = $derived(scheduled.length * ROW_H);
  const rowIndex = $derived.by(() => {
    const map = new Map<string, number>();
    scheduled.forEach((task, index) => map.set(task.task_key, index));
    return map;
  });

  // 依赖连线:blocker 条右缘 → 被阻塞任务行左缘,未完成 blocker 标红。
  const dependencyLines = $derived.by(() => {
    if (!range) return [];
    const result: { x1: number; y1: number; x2: number; y2: number; blocked: boolean }[] = [];
    for (const edge of edges) {
      const blockedRow = rowIndex.get(edge.task_key);
      const blockerRow = rowIndex.get(edge.depends_on_task_key);
      if (blockedRow === undefined || blockerRow === undefined) continue;
      const blocker = scheduled.find((task) => task.task_key === edge.depends_on_task_key)!;
      const blocked = scheduled.find((task) => task.task_key === edge.task_key)!;
      result.push({
        x1: barLeft(blocker) + barWidth(blocker),
        y1: blockerRow * ROW_H + ROW_H / 2,
        x2: barLeft(blocked),
        y2: blockedRow * ROW_H + ROW_H / 2,
        blocked: !edge.is_done
      });
    }
    return result;
  });

  const axisLabels = $derived.by(() => {
    if (!range) return [];
    const labels: { left: number; text: string }[] = [];
    for (let day = 0; day < range.totalDays; day += 1) {
      const date = new Date(range.start + day * DAY_MS);
      const dayOfMonth = date.getUTCDate();
      if (day === 0 || dayOfMonth === 1) {
        labels.push({ left: day * DAY_W, text: `${date.getUTCFullYear()}-${String(date.getUTCMonth() + 1).padStart(2, '0')}` });
      }
    }
    return labels;
  });
  const milestoneMarks = $derived(
    milestones
      .filter((milestone) => milestone.due_date)
      .map((milestone) => ({
        left: xOf(`${milestone.due_date}T00:00:00Z`) + DAY_W,
        name: milestone.name,
        reached: milestone.is_reached
      }))
  );

  const fmtShort = (iso: string) => new Date(iso).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' });

  onMount(async () => {
    try {
      const [taskResponse, statusResponse, milestoneResponse, edgeResponse] = await Promise.all([
        listTasks(projectKey, 1, 100),
        listStatuses(projectKey),
        listMilestones(projectKey),
        listProjectDependencies(projectKey)
      ]);
      tasks = taskResponse.data.items;
      statuses = statusResponse.data;
      milestones = milestoneResponse.data.items;
      edges = edgeResponse.data.items;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '时间线加载失败';
    } finally {
      loading = false;
    }
  });
</script>

<PageHeader
  title="时间线"
  eyebrow="Timeline"
  description="任务排期横条与里程碑节点,虚线箭头表示「被谁阻塞」;红色代表阻塞任务尚未完成。"
/>

{#if errorMessage}
  <div class="workspace-card error-state">{errorMessage}</div>
{/if}
{#if loading}
  <section class="workspace-card state-box">正在加载时间线…</section>
{:else if range && scheduled.length}
  <section class="workspace-card gantt">
    <div class="gantt-scroll">
      <div class="gantt-inner" style="width: {LABEL_W + chartWidth}px">
        <div class="axis-row" style="width: {chartWidth}px; margin-left: {LABEL_W}px">
          {#each axisLabels as label (label.left)}
            <span style="left: {label.left}px">{label.text}</span>
          {/each}
        </div>
        <div class="gantt-body">
          <div class="label-col" style="width: {LABEL_W}px">
            {#each scheduled as task (task.id)}
              <a class="row-label" href={`/tasks/${task.task_key}`} style="height: {ROW_H}px">
                <code>{task.task_key}</code>
                <span title={task.title}>{task.title}</span>
              </a>
            {/each}
          </div>
          <div class="chart-col" style="width: {chartWidth}px; --day-w: {DAY_W}px">
            {#each milestoneMarks as mark (mark.left)}
              <div class="milestone-line" class:reached={mark.reached} style="left: {mark.left}px" title={mark.name}>
                <span>{mark.name}</span>
              </div>
            {/each}
            <svg class="dep-overlay" width={chartWidth} height={chartHeight} viewBox="0 0 {chartWidth} {chartHeight}">
              <defs>
                <marker id="dep-arrow" markerWidth="7" markerHeight="7" refX="6" refY="3.5" orient="auto">
                  <polygon points="0 0, 7 3.5, 0 7" fill="currentColor" />
                </marker>
              </defs>
              {#each dependencyLines as line, index (index)}
                <path
                  class:line-blocked={line.blocked}
                  d="M {line.x1} {line.y1} H {(line.x1 + line.x2) / 2} V {line.y2} H {Math.max(line.x2 - 2, line.x1)}"
                  marker-end="url(#dep-arrow)"
                />
              {/each}
            </svg>
            {#each scheduled as task (task.id)}
              <div class="chart-row" style="height: {ROW_H}px">
                <a
                  class="bar"
                  class:due-only={!task.start_at && Boolean(task.due_at)}
                  class:done={isDone(task)}
                  class:overdue={isOverdue(task)}
                  href={`/tasks/${task.task_key}`}
                  style="left: {barLeft(task)}px; width: {barWidth(task)}px"
                  title={`${task.task_key} ${task.start_at ? fmtShort(task.start_at) : ''}${task.start_at && task.due_at ? ' → ' : ''}${task.due_at ? fmtShort(task.due_at) : ''}`}
                >
                  <span>{task.title}</span>
                </a>
              </div>
            {/each}
          </div>
        </div>
      </div>
    </div>
  </section>
  {#if unscheduled.length}
    <section class="workspace-card unscheduled">
      <h2>未排期任务({unscheduled.length})</h2>
      <p>以下任务没有设置开始或截止时间,不参与甘特排布。</p>
      <div class="unscheduled-list">
        {#each unscheduled as task (task.id)}
          <a href={`/tasks/${task.task_key}`}>
            <code>{task.task_key}</code>
            <span>{task.title}</span>
          </a>
        {/each}
      </div>
    </section>
  {/if}
{:else}
  <section class="workspace-card state-box">
    <strong>还没有可排期的任务</strong>
    <p>给任务设置开始或截止时间后,会以横条形式出现在时间线上。</p>
  </section>
{/if}

<style>
  .gantt { padding: 0; overflow: hidden; }
  .gantt-scroll { overflow: auto; max-height: 70vh; }
  .gantt-inner { position: relative; }
  .axis-row { position: sticky; top: 0; z-index: 3; height: 26px; background: var(--color-surface); border-bottom: 1px solid var(--color-border); }
  .axis-row span { position: absolute; top: 5px; left: 4px; color: var(--color-text-muted); font-size: 11px; font-family: var(--font-mono); white-space: nowrap; }
  .gantt-body { display: flex; align-items: flex-start; }
  .label-col { position: sticky; left: 0; z-index: 4; background: var(--color-surface); border-right: 1px solid var(--color-border); }
  .row-label { display: grid; grid-template-columns: 84px minmax(0, 1fr); align-items: center; gap: 8px; padding: 0 10px 0 14px; color: var(--color-text); text-decoration: none; font-size: 12px; }
  .row-label:hover span { color: var(--color-primary); }
  .row-label code { font-family: var(--font-mono); font-size: 11px; color: var(--color-text-muted); }
  .row-label span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chart-col { position: relative; --day-w: 32px; background-image: repeating-linear-gradient(to right, var(--color-border-weak) 0 1px, transparent 1px var(--day-w)); }
  .chart-row { position: relative; }
  .bar { position: absolute; top: 9px; height: 22px; display: flex; align-items: center; padding: 0 8px; border-radius: var(--radius-sm); background: var(--color-primary-soft); color: var(--color-primary-strong); font-size: 12px; text-decoration: none; white-space: nowrap; overflow: hidden; }
  .bar:hover { filter: brightness(1.05); }
  .bar.done { background: var(--color-hover); color: var(--color-text-muted); }
  .bar.overdue { background: var(--color-danger); color: #fff; }
  .bar.due-only { width: 14px !important; padding: 0; transform: rotate(45deg) scale(0.9); border-radius: 3px; }
  .bar.due-only span { display: none; }
  .dep-overlay { position: absolute; top: 0; left: 0; color: var(--color-text-muted); pointer-events: none; }
  .dep-overlay path { fill: none; stroke: currentColor; stroke-width: 1.4; stroke-dasharray: 5 4; }
  .dep-overlay path.line-blocked { stroke: var(--color-danger); color: var(--color-danger); }
  .milestone-line { position: absolute; top: 0; bottom: 0; z-index: 1; width: 0; border-left: 1px dashed var(--color-warning); }
  .milestone-line.reached { border-left-color: var(--color-success); }
  .milestone-line span { position: sticky; top: 30px; display: inline-block; margin: 30px 0 0 4px; max-width: 120px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-warning); font-size: 11px; font-weight: 500; }
  .milestone-line.reached span { color: var(--color-success); }
  .unscheduled h2 { margin: 0 0 4px; font-size: 15px; }
  .unscheduled p { margin: 0 0 10px; color: var(--color-text-muted); font-size: 13px; }
  .unscheduled-list { display: grid; }
  .unscheduled-list a { display: grid; grid-template-columns: 110px minmax(0, 1fr); gap: 10px; padding: 8px 0; border-top: 1px solid var(--color-border); color: var(--color-text); font-size: 13px; text-decoration: none; }
  .unscheduled-list a:hover span { color: var(--color-primary); }
  .unscheduled-list code { font-family: var(--font-mono); font-size: 12px; color: var(--color-text-muted); }
  .error-state { color: var(--color-danger); margin-bottom: 16px; }
  .state-box { text-align: center; color: var(--color-text-muted); }
</style>
