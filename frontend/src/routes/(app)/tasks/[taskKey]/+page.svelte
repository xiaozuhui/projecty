<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { confirmDialog } from '$lib/features/ui/dialog.svelte';
  import { listStatuses, listProjectMembers } from '$lib/api/projects';
  import { listMilestones } from '$lib/api/milestones';
  import { deleteAttachment, listTaskAttachments, uploadTaskAttachment, attachmentUrl } from '$lib/api/attachments';
  import { addDependency, addTaskLabel, createComment, createSubtask, deleteComment, deleteTask, getSubtasks, getTask, listComments, listDependencies, listLabels, listTasks, removeDependency, removeTaskLabel, transitionTask, updateTask } from '$lib/api/tasks';
  import type { Attachment, Comment, LabelView, Milestone, ProjectMember, ProjectStatus, TaskDependencies, TaskView } from '$lib/api/types';
  import MemberPicker from '$lib/features/task-list/MemberPicker.svelte';
  import { meStore } from '$lib/features/auth/me.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';
  import { taskTypeOptions } from '$lib/features/task-types';
  import TaskTypePill from '$lib/components/TaskTypePill.svelte';
  import LabelPill from '$lib/components/LabelPill.svelte';

  const taskKey = $derived(String(page.params.taskKey ?? ''));
  const projectKey = $derived(taskKey.replace(/-\d+$/, ''));
  let task = $state<TaskView | null>(null);
  let comments = $state<Comment[]>([]);
  let attachments = $state<Attachment[]>([]);
  let pending = $state<Attachment[]>([]);
  let commentBody = $state('');
  let subtasks = $state<TaskView[]>([]);
  let statuses = $state<ProjectStatus[]>([]);
  let members = $state<ProjectMember[]>([]);
  let rootTasks = $state<TaskView[]>([]);
  let milestones = $state<Milestone[]>([]);
  let labels = $state<LabelView[]>([]);
  let dependencies = $state<TaskDependencies | null>(null);
  let projectTasks = $state<TaskView[]>([]);
  let selectedStatus = $state('');
  let selectedStartAt = $state('');
  let selectedDueAt = $state('');
  let selectedAssigneeId = $state<string | null>(null);
  let selectedReviewerId = $state<string | null>(null);
  let selectedTaskType = $state('feature');
  let selectedMilestoneId = $state<string | null>(null);
  let labelInput = $state('');
  let dependencyTarget = $state('');
  let savingDetails = $state(false);
  let labelBusy = $state(false);
  let dependencyBusy = $state(false);
  let attachParentId = $state('');
  let showSubtaskModal = $state(false);
  let subtaskTitle = $state('');
  let subtaskDescription = $state('');
  let subtaskAssigneeId = $state<string | null>(null);
  let subtaskReviewerId = $state<string | null>(null);
  let subtaskType = $state('feature');
  let subtaskStartAt = $state('');
  let subtaskDueAt = $state('');
  let subtaskComment = $state('');
  let subtaskImages = $state<{ file: File; url: string }[]>([]);
  let subtaskError = $state('');
  let loading = $state(true);
  let submitting = $state(false);
  let uploading = $state(false);
  let deleting = $state(false);
  let reparenting = $state(false);
  let errorMessage = $state('');
  let taskFileInput = $state<HTMLInputElement | null>(null);
  let commentFileInput = $state<HTMLInputElement | null>(null);
  let subtaskImageInput = $state<HTMLInputElement | null>(null);
  const statusName = (id: string) => statuses.find((status) => status.id === id)?.name || id.slice(0, 8);
  const priorityName: Record<string, string> = { urgent: '紧急', high: '高', medium: '中', low: '低', none: '无' };
  // datetime-local 的值是本地时区无时区后缀,与 ISO 互转都经 Date 对象走本机时区。
  const isoToLocalInput = (iso: string) => {
    const date = new Date(iso);
    const pad = (value: number) => String(value).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
  };
  // 父任务信息:面包屑、归属卡、子任务进度都要用。
  const parentTask = $derived.by(() => {
    const parentId = task?.parent_task_id;
    return parentId ? (rootTasks.find((item) => item.id === parentId) ?? null) : null;
  });
  const doneSubtasks = $derived(
    subtasks.filter((item) => statuses.find((status) => status.id === item.status_id)?.category === 'done').length
  );
  // 状态流转权限镜像后端规则:管理员豁免;评审人任意;负责人仅限非完成列;其他成员只读。
  const statusControl = $derived.by(() => {
    const me = meStore.current;
    const myRole = members.find((member) => member.user_id === me?.id)?.role;
    const exempt = meStore.isAdmin || myRole === 'manager';
    const isReviewer = task?.reviewer_id != null && task.reviewer_id === me?.id;
    const isAssignee = task?.assignee_id != null && task.assignee_id === me?.id;
    const canChange = Boolean(task) && (exempt || isReviewer || isAssignee);
    const canSetDone = exempt || isReviewer;
    const allowed = canSetDone ? statuses : statuses.filter((status) => status.category !== 'done');
    // 当前状态不在可选集(负责人视角下的已完成)时保留回显项,避免 select 显示空白。
    const current = statuses.find((status) => status.id === task?.status_id) ?? null;
    const options =
      current && !allowed.some((status) => status.id === current.id) ? [...allowed, current] : allowed;
    return { canChange, canSetDone, options: canChange || !current ? options : [current] };
  });

  async function load() {
    loading = true;
    errorMessage = '';
    try {
      const taskResponse = await getTask(taskKey);
      task = taskResponse.data;
      const [subtaskResponse, statusResponse, commentResponse, attachmentResponse, memberResponse, projectTaskResponse, milestoneResponse, labelResponse, dependencyResponse] = await Promise.all([
        getSubtasks(taskKey),
        listStatuses(projectKey),
        listComments(taskKey),
        listTaskAttachments(taskKey),
        listProjectMembers(projectKey),
        listTasks(projectKey, 1, 100),
        listMilestones(projectKey),
        listLabels(projectKey),
        listDependencies(taskKey)
      ]);
      subtasks = subtaskResponse.data;
      statuses = statusResponse.data;
      comments = commentResponse.data;
      attachments = attachmentResponse.data;
      members = memberResponse.data.items;
      projectTasks = projectTaskResponse.data.items;
      rootTasks = projectTaskResponse.data.items.filter((item) => !item.parent_task_id);
      milestones = milestoneResponse.data.items;
      labels = labelResponse.data;
      dependencies = dependencyResponse.data;
      selectedStatus = task.status_id;
      selectedStartAt = taskResponse.data.start_at ? isoToLocalInput(taskResponse.data.start_at) : '';
      selectedDueAt = taskResponse.data.due_at ? isoToLocalInput(taskResponse.data.due_at) : '';
      selectedAssigneeId = taskResponse.data.assignee_id;
      selectedReviewerId = taskResponse.data.reviewer_id;
      selectedTaskType = taskResponse.data.task_type;
      selectedMilestoneId = taskResponse.data.milestone_id;
      attachParentId = '';
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务加载失败';
    } finally {
      loading = false;
    }
  }

  async function changeStatus() {
    if (!task || !selectedStatus || selectedStatus === task.status_id) return;
    submitting = true;
    try {
      task = (await transitionTask(task.task_key, selectedStatus)).data;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '状态修改失败';
      if (task) selectedStatus = task.status_id;
    } finally {
      submitting = false;
    }
  }

  function resetSubtaskForm() {
    for (const image of subtaskImages) URL.revokeObjectURL(image.url);
    subtaskTitle = '';
    subtaskDescription = '';
    subtaskAssigneeId = null;
    subtaskReviewerId = null;
    subtaskType = 'feature';
    subtaskStartAt = '';
    subtaskDueAt = '';
    subtaskComment = '';
    subtaskImages = [];
    subtaskError = '';
    if (subtaskImageInput) subtaskImageInput.value = '';
  }

  function openSubtaskModal() {
    resetSubtaskForm();
    showSubtaskModal = true;
  }

  function closeSubtaskModal() {
    if (submitting) return;
    showSubtaskModal = false;
    resetSubtaskForm();
  }

  function addSubtaskImages(event: Event) {
    const files = Array.from((event.currentTarget as HTMLInputElement).files ?? []);
    const images = files.filter((file) => file.type.startsWith('image/'));
    subtaskImages = [...subtaskImages, ...images.map((file) => ({ file, url: URL.createObjectURL(file) }))];
    if (subtaskImageInput) subtaskImageInput.value = '';
  }

  function removeSubtaskImage(image: { file: File; url: string }) {
    URL.revokeObjectURL(image.url);
    subtaskImages = subtaskImages.filter((item) => item !== image);
  }

  async function addSubtask(event: SubmitEvent) {
    event.preventDefault();
    if (!subtaskTitle.trim()) {
      subtaskError = '子任务名称不能为空';
      return;
    }
    submitting = true;
    subtaskError = '';
    errorMessage = '';
    let created: TaskView | null = null;
    try {
      // 继承当前状态,但完成列只有可定稿的人才能作为子任务初始状态,其余回落后端默认状态。
      const inherit = statuses.find((status) => status.id === selectedStatus);
      const statusId = inherit && (inherit.category !== 'done' || statusControl.canSetDone) ? selectedStatus : undefined;
      created = (await createSubtask(taskKey, {
        title: subtaskTitle.trim(),
        description: subtaskDescription.trim() || undefined,
        task_type: subtaskType,
        status_id: statusId || undefined,
        assignee_id: subtaskAssigneeId,
        reviewer_id: subtaskReviewerId,
        start_at: subtaskStartAt ? new Date(subtaskStartAt).toISOString() : undefined,
        due_at: subtaskDueAt ? new Date(subtaskDueAt).toISOString() : undefined
      })).data;

      const createdTaskKey = created.task_key;
      const uploaded = await Promise.all(
        subtaskImages.map(({ file }) => uploadTaskAttachment(createdTaskKey, file).then((response) => response.data))
      );
      if (subtaskComment.trim()) {
        await createComment(created.task_key, subtaskComment.trim(), uploaded.map((attachment) => attachment.id));
      }

      subtasks = (await getSubtasks(taskKey)).data;
      showSubtaskModal = false;
      resetSubtaskForm();
    } catch (error) {
      const message = error instanceof ApiClientError ? error.message : '子任务创建失败';
      subtaskError = created
        ? `子任务 ${created.task_key} 已创建，但评论或图片保存失败：${message}。请在子任务详情页补充。`
        : message;
    } finally {
      submitting = false;
    }
  }

  async function addComment(event: SubmitEvent) {
    event.preventDefault();
    if (!commentBody.trim()) return;
    submitting = true;
    try {
      const created = (await createComment(taskKey, commentBody.trim(), pending.map((item) => item.id))).data;
      comments = [...comments, created];
      commentBody = '';
      pending = [];
      attachments = (await listTaskAttachments(taskKey)).data;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '评论创建失败';
    } finally {
      submitting = false;
    }
  }

  async function removeComment(comment: Comment) {
    if (!(await confirmDialog({ title: '删除评论', message: '确定逻辑删除这条评论吗？', confirmLabel: '删除', danger: true }))) return;
    try {
      await deleteComment(comment.id);
      comments = comments.filter((item) => item.id !== comment.id);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '评论删除失败';
    }
  }

  async function removeTask() {
    if (!task || !(await confirmDialog({ title: '逻辑删除任务', message: `确认逻辑删除任务 ${task.task_key}？`, confirmLabel: '删除', danger: true }))) return;
    deleting = true;
    try {
      await deleteTask(task.task_key, '用户从任务详情页发起删除');
      await goto(`/projects/${projectKey}/board`);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务删除失败';
    } finally {
      deleting = false;
    }
  }

  async function saveTaskDetails() {
    if (!task) return;
    savingDetails = true;
    errorMessage = '';
    try {
      const updated = (await updateTask(task.task_key, {
        assignee_id: selectedAssigneeId,
        reviewer_id: selectedReviewerId,
        task_type: selectedTaskType,
        start_at: selectedStartAt ? new Date(selectedStartAt).toISOString() : null,
        due_at: selectedDueAt ? new Date(selectedDueAt).toISOString() : null,
        milestone_id: selectedMilestoneId
      })).data;
      task = updated;
      selectedAssigneeId = updated.assignee_id;
      selectedReviewerId = updated.reviewer_id;
      selectedMilestoneId = updated.milestone_id;
      selectedStartAt = updated.start_at ? isoToLocalInput(updated.start_at) : '';
      selectedDueAt = updated.due_at ? isoToLocalInput(updated.due_at) : '';
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务信息保存失败';
    } finally {
      savingDetails = false;
    }
  }

  // 标签:输入回车新建,或点项目已有标签快速附上;× 移除关联。
  async function submitLabel(event: SubmitEvent) {
    event.preventDefault();
    if (!task || !labelInput.trim()) return;
    labelBusy = true;
    errorMessage = '';
    try {
      const label = (await addTaskLabel(task.task_key, labelInput.trim())).data;
      if (!task.labels.some((item) => item.id === label.id)) task.labels = [...task.labels, label];
      if (!labels.some((item) => item.id === label.id)) labels = [...labels, label];
      labelInput = '';
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '标签添加失败';
    } finally {
      labelBusy = false;
    }
  }

  async function detachLabel(labelId: string) {
    if (!task) return;
    labelBusy = true;
    try {
      await removeTaskLabel(task.task_key, labelId);
      task.labels = task.labels.filter((item) => item.id !== labelId);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '标签移除失败';
    } finally {
      labelBusy = false;
    }
  }

  // 依赖:选择同项目任务添加「阻塞我」关系;返回全量列表直接回填。
  async function submitDependency(event: SubmitEvent) {
    event.preventDefault();
    if (!task || !dependencyTarget) return;
    dependencyBusy = true;
    errorMessage = '';
    try {
      dependencies = (await addDependency(task.task_key, dependencyTarget)).data;
      dependencyTarget = '';
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '依赖添加失败';
    } finally {
      dependencyBusy = false;
    }
  }

  async function detachDependency(dependencyId: string) {
    if (!task) return;
    dependencyBusy = true;
    errorMessage = '';
    try {
      dependencies = (await removeDependency(task.task_key, dependencyId)).data;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '依赖移除失败';
    } finally {
      dependencyBusy = false;
    }
  }

  // 依赖候选:同项目、非自身、尚未建立「阻塞我」关系的任务。
  const dependencyOptions = $derived.by(() => {
    const linked = new Set((dependencies?.blocked_by ?? []).map((item) => item.task_id));
    return projectTasks.filter((item) => item.id !== task?.id && !linked.has(item.id));
  });
  const blockedByIncomplete = $derived(
    (dependencies?.blocked_by ?? []).filter((item) => !item.is_done)
  );

  // 归属变更:根任务挂靠为子任务,或子任务脱离父任务转回主任务。
  async function changeParent(parentId: string | null) {
    if (!task) return;
    reparenting = true;
    errorMessage = '';
    try {
      const updated = (await updateTask(task.task_key, { parent_task_id: parentId })).data;
      task = updated;
      attachParentId = '';
      // 归属变了,子任务列表与根任务选择集同步调整。
      subtasks = (await getSubtasks(taskKey)).data;
      rootTasks = parentId
        ? rootTasks.filter((item) => item.id !== updated.id)
        : [...rootTasks, updated];
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务归属修改失败';
    } finally {
      reparenting = false;
    }
  }

  // 选图后立刻上传:详情页图片直接落库,评论图片先进暂存区,提交时一并关联。
  async function uploadImages(event: Event, into: 'task' | 'pending') {
    const input = event.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = '';
    if (!files.length) return;
    uploading = true;
    errorMessage = '';
    try {
      for (const file of files) {
        const created = (await uploadTaskAttachment(taskKey, file)).data;
        if (into === 'task') attachments = [...attachments, created];
        else pending = [...pending, created];
      }
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '图片上传失败';
    } finally {
      uploading = false;
    }
  }

  async function removePending(item: Attachment) {
    pending = pending.filter((attachment) => attachment.id !== item.id);
    try {
      await deleteAttachment(item.id, '评论未提交，撤回暂存图片');
    } catch {
      errorMessage = '暂存图片已从评论移除，但服务端删除失败';
    }
  }

  async function removeAttachment(item: Attachment) {
    if (!(await confirmDialog({ title: '删除图片', message: `确定删除 ${item.file_name} 吗？`, confirmLabel: '删除', danger: true }))) return;
    try {
      await deleteAttachment(item.id, '用户从任务详情页删除图片');
      attachments = attachments.filter((attachment) => attachment.id !== item.id);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '图片删除失败';
    }
  }

  bindReload(() => void load());
