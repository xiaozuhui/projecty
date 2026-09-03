<script lang="ts">
  import { ApiClientError } from '$lib/api/client';
  import { listProjectMembers, listStatuses } from '$lib/api/projects';
  import { listMilestones } from '$lib/api/milestones';
  import { createTask, listTasks, moveTask } from '$lib/api/tasks';
  import type { Milestone, ProjectMember, ProjectStatus, TaskView } from '$lib/api/types';
  import { meStore } from '$lib/features/auth/me.svelte';
  import BoardColumn from './BoardColumn.svelte';

  interface Props {
    projectKey: string;
  }

  let { projectKey }: Props = $props();

  let statuses = $state<ProjectStatus[]>([]);
  let tasks = $state<TaskView[]>([]);
  let members = $state<ProjectMember[]>([]);
  let milestones = $state<Milestone[]>([]);
  let loading = $state(true);
  let errorMessage = $state('');
  let draggingId = $state<string | null>(null);
  // 落点以「插到哪张卡之前」表达:null = 插到列尾。被过滤隐藏的卡片不参与,
  // 位置换算始终基于完整列序,与后端重编号一致。
  let dropTarget = $state<{ statusId: string; beforeTaskId: string | null } | null>(null);
  let groupMode = $state<'none' | 'assignee' | 'label' | 'milestone'>('none');
  let keyword = $state('');
  let assigneeFilter = $state('');
  let labelFilter = $state('');

  const groupOptions: { value: 'none' | 'assignee' | 'label' | 'milestone'; label: string }[] = [
    { value: 'none', label: '不分组' },
    { value: 'assignee', label: '按负责人' },
    { value: 'label', label: '按标签' },
    { value: 'milestone', label: '按里程碑' }
  ];

  // 泳道分组:标签取首个,无值任务归「未分组」。
  const groupOf = $derived.by<((task: TaskView) => string) | null>(() => {
    switch (groupMode) {
      case 'assignee':
        return (task) => task.assignee_name ?? '未分配';
      case 'label':
        return (task) => task.labels[0]?.name ?? '未分组';
      case 'milestone':
        return (task) =>
          milestones.find((milestone) => milestone.id === task.milestone_id)?.name ?? '未关联里程碑';
      default:
        return null;
    }
  });

  // 客户端筛选:只作用于渲染,完整 tasks 保留用于落点换算与计数。
  const filtered = $derived.by(() => {
    const kw = keyword.trim().toLowerCase();
    return tasks.filter((task) => {
      if (kw && !task.title.toLowerCase().includes(kw) && !task.task_key.toLowerCase().includes(kw)) return false;
      if (assigneeFilter === 'none') {
        if (task.assignee_id) return false;
      } else if (assigneeFilter && task.assignee_id !== assigneeFilter) return false;
      if (labelFilter && !task.labels.some((label) => label.id === labelFilter)) return false;
      return true;
    });
  });

  const boardLabels = $derived.by(() => {
    const seen = new Map<string, string>();
    for (const task of tasks) for (const label of task.labels) if (!seen.has(label.id)) seen.set(label.id, label.name);
    return [...seen].map(([id, name]) => ({ id, name }));
  });

  const isOverdueTask = (task: TaskView) => Boolean(task.due_at && new Date(task.due_at) < new Date());
  // 页头统计基于全量(未过滤)任务,筛选变化不影响计数。
  const stats = $derived.by(() => {
    const byCategory = new Map<string, number>();
    for (const task of tasks) byCategory.set(task.status_id, (byCategory.get(task.status_id) ?? 0) + 1);
    let inProgress = 0;
    let done = 0;
    for (const status of statuses) {
      const count = byCategory.get(status.id) ?? 0;
      if (status.category === 'in_progress') inProgress += count;
      if (status.category === 'done') done += count;
    }
    return { total: tasks.length, inProgress, done, overdue: tasks.filter(isOverdueTask).length };
  });

  // Svelte 5 不允许导出派生值,宿主页面经由此函数读取(模板内调用仍保持响应)。
  export function getStats() {
    return stats;
  }

  // 流转豁免与后端一致:超管/项目管理员不受负责人、评审人限制。
  const exempt = $derived.by(() => {
    const me = meStore.current;
    const myRole = members.find((member) => member.user_id === me?.id)?.role;
    return meStore.isAdmin || myRole === 'manager';
  });

  const ordered = (statusId: string) =>
    [...filtered.filter((task) => task.status_id === statusId)]
      .sort((left, right) => left.position - right.position || left.task_number - right.task_number);

  // 子任务卡片需要显示父任务 Key,从全量任务里反查(父卡可能被筛掉)。
  const parentKeyOf = (task: TaskView) =>
    task.parent_task_id ? (tasks.find((item) => item.id === task.parent_task_id)?.task_key ?? null) : null;

  export async function reload() {
    loading = true;
    errorMessage = '';
    try {
      const [statusResponse, taskResponse, memberResponse, milestoneResponse] = await Promise.all([
        listStatuses(projectKey),
        listTasks(projectKey, 1, 100),
        listProjectMembers(projectKey),
        listMilestones(projectKey)
      ]);
      statuses = statusResponse.data;
      tasks = taskResponse.data.items;
      members = memberResponse.data.items;
      milestones = milestoneResponse.data.items;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '看板加载失败';
    } finally {
      loading = false;
    }
  }

  function dragCardStart(event: DragEvent, task: TaskView) {
    draggingId = task.id;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
      event.dataTransfer.setData('text/plain', task.task_key);
    }
  }

  function dragCardEnd() {
    draggingId = null;
    dropTarget = null;
  }

  // 落点换算用完整列序:先去掉被拖卡得插入位,再在剩余序列里找 beforeTaskId;
  // beforeTaskId 为 null 表示插到列尾(完整列序的末尾,而非可见末尾)。
  async function drop(statusId: string, beforeTaskId: string | null) {
    const taskId = draggingId;
    dragCardEnd();
    if (!taskId) return;
    const task = tasks.find((item) => item.id === taskId);
    if (!task || task.status_id === statusId && beforeTaskId === taskId) return;
    const withoutDragged = [...tasks.filter((item) => item.status_id === statusId)]
      .sort((left, right) => left.position - right.position || left.task_number - right.task_number)
      .map((item) => item.id)
      .filter((id) => id !== taskId);
    const index = beforeTaskId ? withoutDragged.indexOf(beforeTaskId) : withoutDragged.length;
    await commitMove(task, statusId, withoutDragged, index < 0 ? withoutDragged.length : index);
  }

  async function commitMove(task: TaskView, statusId: string, columnIds: string[], index: number) {
    const snapshot = tasks;
    const finalOrder = [...columnIds];
    finalOrder.splice(index, 0, task.id);
    // 乐观更新:两列顺序即服务端重编号结果,失败整体回滚。
    tasks = tasks.map((item) => {
      if (item.id === task.id) return { ...item, status_id: statusId, position: finalOrder.indexOf(item.id) };
      if (finalOrder.includes(item.id)) return { ...item, position: finalOrder.indexOf(item.id) };
      return item;
    });
    try {
      await moveTask(task.task_key, statusId, index);
      errorMessage = '';
    } catch (error) {
      tasks = snapshot;
      errorMessage = error instanceof ApiClientError ? error.message : '任务移动失败,已还原';
    }
  }

  // 拖拽是状态流转入口之一:非负责人非评审人的普通成员禁拖,后端校验兜底。
  const canDrag = (task: TaskView) =>
    exempt || task.reviewer_id === meStore.current?.id || task.assignee_id === meStore.current?.id;

  // 完成列新建任务需要评审人身份,而新任务还没有评审人,仅对豁免角色开放。
  const canQuickAdd = (status: ProjectStatus) => exempt || status.category !== 'done';

  async function quickAdd(statusId: string, title: string) {
    try {
      const created = (await createTask(projectKey, { title, status_id: statusId })).data;
      tasks = [...tasks, created];
      errorMessage = '';
      return true;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务创建失败';
      return false;
    }
  }

  // 初次加载由宿主页面的 bindReload 触发,这里不再挂 onMount 以免重复请求。
