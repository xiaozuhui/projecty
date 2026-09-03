<script lang="ts">
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { listProjectMembers, listStatuses } from '$lib/api/projects';
  import { createTask, deleteTask, listTasks } from '$lib/api/tasks';
  import type { ProjectMember, ProjectStatus, TaskView } from '$lib/api/types';
  import MemberPicker from '$lib/features/task-list/MemberPicker.svelte';
  import { meStore } from '$lib/features/auth/me.svelte';
  import { confirmDialog } from '$lib/features/ui/dialog.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';
  import TaskTypePill from '$lib/components/TaskTypePill.svelte';
  import { taskTypeOptions } from '$lib/features/task-types';

  const projectKey = $derived(String(page.params.projectKey ?? ''));

  let tasks = $state<TaskView[]>([]);
  let rootTasks = $state<TaskView[]>([]);
  let statuses = $state<ProjectStatus[]>([]);
  let members = $state<ProjectMember[]>([]);
  let currentPage = $state(1);
  let hasMore = $state(false);
  let loading = $state(true);
  let submitting = $state(false);
  let showCreate = $state(false);
  let title = $state('');
  let description = $state('');
  let priority = $state('medium');
  let taskType = $state('feature');
  let assigneeId = $state<string | null>(null);
  let reviewerId = $state<string | null>(null);
  let startAt = $state('');
  let dueAt = $state('');
  let createStatusId = $state('');
  let statusFilter = $state('');
  let parentFilter = $state('');
  let taskTypeFilter = $state('');
  let errorMessage = $state('');
  let deletingId = $state<string | null>(null);

  const statusName = (id: string) => statuses.find((status) => status.id === id)?.name || id.slice(0, 8);
  // 与后端一致:超管/项目管理员豁免流转限制,可把任务直接建在完成列,普通成员下拉不出现该选项。
  const exempt = $derived.by(() => {
    const me = meStore.current;
    const myRole = members.find((member) => member.user_id === me?.id)?.role;
    return meStore.isAdmin || myRole === 'manager';
  });
  const creatableStatuses = $derived(
    exempt ? statuses : statuses.filter((status) => status.category !== 'done')
  );
  // 父任务 Key 反查:优先当前页,再从全量根任务补齐,子任务行据此展示归属。
  const allTasks = $derived([...tasks, ...rootTasks]);
  const parentKeyOf = (task: TaskView) =>
    task.parent_task_id ? (allTasks.find((item) => item.id === task.parent_task_id)?.task_key ?? null) : null;
  const priorityName: Record<string, string> = {
    urgent: '紧急',
    high: '高',
    medium: '中',
    low: '低',
    none: '无'
  };

  async function load(targetPage = 1) {
    loading = true;
    errorMessage = '';
    try {
      const [taskResponse, statusResponse, rootResponse, memberResponse] = await Promise.all([
        listTasks(projectKey, targetPage, 20, {
          statusId: statusFilter || undefined,
          parentTaskId: parentFilter || undefined,
          taskType: taskTypeFilter || undefined
        }),
        listStatuses(projectKey),
        listTasks(projectKey, 1, 100),
        listProjectMembers(projectKey)
      ]);
      tasks = taskResponse.data.items;
      currentPage = taskResponse.data.page;
      hasMore = taskResponse.data.has_more;
      statuses = statusResponse.data;
      rootTasks = rootResponse.data.items.filter((task) => !task.parent_task_id);
      members = memberResponse.data.items;
      if (!createStatusId && statuses[0]) createStatusId = statuses[0].id;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务加载失败';
    } finally {
      loading = false;
    }
  }

  function applyFilters() {
    void load(1);
  }

  function clearFilters() {
    statusFilter = '';
    parentFilter = '';
    taskTypeFilter = '';
    void load(1);
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!title.trim()) {
      errorMessage = '任务标题不能为空';
      return;
    }
    submitting = true;
    errorMessage = '';
    try {
      await createTask(projectKey, {
        title: title.trim(),
        description: description.trim() || undefined,
        priority,
        task_type: taskType,
        status_id: createStatusId || undefined,
        assignee_id: assigneeId,
        reviewer_id: reviewerId,
        start_at: startAt ? new Date(startAt).toISOString() : undefined,
        due_at: dueAt ? new Date(dueAt).toISOString() : undefined
      });
      title = '';
      description = '';
      taskType = 'feature';
      assigneeId = null;
      reviewerId = null;
      startAt = '';
      dueAt = '';
      showCreate = false;
      await load(1);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务创建失败';
    } finally {
      submitting = false;
    }
  }

  async function removeTask(task: TaskView) {
    if (
      !(await confirmDialog({
        title: '逻辑删除任务',
        message: `确定删除 ${task.task_key}「${task.title}」吗？删除后可在操作日志追溯。`,
        confirmLabel: '删除',
        danger: true
      }))
    ) {
      return;
    }
    deletingId = task.id;
    errorMessage = '';
    try {
      await deleteTask(task.task_key, '用户从任务列表页删除任务');
      await load(currentPage);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务删除失败';
    } finally {
      deletingId = null;
    }
  }

  bindReload(() => void load());