</script>

{#if loading}
  <div class="workspace-card state-box">正在加载任务详情…</div>
{:else if errorMessage && !task}
  <div class="workspace-card state-box error-state">
    <strong>{errorMessage}</strong>
    <a class="primary-button" href="/projects">返回项目列表</a>
  </div>
{:else if task}
  <PageHeader
    title={task.title}
    crumbs={[
      { label: '任务', href: '/tasks' },
      { label: projectKey, href: `/projects/${projectKey}/board` },
      ...(parentTask ? [{ label: `父任务 ${parentTask.task_key}`, href: `/tasks/${parentTask.task_key}` }] : []),
      { label: task.task_key }
    ]}
    description={task.description || '暂无描述。'}
  />
  <div class="detail-grid">
    <section class="workspace-card main-card">
      <div class="field-grid">
        <div>
          <span class="field-label">任务状态</span>
          <select bind:value={selectedStatus} onchange={changeStatus} disabled={submitting || !statusControl.canChange}>
            {#each statusControl.options as status}<option value={status.id}>{status.name}</option>{/each}
            {#if !statusControl.options.length}<option value={task.status_id}>{statusName(task.status_id)}</option>{/if}
          </select>
          {#if !statusControl.canChange}<span class="flow-hint">仅负责人或评审人可变更状态</span>{/if}
        </div>
        <div>
          <span class="field-label">负责人</span>
          <MemberPicker value={selectedAssigneeId} {members} disabled={submitting || savingDetails} onchange={(value) => (selectedAssigneeId = value)} ariaLabel={`设置 ${task.title} 的负责人`} />
        </div>
        <div>
          <span class="field-label">评审人</span>
          <MemberPicker value={selectedReviewerId} {members} disabled={submitting || savingDetails} onchange={(value) => (selectedReviewerId = value)} ariaLabel={`设置 ${task.title} 的评审人`} />
        </div>
        <div><span class="field-label">类型</span><select bind:value={selectedTaskType} disabled={submitting || savingDetails} aria-label="任务类型">
          {#each taskTypeOptions as option}<option value={option.value}>{option.label}</option>{/each}
        </select></div>
        <div><span class="field-label">优先级</span><strong class="priority">{priorityName[task.priority]}</strong></div>
        <div>
          <span class="field-label">里程碑</span>
          <select bind:value={selectedMilestoneId} disabled={submitting || savingDetails} aria-label="关联里程碑">
            <option value={null}>未关联</option>
            {#each milestones as milestone (milestone.id)}
              <option value={milestone.id}>{milestone.name}{milestone.due_date ? ` · ${milestone.due_date}` : ''}</option>
            {/each}
          </select>
        </div>
        <div>
          <span class="field-label">开始时间</span>
          <input
            class="due-input"
            type="datetime-local"
            bind:value={selectedStartAt}
            disabled={submitting || savingDetails}
            aria-label="开始时间"
          />
        </div>
        <div>
          <span class="field-label">结束时间</span>
          <input
            class="due-input"
            type="datetime-local"
            bind:value={selectedDueAt}
            disabled={submitting || savingDetails}
            aria-label="结束时间"
          />
        </div>
        <div><span class="field-label">任务编号</span><strong class="mono">#{task.task_number}</strong></div>
        <div><span class="field-label">更新时间</span><strong>{new Date(task.updated_at).toLocaleString('zh-CN')}</strong></div>
      </div>
      <div class="details-save-bar">
        <span>负责人、审批人和排期修改后，点击保存才会生效。</span>
        <button class="primary-button" type="button" onclick={saveTaskDetails} disabled={savingDetails || submitting}>
          {savingDetails ? '保存中…' : '保存任务信息'}
        </button>
      </div>
      <div class="description-block">
        <span class="field-label">任务描述</span>
        <p>{task.description || '暂无描述内容。'}</p>
      </div>
      <div class="labels-block">
        <span class="field-label">标签</span>
        {#if task.labels.length}
          <div class="label-list">
            {#each task.labels as label (label.id)}
              <LabelPill name={label.name} onremove={labelBusy ? undefined : () => detachLabel(label.id)} />
            {/each}
          </div>
        {:else}
          <p class="empty-inline">还没有标签。</p>
        {/if}
        <form class="label-form" onsubmit={submitLabel}>
          <input bind:value={labelInput} placeholder="输入标签名,回车添加" aria-label="新标签名称" disabled={labelBusy} />
          <button class="secondary-button" type="submit" disabled={labelBusy || !labelInput.trim()}>添加</button>
        </form>
        {#if labels.length}
          <div class="label-suggestions">
            {#each labels as label (label.id)}
              {#if !task.labels.some((item) => item.id === label.id)}
                <button class="text-button label-suggest" type="button" disabled={labelBusy} onclick={() => { labelInput = label.name; }}>+ {label.name}</button>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
      <div class="attachment-block">
        <div class="subtask-heading">
          <div><h2>图片</h2><p>支持 PNG、JPEG、GIF、WebP，单张不超过 10MB。</p></div>
          <span>{attachments.length} 张</span>
        </div>
        <div class="attachment-grid">
          {#each attachments as item}
            <figure>
              <a href={attachmentUrl(item.url)} target="_blank" rel="noreferrer">
                <img src={attachmentUrl(item.url)} alt={item.file_name} loading="lazy" />
              </a>
              <figcaption>
                <span title={item.file_name}>{item.file_name}</span>
                <button class="text-button" type="button" onclick={() => removeAttachment(item)}>删除</button>
              </figcaption>
            </figure>
          {:else}
            <div class="empty-inline">还没有上传图片。</div>
          {/each}
        </div>
        <input type="file" accept="image/png,image/jpeg,image/gif,image/webp" multiple hidden bind:this={taskFileInput} onchange={(event) => uploadImages(event, 'task')} />
        <button class="secondary-button attach-button" type="button" disabled={uploading} onclick={() => taskFileInput?.click()}>
          {uploading ? '上传中…' : '上传图片'}
        </button>
      </div>
      <div class="subtask-heading">
        <div><h2>子任务</h2><p>子任务不能再创建子任务。</p></div>
        <span>{subtasks.length} 项 · 已完成 {doneSubtasks}</span>
      </div>
      <div class="subtask-list">
        {#each subtasks as subtask}
          <a href={`/tasks/${subtask.task_key}`}>
            <span class="task-key">{subtask.task_key}</span>
            <strong>{subtask.title}</strong>
            <span class="status-pill">{statusName(subtask.status_id)}</span>
            <TaskTypePill taskType={subtask.task_type} />
          </a>
        {:else}
          <div class="empty-inline">还没有子任务。</div>
        {/each}
      </div>
      <button class="secondary-button add-subtask-button" type="button" disabled={submitting} onclick={openSubtaskModal}>
        添加子任务
      </button>
      <div class="comments">
        <div class="subtask-heading">
          <div><h2>评论</h2><p>评论新增和逻辑删除都会写入操作日志。</p></div>
          <span>{comments.length} 条</span>
        </div>
        <div class="comment-list">
          {#each comments as comment}
            <article>
              <div class="comment-meta">
                <strong>{comment.author_name}</strong>
                <time>{new Date(comment.created_at).toLocaleString('zh-CN')}</time>
                <button class="text-button" onclick={() => removeComment(comment)}>删除</button>
              </div>
              <p>{comment.body}</p>
              {#if comment.attachments?.length}
                <div class="comment-images">
                  {#each comment.attachments as item}
                    <a href={attachmentUrl(item.url)} target="_blank" rel="noreferrer" title={item.file_name}>
                      <img src={attachmentUrl(item.url)} alt={item.file_name} loading="lazy" />
                    </a>
                  {/each}
                </div>
              {/if}
            </article>
          {:else}
            <div class="empty-inline">还没有评论。</div>
          {/each}
        </div>
        <form class="comment-form" onsubmit={addComment}>
          {#if pending.length}
            <div class="pending-images">
              {#each pending as item}
                <span class="pending-image">
                  <img src={attachmentUrl(item.url)} alt={item.file_name} />
                  <button class="text-button" type="button" aria-label={`移除 ${item.file_name}`} onclick={() => removePending(item)}>×</button>
                </span>
              {/each}
            </div>
          {/if}
          <textarea bind:value={commentBody} rows="3" placeholder="写下你的评论" aria-label="评论内容"></textarea>
          <div class="comment-actions">
            <input type="file" accept="image/png,image/jpeg,image/gif,image/webp" multiple hidden bind:this={commentFileInput} onchange={(event) => uploadImages(event, 'pending')} />
            <button class="secondary-button" type="button" disabled={uploading} onclick={() => commentFileInput?.click()}>
              {uploading ? '上传中…' : '添加图片'}
            </button>
            <button class="primary-button" type="submit" disabled={submitting}>发表评论</button>
          </div>
        </form>
      </div>
    </section>
    <aside class="workspace-card side-card">
      <h2>任务归属</h2>
      {#if parentTask}
        <p class="parent-line">
          当前是 <a href={`/tasks/${parentTask.task_key}`}>{parentTask.task_key}</a> 的子任务,
          脱离后转回主任务,看板与列表不再显示父任务标识。
        </p>
        <button class="secondary-button" type="button" onclick={() => changeParent(null)} disabled={reparenting}>
          {reparenting ? '处理中…' : '脱离父任务'}
        </button>
      {:else}
        <p>挂靠后成为所选任务的子任务,在看板卡片与列表行显示「↳ 父任务」标识。</p>
        <select bind:value={attachParentId} disabled={reparenting} aria-label="选择父任务">
          <option value="">选择要挂靠的任务</option>
          {#each rootTasks as root (root.id)}
            {#if root.id !== task.id}
              <option value={root.id}>{root.task_key} · {root.title}</option>
            {/if}
          {/each}
        </select>
        <button
          class="secondary-button"
          type="button"
          disabled={reparenting || !attachParentId}
          onclick={() => attachParentId && changeParent(attachParentId)}
        >
          {reparenting ? '处理中…' : '设为子任务'}
        </button>
      {/if}
      <h2>依赖</h2>
      {#if dependencies}
        {#if blockedByIncomplete.length}
          <p class="dependency-hint">还有 {blockedByIncomplete.length} 个未完成的任务阻塞当前任务。</p>
        {/if}
        <div class="dependency-groups">
          <div>
            <span class="field-label">阻塞我</span>
            {#each dependencies.blocked_by as item (item.dependency_id)}
              <div class="dependency-row">
                <a href={`/tasks/${item.task_key}`} class:done={item.is_done}>
                  <span class="task-key">{item.task_key}</span>
                  <span class="dep-title">{item.title}</span>
                  <span class="status-pill">{item.status_name}</span>
                </a>
                <button class="text-button" type="button" disabled={dependencyBusy} onclick={() => detachDependency(item.dependency_id)}>移除</button>
              </div>
            {:else}
              <p class="empty-inline">无。</p>
            {/each}
          </div>
          <div>
            <span class="field-label">我阻塞</span>
            {#each dependencies.blocks as item (item.dependency_id)}
              <div class="dependency-row">
                <a href={`/tasks/${item.task_key}`} class:done={item.is_done}>
                  <span class="task-key">{item.task_key}</span>
                  <span class="dep-title">{item.title}</span>
                  <span class="status-pill">{item.status_name}</span>
                </a>
                <button class="text-button" type="button" disabled={dependencyBusy} onclick={() => detachDependency(item.dependency_id)}>移除</button>
              </div>
            {:else}
              <p class="empty-inline">无。</p>
            {/each}
          </div>
        </div>
        <form class="dependency-form" onsubmit={submitDependency}>
          <select bind:value={dependencyTarget} disabled={dependencyBusy} aria-label="选择依赖任务">
            <option value="">选择要依赖的任务</option>
            {#each dependencyOptions as option (option.id)}
              <option value={option.task_key}>{option.task_key} · {option.title}</option>
            {/each}
          </select>
          <button class="secondary-button" type="submit" disabled={dependencyBusy || !dependencyTarget}>
            {dependencyBusy ? '处理中…' : '添加依赖'}
          </button>
        </form>
        <p class="dependency-note">依赖仅支持同项目任务,系统会阻止自依赖与循环依赖。</p>
      {/if}
      <h2>操作</h2>
      <p>删除采用逻辑删除，动作会写入项目操作日志。</p>
      <button class="danger-button" type="button" onclick={removeTask} disabled={deleting}>{deleting ? '删除中…' : '逻辑删除任务'}</button>
      {#if errorMessage}<p class="error-message">{errorMessage}</p>{/if}
    </aside>
  </div>
{/if}


<Modal open={showSubtaskModal} title="添加子任务" onClose={closeSubtaskModal}>
  <form id="create-subtask-form" class="subtask-modal-form" onsubmit={addSubtask}>
    <label>
      <span>名称 <em>*</em></span>
      <input bind:value={subtaskTitle} maxlength="200" placeholder="输入子任务名称" aria-label="子任务名称" />
    </label>
    <label>
      <span>描述</span>
      <textarea bind:value={subtaskDescription} rows="4" placeholder="补充子任务背景、范围或验收标准" aria-label="子任务描述"></textarea>
    </label>
    <div class="subtask-form-row">
      <label>
        <span>类型</span>
        <select bind:value={subtaskType} aria-label="子任务类型">
          {#each taskTypeOptions as option}<option value={option.value}>{option.label}</option>{/each}
        </select>
      </label>
      <label>
        <span>负责人</span>
        <MemberPicker value={subtaskAssigneeId} {members} onchange={(value) => (subtaskAssigneeId = value)} ariaLabel="子任务负责人" />
      </label>
      <label>
        <span>审批人</span>
        <MemberPicker value={subtaskReviewerId} {members} onchange={(value) => (subtaskReviewerId = value)} ariaLabel="子任务审批人" />
      </label>
    </div>
    <div class="subtask-form-row">
      <label>
        <span>开始时间</span>
        <input type="datetime-local" bind:value={subtaskStartAt} aria-label="子任务开始时间" />
      </label>
      <label>
        <span>结束时间</span>
        <input type="datetime-local" bind:value={subtaskDueAt} aria-label="子任务结束时间" />
      </label>
    </div>
    <label>
      <span>评论</span>
      <textarea bind:value={subtaskComment} rows="3" placeholder="添加创建说明或评论（可选）" aria-label="子任务评论"></textarea>
    </label>
    <div class="subtask-images-field">
      <span>图片</span>
      {#if subtaskImages.length}
        <div class="subtask-image-previews">
          {#each subtaskImages as image (image.url)}
            <span class="subtask-image-preview">
              <img src={image.url} alt={image.file.name} />
              <button type="button" aria-label={`移除 ${image.file.name}`} onclick={() => removeSubtaskImage(image)}>×</button>
            </span>
          {/each}
        </div>
      {/if}
      <input type="file" accept="image/png,image/jpeg,image/gif,image/webp" multiple hidden bind:this={subtaskImageInput} onchange={addSubtaskImages} />
      <button class="secondary-button" type="button" onclick={() => subtaskImageInput?.click()} disabled={submitting}>添加图片</button>
      <small>支持 PNG、JPG、GIF、WebP。填写评论时，图片会附在该评论中。</small>
    </div>
    {#if subtaskError}<p class="error-message" role="alert">{subtaskError}</p>{/if}
  </form>
  {#snippet footer()}
    <button class="secondary-button" type="button" onclick={closeSubtaskModal} disabled={submitting}>取消</button>
    <button class="primary-button" type="submit" form="create-subtask-form" disabled={submitting}>
      {submitting ? '创建中…' : '创建子任务'}
    </button>
  {/snippet}
</Modal>

<style>
  h2, p { margin: 0; }
  .detail-grid { display: grid; grid-template-columns: minmax(0, 1fr) 290px; gap: 18px; }
  .main-card { display: grid; gap: 24px; }
  .field-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px; padding-bottom: 20px; border-bottom: 1px solid var(--color-border); }
  .field-grid > div { display: grid; gap: 7px; min-width: 0; }
  .field-label { color: var(--color-text-muted); font-size: 12px; font-weight: 500; }
  .flow-hint { color: var(--color-text-muted); font-size: 11px; }
  .field-grid select { min-width: 0; }
  .priority { color: var(--color-warning); }
  .mono { font-family: var(--font-mono); }
  .description-block { display: grid; gap: 8px; }
  .description-block p { white-space: pre-wrap; color: var(--color-text-secondary); line-height: 1.7; }
  .labels-block { display: grid; gap: 8px; }
  .label-list { display: flex; flex-wrap: wrap; gap: 6px; }
  .label-form { display: flex; gap: 8px; }
  .label-form input { flex: 1; min-width: 0; }
  .label-form button { border: 0; white-space: nowrap; }
  .label-suggestions { display: flex; flex-wrap: wrap; gap: 6px; }
  .label-suggest { color: var(--color-primary-strong); }
  .dependency-groups { display: grid; gap: 12px; }
  .dependency-groups > div { display: grid; gap: 4px; }
  .dependency-hint { color: var(--color-warning); }
  .dependency-row { display: flex; align-items: center; gap: 8px; }
  .dependency-row > a { display: grid; grid-template-columns: 90px minmax(0, 1fr) auto; align-items: center; gap: 8px; flex: 1; min-width: 0; color: var(--color-text); text-decoration: none; font-size: 13px; }
  .dependency-row > a:hover .dep-title { color: var(--color-primary); }
  .dependency-row .dep-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dependency-row a.done { opacity: 0.55; }
  .dependency-form { display: grid; gap: 8px; }
  .dependency-note { color: var(--color-text-muted); font-size: 12px; }
  .subtask-heading { display: flex; justify-content: space-between; gap: 12px; align-items: flex-start; }
  .subtask-heading h2, .side-card h2 { font-size: 18px; }
  .subtask-heading p, .side-card p { margin-top: 5px; color: var(--color-text-muted); font-size: 13px; }
  .subtask-heading > span { color: var(--color-text-muted); font-size: 13px; }
  .subtask-list { display: grid; }
  .subtask-list > a { display: grid; grid-template-columns: 105px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 12px 0; border-top: 1px solid var(--color-border); }
  .subtask-list > a:hover strong { color: var(--color-primary); }
  .task-key { color: var(--color-primary-strong); font-family: var(--font-mono); font-size: 12px; }
  .empty-inline { padding: 12px 0; color: var(--color-text-muted); font-size: 13px; }
  .add-subtask-button { justify-self: start; border: 0; }
  .subtask-modal-form { display: grid; gap: 14px; }
  .subtask-modal-form label, .subtask-images-field { display: grid; gap: 6px; }
  .subtask-modal-form label > span, .subtask-images-field > span { color: var(--color-text-secondary); font-size: 13px; font-weight: 500; }
  .subtask-modal-form em { color: var(--color-danger); font-style: normal; }
  .subtask-modal-form input, .subtask-modal-form textarea { width: 100%; min-width: 0; }
  .subtask-modal-form textarea { resize: vertical; }
  .subtask-form-row { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .subtask-image-previews { display: flex; flex-wrap: wrap; gap: 8px; }
  .subtask-image-preview { position: relative; display: block; width: 88px; height: 66px; overflow: hidden; border: 1px solid var(--color-border); border-radius: var(--radius-md); }
  .subtask-image-preview img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .subtask-image-preview button { position: absolute; top: 2px; right: 2px; display: grid; width: 20px; height: 20px; place-items: center; border: 0; border-radius: var(--radius-sm); background: rgba(0, 0, 0, 0.65); color: #fff; cursor: pointer; }
  .subtask-images-field small { color: var(--color-text-muted); font-size: 12px; line-height: 1.5; }
  .attachment-block { display: grid; gap: 12px; }
  .attachment-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 12px; }
  .attachment-grid figure { display: grid; gap: 6px; min-width: 0; margin: 0; }
  .attachment-grid img, .comment-images img, .pending-images img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .attachment-grid a { display: block; aspect-ratio: 4 / 3; border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; background: var(--color-surface-sunken); }
  .attachment-grid a:hover { border-color: var(--color-border-strong); }
  .attachment-grid figcaption { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: 12px; color: var(--color-text-muted); }
  .attachment-grid figcaption span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .attach-button { justify-self: start; border: 0; }
  .comments { display: grid; gap: 12px; }
  .comment-list { display: grid; gap: 10px; }
  .comment-list article { padding: 12px; background: var(--color-surface-sunken); border: 1px solid var(--color-border-weak); border-radius: var(--radius-md); }
  .comment-meta { display: flex; align-items: center; gap: 10px; }
  .comment-meta time { color: var(--color-text-muted); font-size: 12px; }
  .comment-meta .text-button { margin-left: auto; }
  .comment-list p { margin: 8px 0 0; white-space: pre-wrap; color: var(--color-text-secondary); line-height: 1.6; }
  .comment-images { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
  .comment-images a { display: block; width: 96px; height: 72px; border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; }
  .comment-images a:hover { border-color: var(--color-border-strong); }
  .comment-form { display: grid; gap: 8px; }
  .comment-form textarea { width: 100%; resize: vertical; }
  .comment-actions { display: flex; justify-content: flex-end; gap: 8px; }
  .comment-actions button { border: 0; }
  .pending-images { display: flex; flex-wrap: wrap; gap: 8px; }
  .pending-image { position: relative; display: block; width: 96px; height: 72px; border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; }
  .pending-image button { position: absolute; top: 2px; right: 2px; width: 20px; height: 20px; line-height: 1; display: grid; place-items: center; background: rgba(0, 0, 0, 0.6); color: #fff; font-size: 14px; border-radius: var(--radius-sm); }
  .text-button { border: 0; background: transparent; color: var(--color-danger); font-weight: 500; cursor: pointer; }
  .side-card { display: grid; align-content: start; gap: 12px; height: max-content; }
  .parent-line a { color: var(--color-primary-strong); font-family: var(--font-mono); font-size: 13px; }
  .side-card select { width: 100%; }
  .side-card .secondary-button, .side-card .danger-button { border: 0; }
  .due-input { min-width: 0; }
  .details-save-bar { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 14px; border: 1px solid var(--color-border-weak); border-radius: var(--radius-md); background: var(--color-surface-sunken); color: var(--color-text-muted); font-size: 12px; }
  .details-save-bar button { border: 0; white-space: nowrap; }
  .error-message { color: var(--color-danger); font-size: 13px; }
  .state-box { display: grid; place-items: center; gap: 12px; min-height: 220px; }
  .error-state { color: var(--color-danger); }
  @media (max-width: 900px) {
    .detail-grid { grid-template-columns: 1fr; }
  }
  @media (max-width: 560px) {
    .subtask-list > a { grid-template-columns: 1fr; gap: 5px; }
    .subtask-form-row { grid-template-columns: 1fr; }
    .details-save-bar { align-items: stretch; flex-direction: column; }
    .comment-actions { justify-content: stretch; }
    .comment-actions button { flex: 1; }
  }
</style>