</script>

{#if loading}
  <div class="state-box">正在加载看板…</div>
{:else}
  {#if errorMessage}<div class="board-error" role="alert">{errorMessage}</div>{/if}
  {#if statuses.length}
    <div class="board-toolbar">
      <form onsubmit={(event) => event.preventDefault()}>
        <input class="search-input" bind:value={keyword} placeholder="搜索标题或编号" aria-label="看板搜索" />
      </form>
      <select bind:value={assigneeFilter} aria-label="按负责人筛选">
        <option value="">负责人:全部</option>
        <option value="none">未分配</option>
        {#each members as member}
          <option value={member.user_id}>{member.display_name}</option>
        {/each}
      </select>
      <select bind:value={labelFilter} aria-label="按标签筛选">
        <option value="">标签:全部</option>
        {#each boardLabels as label (label.id)}
          <option value={label.id}>{label.name}</option>
        {/each}
      </select>
      <span class="flex-fill"></span>
      <label class="group-label">
        泳道分组
        <select bind:value={groupMode} aria-label="看板分组方式">
          {#each groupOptions as option}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      </label>
    </div>
    <section class="board-columns">
      {#each statuses as status (status.id)}
        <BoardColumn
          {status}
          tasks={ordered(status.id)}
          {dropTarget}
          {draggingId}
          {groupOf}
          candrag={canDrag}
          canquickadd={canQuickAdd(status)}
          ondragcardstart={dragCardStart}
          ondragcardend={dragCardEnd}
          onover={(statusId, beforeTaskId) => (dropTarget = { statusId, beforeTaskId })}
          onleave={(statusId) => { if (dropTarget?.statusId === statusId) dropTarget = null; }}
          ondrop={drop}
          onquickadd={quickAdd}
          {parentKeyOf}
        />
      {/each}
    </section>
  {:else}
    <div class="state-box">当前项目还没有状态。</div>
  {/if}
{/if}

<style>
  .board-columns {
    display: flex;
    gap: 12px;
    overflow-x: auto;
    padding-bottom: 8px;
    align-items: flex-start;
  }
  .board-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 14px;
  }
  .board-toolbar select, .search-input {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface);
    color: var(--color-text-secondary);
    font-size: 13px;
  }
  .search-input { width: 180px; }
  .search-input:focus-visible, .board-toolbar select:focus-visible {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: var(--color-focus-ring);
  }
  .group-label {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--color-text-muted);
    font-size: 12px;
    font-weight: 500;
  }
  .flex-fill { flex: 1; }
  .board-error {
    margin-bottom: 12px;
    padding: 8px 12px;
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-md);
    color: var(--color-danger);
    font-size: 13px;
  }
  .state-box { display: grid; place-items: center; min-height: 220px; color: var(--color-text-muted); }
</style>
