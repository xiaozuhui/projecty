<script lang="ts">
  import { ApiClientError } from '$lib/api/client';
  import { listProjectMembers, listStatuses } from '$lib/api/projects';
  import { createTask, listTasks, moveTask } from '$lib/api/tasks';
  import type { ProjectMember, ProjectStatus, TaskView } from '$lib/api/types';
  import { meStore } from '$lib/features/auth/me.svelte';
  import BoardColumn from './BoardColumn.svelte';

  interface Props {
    projectKey: string;
  }

  let { projectKey }: Props = $props();

  let statuses = $state<ProjectStatus[]>([]);
  let tasks = $state<TaskView[]>([]);
  let members = $state<ProjectMember[]>([]);
  let loading = $state(true);
  let errorMessage = $state('');
  let draggingId = $state<string | null>(null);
  let dropTarget = $state<{ statusId: string; index: number } | null>(null);

  // 流转豁免与后端一致:超管/项目管理员不受负责人、评审人限制。
  const exempt = $derived.by(() => {
    const me = meStore.current;
    const myRole = members.find((member) => member.user_id === me?.id)?.role;
    return meStore.isAdmin || myRole === 'manager';
  });

  const ordered = (statusId: string) =>
    [...tasks.filter((task) => task.status_id === statusId)]
      .sort((left, right) => left.position - right.position || left.task_number - right.task_number);

  // 子任务卡片需要显示父任务 Key,从当前列视图里反查。
  const parentKeyOf = (task: TaskView) =>
    task.parent_task_id ? (tasks.find((item) => item.id === task.parent_task_id)?.task_key ?? null) : null;

  export async function reload() {
    loading = true;
    errorMessage = '';
    try {
      const [statusResponse, taskResponse, memberResponse] = await Promise.all([
        listStatuses(projectKey),
        listTasks(projectKey, 1, 100),
        listProjectMembers(projectKey)
      ]);
      statuses = statusResponse.data;
      tasks = taskResponse.data.items;
      members = memberResponse.data.items;
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
    const columnIds = ordered(statusId).map((item) => item.id);
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
    <section class="board-columns">
      {#each statuses as status (status.id)}
        <BoardColumn
          {status}
          tasks={ordered(status.id)}
          {dropTarget}
          {draggingId}
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
