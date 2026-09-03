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
  let dropTarget = $state<{ statusId: string; index: number } | null>(null);
  let groupMode = $state<'none' | 'assignee' | 'label' | 'milestone'>('none');

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

  // 流转豁免与后端一致:超管/项目管理员不受负责人、评审人限制。
  const exempt = $derived.by(() => {
    const me = meStore.current;
    const myRole = members.find((member) => member.user_id === me?.id)?.role;
    return meStore.isAdmin || myRole === 'manager';
  });

  const ordered = (statusId: string) =>
    [...tasks.filter((task) => task.status_id === statusId)]
      .sort((left, right) => left.position - right.position || left.task_number - right.task_number);

  // 列内卡片 id 的视觉顺序:分组时按泳道重排(与 BoardColumn 的渲染顺序一致),
  // 拖拽落点换算必须用它,否则分组模式下插错位置。
  const visualColumnIds = (statusId: string) => {
    const list = ordered(statusId);
    if (!groupOf) return list.map((task) => task.id);
    const order: string[] = [];
    const buckets = new Map<string, TaskView[]>();
    for (const task of list) {
      const group = groupOf(task);
      if (!buckets.has(group)) {
        buckets.set(group, []);
        order.push(group);
      }
      buckets.get(group)!.push(task);
    }
    return order.flatMap((group) => buckets.get(group)!.map((task) => task.id));
  };

  // 子任务卡片需要显示父任务 Key,从当前列视图里反查。
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

  // 渲染列表仍含被拖卡:落到拖拽卡之后的下标需减一,换算为“去掉被拖卡”的插入位。
  async function drop(statusId: string, renderedIndex: number) {
    const taskId = draggingId;
    dragCardEnd();
    if (!taskId) return;
    const task = tasks.find((item) => item.id === taskId);
    if (!task) return;
    const columnIds = visualColumnIds(statusId);
    let index = renderedIndex;
    if (task.status_id === statusId) {
      const dragIndex = columnIds.indexOf(taskId);
      if (dragIndex >= 0 && index > dragIndex) index -= 1;
    }
    const withoutDragged = columnIds.filter((id) => id !== taskId);
    index = Math.min(Math.max(0, index), withoutDragged.length);
    await commitMove(task, statusId, withoutDragged, index);
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
  <div class="workspace-card state-box">正在加载看板…</div>
{:else}
  {#if errorMessage}<div class="board-error" role="alert">{errorMessage}</div>{/if}
  {#if statuses.length}
    <div class="board-toolbar">
      <label>
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
          onover={(statusId, index) => (dropTarget = { statusId, index })}
          onleave={(statusId) => { if (dropTarget?.statusId === statusId) dropTarget = null; }}
          ondrop={drop}
          onquickadd={quickAdd}
          {parentKeyOf}
        />
      {/each}
    </section>
  {:else}
    <div class="workspace-card state-box">当前项目还没有状态。</div>
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
    justify-content: flex-end;
    margin-bottom: 12px;
  }
  .board-toolbar label {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--color-text-muted);
    font-size: 12px;
    font-weight: 500;
  }
  .board-toolbar select {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface);
    color: var(--color-text);
    font-size: 13px;
  }
  .board-error {
    margin-bottom: 12px;
    padding: 8px 12px;
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-md);
    color: var(--color-danger);
    font-size: 13px;
  }
  .state-box { text-align: center; color: var(--color-text-muted); }
</style>
