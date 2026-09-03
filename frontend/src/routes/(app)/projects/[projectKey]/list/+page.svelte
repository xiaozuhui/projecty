<script lang="ts">
  import { page } from '$app/state';
  import Modal from '$lib/components/Modal.svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import PriorityPill from '$lib/components/PriorityPill.svelte';
  import TaskTypePill from '$lib/components/TaskTypePill.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import LabelPill from '$lib/components/LabelPill.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { listProjectMembers, listStatuses } from '$lib/api/projects';
  import { listMilestones } from '$lib/api/milestones';
  import { createTask, deleteTask, downloadTaskExport, listDeletedTasks, listLabels, listTasks, restoreTask } from '$lib/api/tasks';
  import type { DeletedTaskItem, LabelView, Milestone, ProjectMember, ProjectStatus, TaskView } from '$lib/api/types';
  import MemberPicker from '$lib/features/task-list/MemberPicker.svelte';
  import { meStore } from '$lib/features/auth/me.svelte';
  import { confirmDialog } from '$lib/features/ui/dialog.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';
  import { taskTypeOptions } from '$lib/features/task-types';

  const projectKey = $derived(String(page.params.projectKey ?? ''));

  let tasks = $state<TaskView[]>([]);
  let rootTasks = $state<TaskView[]>([]);
  let statuses = $state<ProjectStatus[]>([]);
  let members = $state<ProjectMember[]>([]);
  let labels = $state<LabelView[]>([]);
  let milestones = $state<Milestone[]>([]);
  let currentPage = $state(1);
  let total = $state(0);
  let hasMore = $state(false);
  let loading = $state(true);
  let submitting = $state(false);
  let showCreate = $state(false);
  let moreFiltersOpen = $state(false);
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
  let keywordFilter = $state('');
  let assigneeFilter = $state('');
  let priorityFilter = $state('');
  let milestoneFilter = $state('');
  let labelFilter = $state('');
  let overdueFilter = $state(false);
  let errorMessage = $state('');
  let deletingId = $state<string | null>(null);
  let recycleOpen = $state(false);
  let recycledTasks = $state<DeletedTaskItem[]>([]);
  let recycleLoading = $state(false);
  let restoringKey = $state<string | null>(null);
  let exporting = $state(false);

  async function openRecycle() {
    recycleOpen = true;
    recycleLoading = true;
    try {
      recycledTasks = (await listDeletedTasks(projectKey)).data.items;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '回收站加载失败';
    } finally {
      recycleLoading = false;
    }
  }

  async function restoreFromRecycle(item: DeletedTaskItem) {
    restoringKey = item.task_key;
    errorMessage = '';
    try {
      await restoreTask(item.task_key);
      recycledTasks = recycledTasks.filter((row) => row.id !== item.id);
      await load(1);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务恢复失败';
    } finally {
      restoringKey = null;
    }
  }

  async function exportCsv() {
    exporting = true;
    errorMessage = '';
    try {
      const blob = await downloadTaskExport(projectKey, {
        statusId: statusFilter || undefined,
        parentTaskId: parentFilter || undefined,
        taskType: taskTypeFilter || undefined,
        keyword: keywordFilter.trim() || undefined,
        assigneeId: assigneeFilter && assigneeFilter !== 'none' ? assigneeFilter : undefined,
        unassigned: assigneeFilter === 'none',
        priority: priorityFilter || undefined,
        milestoneId: milestoneFilter || undefined,
        labelId: labelFilter || undefined,
        overdue: overdueFilter
      });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `${projectKey}-tasks.csv`;
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '导出失败';
    } finally {
      exporting = false;
    }
  }

  const statusName = (id: string) => statuses.find((status) => status.id === id)?.name || id.slice(0, 8);
  const statusCategory = (id: string) => statuses.find((status) => status.id === id)?.category;
  const memberName = (id: string) => members.find((member) => member.user_id === id)?.display_name ?? id.slice(0, 8);
  const milestoneName = (id: string) => milestones.find((milestone) => milestone.id === id)?.name ?? id.slice(0, 8);
  const labelName = (id: string) => labels.find((label) => label.id === id)?.name ?? id.slice(0, 8);
  const rootTaskLabel = (id: string) => {
    const root = rootTasks.find((task) => task.id === id);
    return root ? root.task_key : id.slice(0, 8);
  };
  const priorityName: Record<string, string> = { urgent: '紧急', high: '高', medium: '中', low: '低', none: '无' };
  const taskTypeLabel = (value: string) => taskTypeOptions.find((option) => option.value === value)?.label ?? value;
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
  const isOverdue = (task: TaskView) => Boolean(task.due_at && statusCategory(task.status_id) !== 'done' && new Date(task.due_at) < new Date());

  // 生效筛选以 chips 回显,单项 × 取消;任何一个筛选在用时,「更多筛选」常开避免藏条件。
  const activeChips = $derived.by(() => {
    const chips: { key: string; label: string; clear: () => void }[] = [];
    if (keywordFilter.trim()) chips.push({ key: 'kw', label: `关键词:${keywordFilter.trim()}`, clear: () => { keywordFilter = ''; applyFilters(); } });
    if (statusFilter) chips.push({ key: 'status', label: `状态:${statusName(statusFilter)}`, clear: () => { statusFilter = ''; applyFilters(); } });
    if (assigneeFilter) chips.push({ key: 'assignee', label: assigneeFilter === 'none' ? '负责人:未分配' : `负责人:${memberName(assigneeFilter)}`, clear: () => { assigneeFilter = ''; applyFilters(); } });
    if (priorityFilter) chips.push({ key: 'priority', label: `优先级:${priorityName[priorityFilter] ?? priorityFilter}`, clear: () => { priorityFilter = ''; applyFilters(); } });
    if (taskTypeFilter) chips.push({ key: 'type', label: `类型:${taskTypeLabel(taskTypeFilter)}`, clear: () => { taskTypeFilter = ''; applyFilters(); } });
    if (milestoneFilter) chips.push({ key: 'milestone', label: `里程碑:${milestoneName(milestoneFilter)}`, clear: () => { milestoneFilter = ''; applyFilters(); } });
    if (labelFilter) chips.push({ key: 'label', label: `标签:${labelName(labelFilter)}`, clear: () => { labelFilter = ''; applyFilters(); } });
    if (parentFilter) chips.push({ key: 'parent', label: `父任务:${rootTaskLabel(parentFilter)}`, clear: () => { parentFilter = ''; applyFilters(); } });
    if (overdueFilter) chips.push({ key: 'overdue', label: '仅看逾期', clear: () => { overdueFilter = false; applyFilters(); } });
    return chips;
  });

  async function load(targetPage = 1) {
    loading = true;
    errorMessage = '';
    try {
      const [taskResponse, statusResponse, rootResponse, memberResponse, labelResponse, milestoneResponse] = await Promise.all([
        listTasks(projectKey, targetPage, 20, {
          statusId: statusFilter || undefined,
          parentTaskId: parentFilter || undefined,
          taskType: taskTypeFilter || undefined,
          keyword: keywordFilter.trim() || undefined,
          assigneeId: assigneeFilter && assigneeFilter !== 'none' ? assigneeFilter : undefined,
          unassigned: assigneeFilter === 'none',
          priority: priorityFilter || undefined,
          milestoneId: milestoneFilter || undefined,
          labelId: labelFilter || undefined,
          overdue: overdueFilter
        }),
        listStatuses(projectKey),
        listTasks(projectKey, 1, 100, { parentTaskId: 'none' }),
        listProjectMembers(projectKey),
        listLabels(projectKey),
        listMilestones(projectKey)
      ]);
      tasks = taskResponse.data.items;
      total = taskResponse.data.total;
      currentPage = taskResponse.data.page;
      hasMore = taskResponse.data.has_more;
      statuses = statusResponse.data;
      rootTasks = rootResponse.data.items;
      members = memberResponse.data.items;
      labels = labelResponse.data;
      milestones = milestoneResponse.data.items;
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
    keywordFilter = '';
    assigneeFilter = '';
    priorityFilter = '';
    milestoneFilter = '';
    labelFilter = '';
    overdueFilter = false;
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
        message: `确定删除 ${task.task_key}「${task.title}」吗？删除后可在回收站恢复。`,
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

<header class="page-head">
  <nav class="breadcrumb" aria-label="项目路径">
    <a href="/projects">项目</a><span>/</span>
    <a href={`/projects/${projectKey}`}>{projectKey}</a><span>/</span>
    <span>任务列表</span>
  </nav>
  <div class="title-row">
    <h1>任务</h1>
    <div class="header-actions">
      <button class="tool-btn" type="button" onclick={openRecycle}>回收站</button>
      <button class="tool-btn" type="button" onclick={exportCsv} disabled={exporting}>
        {exporting ? '导出中…' : '导出 CSV'}
      </button>
      <button class="primary-button" type="button" onclick={() => (showCreate = !showCreate)}>
        {showCreate ? '收起表单' : '＋ 新建任务'}
      </button>
    </div>
  </div>
  <div class="meta-row">
    <span class="meta-item">共 {total} 项</span><span class="sep">·</span>
    <span class="meta-item">第 {currentPage} 页</span><span class="sep">·</span>
    <span class="meta-item">筛选结果按当前条件导出</span>
  </div>
</header>

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

<div class="filter-bar">
  <form onsubmit={(event) => { event.preventDefault(); applyFilters(); }}>
    <input class="search-input" bind:value={keywordFilter} placeholder="搜索标题或编号" aria-label="按关键词筛选" />
  </form>
  <select bind:value={statusFilter} onchange={applyFilters} aria-label="按状态筛选">
    <option value="">状态:全部</option>
    {#each statuses as status}
      <option value={status.id}>{status.name}</option>
    {/each}
  </select>
  <select bind:value={assigneeFilter} onchange={applyFilters} aria-label="按负责人筛选">
    <option value="">负责人:全部</option>
    <option value="none">未分配</option>
    {#each members as member}
      <option value={member.user_id}>{member.display_name}</option>
    {/each}
  </select>
  <select bind:value={priorityFilter} onchange={applyFilters} aria-label="按优先级筛选">
    <option value="">优先级:全部</option>
    <option value="urgent">紧急</option>
    <option value="high">高</option>
    <option value="medium">中</option>
    <option value="low">低</option>
    <option value="none">无</option>
  </select>
  <button class="more-toggle" type="button" class:open={moreFiltersOpen} onclick={() => (moreFiltersOpen = !moreFiltersOpen)}>
    更多筛选 ⌄
  </button>
  <span class="flex-fill"></span>
  {#if activeChips.length}
    <span class="filter-count">已选 {activeChips.length} 项</span>
    <button class="clear-all" type="button" onclick={clearFilters} disabled={loading}>清空</button>
  {/if}
</div>
{#if moreFiltersOpen}
  <div class="filter-bar more">
    <select bind:value={taskTypeFilter} onchange={applyFilters} aria-label="按类型筛选">
      <option value="">类型:全部</option>
      {#each taskTypeOptions as option}<option value={option.value}>{option.label}</option>{/each}
    </select>
    <select bind:value={milestoneFilter} onchange={applyFilters} aria-label="按里程碑筛选">
      <option value="">里程碑:全部</option>
      {#each milestones as milestone}
        <option value={milestone.id}>{milestone.name}</option>
      {/each}
    </select>
    <select bind:value={labelFilter} onchange={applyFilters} aria-label="按标签筛选">
      <option value="">标签:全部</option>
      {#each labels as label}
        <option value={label.id}>{label.name}</option>
      {/each}
    </select>
    <select bind:value={parentFilter} onchange={applyFilters} aria-label="按父任务筛选">
      <option value="">父任务:全部</option>
      {#each rootTasks as root}
        <option value={root.id} title={root.title}>{root.task_key}</option>
      {/each}
    </select>
    <button class="filter-chip" class:active={overdueFilter} type="button" onclick={() => { overdueFilter = !overdueFilter; applyFilters(); }}>
      仅看逾期
    </button>
  </div>
{/if}
{#if activeChips.length}
  <div class="chip-row">
    {#each activeChips as chip (chip.key)}
      <span class="filter-tag">
        {chip.label}
        <button type="button" aria-label={`取消筛选 ${chip.label}`} onclick={chip.clear}>×</button>
      </span>
    {/each}
  </div>
{/if}

{#if errorMessage}
  <p class="error-message" role="alert">{errorMessage}</p>
{/if}

{#if loading}
  <div class="state-box">正在加载任务…</div>
{:else if !tasks.length}
  <div class="empty-panel">
    <strong>没有匹配的任务</strong>
    <p>可以清除筛选，或创建一个根任务开始推进项目。</p>
  </div>
{:else}
  <section class="list-panel">
    <div class="table-head" role="row">
      <span>编号</span><span>标题</span><span>状态</span><span>负责人</span><span>优先级</span><span>截止</span><span></span>
    </div>
    {#each tasks as task (task.id)}
      <div class="task-row" class:subtask={task.parent_task_id}>
        <a class="task-key" href={`/tasks/${task.task_key}`} title={parentKeyOf(task) ? `父任务 ${parentKeyOf(task)}` : task.task_key}>
          {#if task.parent_task_id}↳ {/if}{task.task_key}
        </a>
        <a class="task-title" href={`/tasks/${task.task_key}`}>
          <span class="title-text">{task.title}</span>
          {#if !task.parent_task_id && task.subtask_total}
            <span class="subtask-progress">{task.subtask_done}/{task.subtask_total}</span>
          {/if}
          {#if task.labels.length}
            <span class="title-labels">
              {#each task.labels.slice(0, 2) as label (label.id)}
                <LabelPill name={label.name} />
              {/each}
            </span>
          {/if}
        </a>
        <StatusBadge name={statusName(task.status_id)} category={statusCategory(task.status_id)} />
        <span class="cell-assignee">
          {#if task.assignee_name}<Avatar name={task.assignee_name} size={18} />{task.assignee_name}{:else}<span class="unassigned">未分配</span>{/if}
        </span>
        <span class="cell-priority"><PriorityPill priority={task.priority} /></span>
        <span class="cell-due" class:danger={isOverdue(task)}>
          {#if task.due_at}{new Date(task.due_at).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })}{:else}—{/if}
        </span>
        <button
          class="row-delete"
          type="button"
          disabled={deletingId === task.id}
          onclick={() => removeTask(task)}
          title="逻辑删除,可在回收站恢复"
        >
          {deletingId === task.id ? '删除中…' : '删除'}
        </button>
      </div>
    {/each}
  </section>

  <div class="pager">
    <button class="secondary-button" type="button" disabled={loading || currentPage <= 1} onclick={() => load(currentPage - 1)}>
      上一页
    </button>
    <button class="secondary-button" type="button" disabled={loading || !hasMore} onclick={() => load(currentPage + 1)}>
      下一页
    </button>
  </div>
{/if}

<Modal open={recycleOpen} title="回收站" onClose={() => (recycleOpen = false)}>
  {#if recycleLoading}
    <p class="recycle-hint">正在加载已删除任务…</p>
  {:else if !recycledTasks.length}
    <p class="recycle-hint">回收站是空的,逻辑删除的任务会在这里保留以便恢复。</p>
  {:else}
    <p class="recycle-hint">恢复后任务回到原状态列;最多展示最近 200 条。</p>
    <div class="recycle-list">
      {#each recycledTasks as item (item.id)}
        <div class="recycle-row">
          <div class="recycle-main">
            <strong>{item.task_key}</strong>
            <span title={item.title}>{item.title}</span>
            <small>
              {item.deleted_at ? new Date(item.deleted_at).toLocaleString('zh-CN') : ''}
              {item.deleted_by_name ? ` · 由 ${item.deleted_by_name} 删除` : ''}
              {item.delete_reason ? ` · 原因:${item.delete_reason}` : ''}
            </small>
          </div>
          <button
            class="text-button recycle-restore"
            type="button"
            disabled={restoringKey === item.task_key}
            onclick={() => restoreFromRecycle(item)}
          >
            {restoringKey === item.task_key ? '恢复中…' : '恢复'}
          </button>
        </div>
      {/each}
    </div>
  {/if}
</Modal>

<style>
  h1, p { margin: 0; }

  .page-head { margin-bottom: 18px; display: grid; gap: 8px; }
  .breadcrumb { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--color-text-muted); }
  .breadcrumb a { color: var(--color-text-muted); }
  .breadcrumb a:hover { color: var(--color-text); text-decoration: none; }
  .title-row { display: flex; align-items: center; gap: 12px; }
  .title-row h1 { flex: 1; font-size: 22px; font-weight: 600; line-height: 1.35; }
  .header-actions { display: flex; gap: 8px; }
  .tool-btn { padding: 6px 14px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface); color: var(--color-text-secondary); font-size: 12px; font-weight: 500; transition: border-color var(--transition-fast), color var(--transition-fast); }
  .tool-btn:hover { border-color: var(--color-border-strong); color: var(--color-text); }
  .tool-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .header-actions .primary-button { border: 0; }
  .meta-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; font-size: 13px; color: var(--color-text-muted); }
  .meta-item { display: inline-flex; align-items: center; }
  .sep { color: var(--color-border); }

  .create-task {
    display: grid;
    grid-template-columns: minmax(180px, 1.6fr) 110px 140px minmax(150px, 1fr) minmax(150px, 1fr) 140px 140px minmax(180px, 1.4fr) auto;
    gap: 8px;
    margin-bottom: 16px;
    padding: 12px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border-weak);
    border-radius: var(--radius-md);
  }
  .create-task input, .create-task select { min-width: 0; border: 1px solid var(--color-border); border-radius: var(--radius-md); padding: 8px 10px; background: var(--color-surface); color: var(--color-text); }
  .create-task button { border: 0; }

  .filter-bar { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; margin-bottom: 10px; }
  .filter-bar.more { padding: 10px; border: 1px solid var(--color-border-weak); border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .filter-bar select, .search-input { padding: 6px 10px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface); color: var(--color-text-2, var(--color-text-secondary)); color: var(--color-text-secondary); font-size: 13px; }
  .search-input { width: 200px; }
  .filter-bar select:focus-visible, .search-input:focus-visible { outline: none; border-color: var(--color-primary); box-shadow: var(--color-focus-ring); }
  .more-toggle { padding: 6px 10px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: transparent; color: var(--color-text-muted); font-size: 13px; cursor: pointer; }
  .more-toggle:hover { color: var(--color-text-secondary); border-color: var(--color-border-strong); }
  .more-toggle.open { color: var(--color-primary-strong); border-color: var(--color-primary); }
  .flex-fill { flex: 1; }
  .filter-count { font-size: 12px; color: var(--color-text-muted); }
  .clear-all { border: 0; background: transparent; color: var(--color-text-muted); font-size: 12px; cursor: pointer; }
  .clear-all:hover { color: var(--color-danger); }
  .clear-all:disabled { opacity: 0.5; cursor: not-allowed; }

  .filter-chip { display: inline-flex; align-items: center; gap: 5px; padding: 5px 12px; border: 1px solid var(--color-border); border-radius: 999px; background: transparent; color: var(--color-text-muted); font-size: 12px; cursor: pointer; transition: color var(--transition-fast), border-color var(--transition-fast), background-color var(--transition-fast); }
  .filter-chip:hover { color: var(--color-text-secondary); border-color: var(--color-border-strong); }
  .filter-chip.active { background: var(--color-primary-soft); border-color: var(--color-primary); color: var(--color-primary-strong); font-weight: 500; }

  .chip-row { display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 14px; }
  .filter-tag { display: inline-flex; align-items: center; gap: 6px; padding: 3px 10px; border-radius: 999px; background: var(--color-primary-soft); color: var(--color-primary-strong); font-size: 12px; }
  .filter-tag button { border: 0; background: transparent; color: inherit; font-size: 13px; cursor: pointer; opacity: 0.7; padding: 0; }
  .filter-tag button:hover { opacity: 1; }

  .error-message { margin: 0 0 14px; color: var(--color-danger); font-size: 13px; }

  .list-panel { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-lg); overflow: hidden; }
  .table-head, .task-row {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr) 110px 130px 64px 64px 44px;
    gap: 12px;
    align-items: center;
    padding: 9px 14px;
  }
  .table-head { font-size: 12px; color: var(--color-text-muted); border-bottom: 1px solid var(--color-border); }
  .table-head span:last-child { text-align: right; }
  .task-row { font-size: 13px; color: var(--color-text); border-top: 1px solid var(--color-border-weak); transition: background-color var(--transition-fast); }
  .task-row:first-of-type { border-top: 0; }
  .task-row:hover { background: var(--color-hover); }
  .task-row.subtask { padding-left: 30px; }
  .task-key { color: var(--color-text-muted); font-family: var(--font-mono); font-size: 12px; text-decoration: none; white-space: nowrap; }
  .task-row:hover .task-key { color: var(--color-primary-strong); }
  .task-title { min-width: 0; display: flex; align-items: center; gap: 8px; color: var(--color-text); text-decoration: none; font-weight: 500; }
  .title-text { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .task-row:hover .task-title { color: var(--color-primary); }
  .subtask-progress { flex: none; padding: 1px 7px; border-radius: 999px; background: var(--color-hover); color: var(--color-text-muted); font-family: var(--font-mono); font-size: 11px; }
  .title-labels { display: inline-flex; gap: 4px; flex: none; }
  .cell-assignee { display: inline-flex; align-items: center; gap: 6px; color: var(--color-text-muted); font-size: 12px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .unassigned { color: var(--color-text-muted); font-size: 12px; }
  .cell-priority { display: inline-flex; justify-content: center; }
  .cell-due { text-align: right; font-size: 12px; color: var(--color-text-muted); font-family: var(--font-mono); }
  .cell-due.danger { color: var(--color-danger); font-weight: 500; }
  .row-delete { justify-self: end; border: 0; background: transparent; color: var(--color-danger); font-size: 12px; font-weight: 500; cursor: pointer; opacity: 0; border-radius: var(--radius-sm); transition: opacity var(--transition-fast); }
  .task-row:hover .row-delete, .row-delete:focus-visible { opacity: 1; }
  .row-delete:disabled { cursor: not-allowed; opacity: 0.45; }

  .state-box { display: grid; place-items: center; gap: 7px; min-height: 220px; color: var(--color-text-muted); }
  .empty-panel strong { color: var(--color-text-secondary); font-size: 14px; font-weight: 500; }
  .empty-panel { display: grid; place-items: center; gap: 8px; min-height: 220px; border: 1px solid var(--color-border); border-radius: var(--radius-lg); color: var(--color-text-muted); }
  .empty-panel p { font-size: 13px; }

  .pager { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
  .secondary-button:disabled { cursor: not-allowed; opacity: 0.45; }

  .recycle-hint { margin: 0 0 10px; color: var(--color-text-muted); font-size: 12px; }
  .recycle-list { display: grid; }
  .recycle-row { display: flex; align-items: center; gap: 10px; padding: 9px 0; border-top: 1px solid var(--color-border-weak); }
  .recycle-main { display: grid; gap: 3px; flex: 1; min-width: 0; font-size: 13px; }
  .recycle-main strong { font-family: var(--font-mono); font-size: 12px; color: var(--color-primary-strong); font-weight: 500; }
  .recycle-main span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .recycle-main small { color: var(--color-text-muted); font-size: 11px; }
  .recycle-restore { color: var(--color-primary-strong); white-space: nowrap; }
  .text-button { border: 0; padding: 0; background: transparent; font-size: 12px; font-weight: 500; cursor: pointer; }

  @media (max-width: 900px) {
    .filter-bar { align-items: stretch; flex-direction: column; }
    .search-input { width: 100%; }
    .create-task { grid-template-columns: 1fr 1fr; }
    .field-title, .field-desc { grid-column: 1 / -1; }
    .create-task button { grid-column: 1 / -1; }
    .table-head { display: none; }
    .task-row { grid-template-columns: minmax(0, 1fr) auto; row-gap: 6px; }
    .task-key, .cell-priority, .cell-due, .row-delete { display: none; }
  }
</style>