</script>

<PageHeader
  title="任务列表"
  eyebrow={projectKey}
  description="根任务与两层子任务统一分页管理，新增任务、删除任务和状态流转都会留下操作日志。"
/>

<section class="workspace-card">
  <div class="toolbar">
    <div>
      <h2>全部任务</h2>
      <p>当前显示第 {currentPage} 页</p>
    </div>
    <button class="primary-button" type="button" onclick={() => (showCreate = !showCreate)}>
      {showCreate ? '收起表单' : '新建任务'}
    </button>
  </div>

  <form class="filters" onsubmit={(event) => { event.preventDefault(); applyFilters(); }}>
    <label>
      状态
      <select bind:value={statusFilter} aria-label="按状态筛选">
        <option value="">全部状态</option>
        {#each statuses as status}
          <option value={status.id}>{status.name}</option>
        {/each}
      </select>
    </label>
    <label>
      父任务
      <select bind:value={parentFilter} aria-label="按父任务筛选">
        <option value="">全部层级</option>
        {#each rootTasks as root}
          <option value={root.id}>{root.task_key} · {root.title}</option>
        {/each}
      </select>
    </label>
    <label>
      类型
      <select bind:value={taskTypeFilter} aria-label="按类型筛选">
        <option value="">全部类型</option>
        {#each taskTypeOptions as option}<option value={option.value}>{option.label}</option>{/each}
      </select>
    </label>
    <button class="secondary-button" type="submit" disabled={loading}>筛选</button>
    <button class="link-button" type="button" onclick={clearFilters} disabled={loading || (!statusFilter && !parentFilter && !taskTypeFilter)}>
      清除筛选
    </button>
  </form>

  {#if showCreate}
    <form class="create-task" onsubmit={submit}>
      <input class="field-title" bind:value={title} placeholder="输入任务标题，例如：完成权限模型评审" aria-label="任务标题" />
      <select bind:value={taskType} aria-label="任务类型">
        {#each taskTypeOptions as option}<option value={option.value}>{option.label}</option>{/each}
      </select>
      <select bind:value={priority} aria-label="优先级">
        <option value="urgent">紧急</option>
        <option value="high">高</option>
        <option value="medium">中</option>
        <option value="low">低</option>
        <option value="none">无</option>
      </select>
      <select bind:value={createStatusId} aria-label="初始状态">
        {#each creatableStatuses as status}
          <option value={status.id}>{status.name}</option>
        {/each}
      </select>
      <MemberPicker value={assigneeId} {members} onchange={(value) => (assigneeId = value)} ariaLabel="负责人" />
      <MemberPicker value={reviewerId} {members} onchange={(value) => (reviewerId = value)} ariaLabel="评审人" />
      <input class="field-due" type="datetime-local" bind:value={startAt} aria-label="开始时间" />
      <input class="field-due" type="datetime-local" bind:value={dueAt} aria-label="截止时间" />
      <input class="field-desc" bind:value={description} placeholder="补充描述（可选）" aria-label="任务描述" />
      <button class="primary-button" type="submit" disabled={submitting}>
        {submitting ? '保存中…' : '创建'}
      </button>
    </form>
  {/if}

  {#if errorMessage}
    <p class="error-message" role="alert">{errorMessage}</p>
  {/if}

  {#if loading}
    <div class="state-box">正在加载任务…</div>
  {:else if tasks.length === 0}
    <div class="state-box">
      <strong>没有匹配的任务</strong>
      <p>可以清除筛选，或创建一个根任务开始推进项目。</p>
    </div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>编号</th>
            <th>标题</th>
            <th>状态</th>
            <th>类型</th>
            <th>优先级</th>
            <th>负责人</th>
            <th>更新时间</th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          {#each tasks as task}
            <tr>
              <td>
                <a class="task-key" href={`/tasks/${task.task_key}`}>{task.task_key}</a>
                {#if task.parent_task_id}
                  <span class="subtask-mark">↳ 父 {parentKeyOf(task) ?? task.parent_task_id.slice(0, 8)}</span>
                {/if}
              </td>
              <td><a class="task-title" href={`/tasks/${task.task_key}`}>{task.title}</a></td>
              <td><span class="status-pill">{statusName(task.status_id)}</span></td>
              <td><TaskTypePill taskType={task.task_type} /></td>
              <td><span class={`priority priority-${task.priority}`}>{priorityName[task.priority]}</span></td>
              <td class="muted">{task.assignee_name ?? '未分配'}</td>
              <td class="muted">
                {new Date(task.updated_at).toLocaleString('zh-CN', {
                  month: 'numeric',
                  day: 'numeric',
                  hour: '2-digit',
                  minute: '2-digit'
                })}
              </td>
              <td>
                <button
                  class="text-button danger"
                  type="button"
                  disabled={deletingId === task.id}
                  onclick={() => removeTask(task)}
                >
                  {deletingId === task.id ? '删除中…' : '删除'}
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  <div class="pager">
    <button class="secondary-button" type="button" disabled={loading || currentPage <= 1} onclick={() => load(currentPage - 1)}>
      上一页
    </button>
    <button class="secondary-button" type="button" disabled={loading || !hasMore} onclick={() => load(currentPage + 1)}>
      下一页
    </button>
  </div>
</section>

<style>
  h2,
  p {
    margin: 0;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 16px;
  }

  .toolbar h2 {
    font-size: 18px;
  }

  .toolbar p {
    margin-top: 4px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .toolbar button,
  .create-task button {
    border: 0;
  }

  .filters {
    display: flex;
    align-items: end;
    gap: 10px;
    margin-bottom: 14px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--color-border);
  }

  .filters label {
    display: grid;
    gap: 6px;
    min-width: 180px;
    color: var(--color-text-muted);
    font-size: 12px;
    font-weight:500;
  }

  .filters select {
    min-width: 0;
    border: 1px solid var(--color-border);
    border-radius:var(--radius-md);
    padding: 9px 10px;
    background: var(--color-surface);
    color: var(--color-text-primary);
  }

  .filters button {
    white-space: nowrap;
  }

  .link-button {
    border: 0;
    padding: 9px 0;
    background: transparent;
    color: var(--color-primary-strong);
    font-weight:500;
    cursor: pointer;
  }

  .link-button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .create-task {
    display: grid;
    grid-template-columns: minmax(180px, 1.6fr) 110px 140px minmax(150px, 1fr) minmax(150px, 1fr) 140px 140px minmax(180px, 1.4fr) auto;
    gap: 8px;
    margin-bottom: 14px;
    padding: 12px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border-weak);
    border-radius: var(--radius-md);
  }

  .create-task input,
  .create-task select {
    min-width: 0;
    border: 1px solid var(--color-border);
    border-radius:var(--radius-md);
    padding: 10px 11px;
    background: var(--color-surface);
  }

  .error-message {
    margin: 0 0 14px;
    color: var(--color-danger);
    font-size: 13px;
  }

  .table-wrap {
    overflow-x: auto;
  }

  table {
    width: 100%;
    min-width: 720px;
    border-collapse: collapse;
  }

  th,
  td {
    padding: 13px 10px;
    border-bottom: 1px solid var(--color-border);
    text-align: left;
  }

  th {
    color: var(--color-text-muted);
    font-size: 12px;
    font-weight:500;
  }

  td {
    font-size: 14px;
  }

  .task-key {
    color: var(--color-primary-strong);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight:500;
  }

  .task-title:hover,
  .task-key:hover {
    color: var(--color-primary);
  }

  .subtask-mark {
    display: block;
    margin-top: 4px;
    color: var(--color-text-muted);
    font-size: 11px;
  }

  .muted {
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .text-button {
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--color-danger);
    font-size: 13px;
    font-weight:500;
    cursor: pointer;
  }

  .text-button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .priority {
    font-size: 13px;
    font-weight:500;
  }

  .priority-urgent,
  .priority-high {
    color: var(--color-danger);
  }

  .priority-medium {
    color: var(--color-warning);
  }

  .priority-low {
    color: var(--color-success);
  }

  .state-box {
    display: grid;
    place-items: center;
    gap: 7px;
    min-height: 220px;
    color: var(--color-text-muted);
  }

  .state-box p {
    font-size: 13px;
  }

  .pager {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }

  .secondary-button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  @media (max-width: 760px) {
    .filters {
      display: grid;
      grid-template-columns: 1fr 1fr;
      align-items: end;
    }

    .filters label {
      min-width: 0;
    }

    .filters button {
      width: 100%;
    }

    .create-task {
      grid-template-columns: 1fr 1fr;
    }

    .field-title,
    .field-desc {
      grid-column: 1 / -1;
    }

    .create-task button {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 480px) {
    .filters {
      grid-template-columns: 1fr;
    }
  }
</style>
