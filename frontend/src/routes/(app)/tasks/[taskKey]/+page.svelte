<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import Modal from '$lib/components/Modal.svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import LabelPill from '$lib/components/LabelPill.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { confirmDialog } from '$lib/features/ui/dialog.svelte';
  import { listStatuses, listProjectMembers } from '$lib/api/projects';
  import { listMilestones } from '$lib/api/milestones';
  import { listTaskLogs } from '$lib/api/audit';
  import { deleteAttachment, listTaskAttachments, uploadTaskAttachment, attachmentUrl } from '$lib/api/attachments';
  import { addDependency, addTaskLabel, copyTask, createComment, createSubtask, deleteComment, deleteTask, getSubtasks, getTask, listComments, listDependencies, listLabels, listTasks, removeDependency, removeTaskLabel, transitionTask, updateTask } from '$lib/api/tasks';
  import type { Attachment, Comment, LabelView, Milestone, OperationLog, Priority, ProjectMember, ProjectStatus, TaskDependencies, TaskView } from '$lib/api/types';
  import MemberPicker from '$lib/features/task-list/MemberPicker.svelte';
  import { meStore } from '$lib/features/auth/me.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';
  import { taskTypeOptions } from '$lib/features/task-types';

  type TaskUpdateInput = Parameters<typeof updateTask>[1];
  type ActivityTab = 'all' | 'comments' | 'changes';
  type FeedItem = { kind: 'comment'; time: number; comment: Comment } | { kind: 'change'; time: number; log: OperationLog };

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
  let changeLogs = $state<OperationLog[]>([]);
  let activityTab = $state<ActivityTab>('all');
  // 标题/描述行内编辑草稿,编辑态才有值。
  let titleDraft = $state('');
  let editingTitle = $state(false);
  let descDraft = $state('');
  let editingDesc = $state(false);
  // 侧栏 ghost 入口展开态。
  let addLabelOpen = $state(false);
  let addDepOpen = $state(false);
  let addParentOpen = $state(false);
  let labelInput = $state('');
  let dependencyTarget = $state('');
  let attachParentId = $state('');
  let propBusy = $state(false);
  let copying = $state(false);
  let labelBusy = $state(false);
  let dependencyBusy = $state(false);
  let showSubtaskModal = $state(false);
  let subtaskTitle = $state('');
  let subtaskDescription = $state('');
  let subtaskAssigneeId = $state<string | null>(null);
  let subtaskReviewerId = $state<string | null>(null);
  let subtaskType = $state('feature');
  let subtaskStartAt = $state('');
  let subtaskDueAt = $state('');
  let subtaskComment = $state('');
  let subtaskFiles = $state<{ file: File; previewUrl: string | null }[]>([]);
  let subtaskError = $state('');
  let quickSubtaskTitle = $state('');
  let loading = $state(true);
  let submitting = $state(false);
  let uploading = $state(false);
  let deleting = $state(false);
  let reparenting = $state(false);
  let errorMessage = $state('');
  let taskFileInput = $state<HTMLInputElement | null>(null);
  let commentFileInput = $state<HTMLInputElement | null>(null);
  let subtaskFileInput = $state<HTMLInputElement | null>(null);
  // 行内编辑挂载后自动聚焦:标题全选,描述落在文末。
  let titleInputEl = $state<HTMLInputElement | null>(null);
  let descInputEl = $state<HTMLTextAreaElement | null>(null);
  $effect(() => {
    if (editingTitle && titleInputEl) {
      titleInputEl.focus();
      titleInputEl.select();
    }
  });
  $effect(() => {
    if (editingDesc && descInputEl) {
      descInputEl.focus();
      descInputEl.setSelectionRange(descInputEl.value.length, descInputEl.value.length);
    }
  });
  const statusName = (id: string) => statuses.find((status) => status.id === id)?.name || id.slice(0, 8);
  const isImageAttachment = (attachment: Attachment) => attachment.mime_type.startsWith('image/');
  const isImageFile = (file: File) => file.type.startsWith('image/') || /\.(png|jpe?g|gif|webp)$/i.test(file.name);
  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };
  const fileIcon = (name: string) => {
    const extension = name.split('.').pop()?.toLowerCase();
    if (extension === 'log' || extension === 'txt') return '≡';
    if (['zip', 'gz', 'tar', 'rar', '7z'].includes(extension ?? '')) return '⌘';
    if (['pdf', 'doc', 'docx', 'xls', 'xlsx', 'csv'].includes(extension ?? '')) return '▤';
    return '↳';
  };
  const priorityOptions: { value: Priority; label: string }[] = [
    { value: 'urgent', label: '紧急' },
    { value: 'high', label: '高' },
    { value: 'medium', label: '中' },
    { value: 'low', label: '低' },
    { value: 'none', label: '无' }
  ];
  // datetime-local 的值是本地时区无时区后缀,与 ISO 互转都经 Date 对象走本机时区。
  const isoToLocalInput = (iso: string) => {
    const date = new Date(iso);
    const pad = (value: number) => String(value).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
  };
  const pad = (value: number) => String(value).padStart(2, '0');
  const fmtTime = (iso: string) => {
    const date = new Date(iso);
    return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
  };
  // 父任务信息:面包屑、归属卡、子任务进度都要用。
  const parentTask = $derived.by(() => {
    const parentId = task?.parent_task_id;
    return parentId ? (rootTasks.find((item) => item.id === parentId) ?? null) : null;
  });
  const currentStatus = $derived(statuses.find((status) => status.id === task?.status_id) ?? null);
  const subtaskDone = (item: TaskView) => statuses.find((status) => status.id === item.status_id)?.category === 'done';
  const doneSubtasks = $derived(subtasks.filter(subtaskDone).length);
  const subtaskProgress = $derived(subtasks.length ? Math.round((doneSubtasks / subtasks.length) * 100) : 0);
  const isOverdue = $derived(Boolean(task?.due_at && currentStatus?.category !== 'done' && new Date(task.due_at) < new Date()));
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
  // 活动流:评论与变更记录合并按时间倒序;「全部」隐藏评论日志本体(评论条目已展示,避免重复)。
  const feed = $derived.by(() => {
    const items: FeedItem[] = [
      ...comments.map((comment): FeedItem => ({ kind: 'comment', time: new Date(comment.created_at).getTime(), comment })),
      ...changeLogs
        .filter((log) => activityTab !== 'all' || log.module !== 'comment')
        .map((log): FeedItem => ({ kind: 'change', time: new Date(log.created_at).getTime(), log }))
    ];
    return items.sort((left, right) => right.time - left.time);
  });
  const tabKind = $derived(activityTab === 'comments' ? 'comment' : 'change');
  const visibleFeed = $derived(activityTab === 'all' ? feed : feed.filter((item) => item.kind === tabKind));
  const actorName = (log: OperationLog) => members.find((member) => member.user_id === log.actor_user_id)?.display_name ?? '成员';

  async function load() {
    loading = true;
    errorMessage = '';
    try {
      const taskResponse = await getTask(taskKey);
      task = taskResponse.data;
      const [subtaskResponse, statusResponse, commentResponse, attachmentResponse, memberResponse, projectTaskResponse, milestoneResponse, labelResponse, dependencyResponse, logResponse] = await Promise.all([
        getSubtasks(taskKey),
        listStatuses(projectKey),
        listComments(taskKey),
        listTaskAttachments(taskKey),
        listProjectMembers(projectKey),
        listTasks(projectKey, 1, 100),
        listMilestones(projectKey),
        listLabels(projectKey),
        listDependencies(taskKey),
        listTaskLogs(taskKey, 1, 20)
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
      changeLogs = logResponse.data.items;
      editingTitle = false;
      editingDesc = false;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务加载失败';
    } finally {
      loading = false;
    }
  }

  // 字段改动即时落库并整体刷新任务视图;变更记录随之补拉,让活动流跟上。
  async function saveField(patch: TaskUpdateInput) {
    if (!task) return;
    propBusy = true;
    errorMessage = '';
    try {
      task = (await updateTask(task.task_key, patch)).data;
      void refreshLogs();
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务字段保存失败';
    } finally {
      propBusy = false;
    }
  }

  async function refreshLogs() {
    try {
      changeLogs = (await listTaskLogs(taskKey, 1, 20)).data.items;
    } catch {
      // 活动流刷新失败不打断主流程,下次进入页面自然补齐。
    }
  }

  async function changeStatus(event: Event) {
    if (!task) return;
    const next = (event.currentTarget as HTMLSelectElement).value;
    if (!next || next === task.status_id) return;
    submitting = true;
    errorMessage = '';
    try {
      task = (await transitionTask(task.task_key, next)).data;
      void refreshLogs();
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '状态修改失败';
    } finally {
      submitting = false;
    }
  }

  async function commitDate(field: 'start_at' | 'due_at', event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    const patch = { [field]: value ? new Date(value).toISOString() : null } as TaskUpdateInput;
    await saveField(patch);
  }

  // 标题:点击切换输入框,回车/失焦提交,Esc 还原。
  function startEditTitle() {
    if (!task) return;
    titleDraft = task.title;
    editingTitle = true;
  }

  async function commitTitle() {
    if (!editingTitle || !task) return;
    editingTitle = false;
    const next = titleDraft.trim();
    if (!next || next === task.title) return;
    await saveField({ title: next });
  }

  function titleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter') {
      event.preventDefault();
      void commitTitle();
    } else if (event.key === 'Escape') {
      editingTitle = false;
    }
  }

  function startEditDesc() {
    if (!task) return;
    descDraft = task.description ?? '';
    editingDesc = true;
  }

  async function commitDesc() {
    if (!editingDesc || !task) return;
    editingDesc = false;
    const next = descDraft.trim();
    if (next === (task.description ?? '')) return;
    await saveField({ description: next || null });
  }

  function resetSubtaskForm() {
    for (const item of subtaskFiles) if (item.previewUrl) URL.revokeObjectURL(item.previewUrl);
    subtaskTitle = '';
    subtaskDescription = '';
    subtaskAssigneeId = null;
    subtaskReviewerId = null;
    subtaskType = 'feature';
    subtaskStartAt = '';
    subtaskDueAt = '';
    subtaskComment = '';
    subtaskFiles = [];
    subtaskError = '';
    if (subtaskFileInput) subtaskFileInput.value = '';
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

  function addSubtaskFiles(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = '';
    if (!files.length) return;
    subtaskFiles = [
      ...subtaskFiles,
      ...files.map((file) => ({ file, previewUrl: isImageFile(file) ? URL.createObjectURL(file) : null }))
    ];
  }

  function removeSubtaskFile(item: { file: File; previewUrl: string | null }) {
    if (item.previewUrl) URL.revokeObjectURL(item.previewUrl);
    subtaskFiles = subtaskFiles.filter((candidate) => candidate !== item);
  }

  // 快捷新建:只填标题就走默认状态/默认列,后续再到子任务详情里补齐。
  async function quickAddSubtask(event: SubmitEvent) {
    event.preventDefault();
    const title = quickSubtaskTitle.trim();
    if (!title || !task) return;
    submitting = true;
    errorMessage = '';
    try {
      await createSubtask(taskKey, { title });
      subtasks = (await getSubtasks(taskKey)).data;
      quickSubtaskTitle = '';
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '子任务创建失败';
    } finally {
      submitting = false;
    }
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
      const inherit = statuses.find((status) => status.id === task?.status_id);
      const statusId = inherit && (inherit.category !== 'done' || statusControl.canSetDone) ? inherit.id : undefined;
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
        subtaskFiles.map(({ file }) => uploadTaskAttachment(createdTaskKey, file).then((response) => response.data))
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
        ? `子任务 ${created.task_key} 已创建，但评论或附件保存失败：${message}。请在子任务详情页补充。`
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
    if (!task || !(await confirmDialog({ title: '逻辑删除任务', message: `确认逻辑删除任务 ${task.task_key}？删除后可在项目回收站恢复。`, confirmLabel: '删除', danger: true }))) return;
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

  // 复制任务:后端复制字段与标签,评论/附件/依赖不随行,成功后直达新任务。
  async function duplicateTask() {
    if (!task || copying) return;
    copying = true;
    errorMessage = '';
    try {
      const created = (await copyTask(task.task_key)).data;
      await goto(`/tasks/${created.task_key}`);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '任务复制失败';
      copying = false;
    }
  }

  // 评论正文安全渲染:先整体转义再高亮 @提及,mention 语法不引入注入面。
  const mentionPattern = /@([^\s@,，。.、;；:：!？?()（）[\]【】"'\n]{1,80})/g;
  function renderComment(body: string): string {
    const escaped = body
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
    return escaped.replace(mentionPattern, '<span class="mention">@$1</span>');
  }

  // 变更记录 diff 字段的中文标签;未收录字段按原 key 显示。
  const fieldLabels: Record<string, string> = {
    title: '标题', description: '描述', priority: '优先级', task_type: '类型',
    assignee_id: '负责人', reviewer_id: '评审人', start_at: '开始时间', due_at: '截止时间',
    parent_task_id: '父任务', milestone_id: '里程碑', status_id: '状态', task_key: '任务编号',
    label: '标签', depends_on_task_key: '依赖任务', reason: '原因', from_status_id: '原状态',
    to_status_id: '目标状态', position: '位置', is_active: '启用状态', password: '密码',
    file_name: '文件名', mime_type: '类型', byte_size: '大小'
  };
  function diffEntries(log: OperationLog): [string, unknown][] {
    if (!log.diff || typeof log.diff !== 'object' || Array.isArray(log.diff)) return [];
    return Object.entries(log.diff as Record<string, unknown>);
  }
  function renderValue(value: unknown): string {
    if (value === null) return '未设置';
    if (typeof value === 'string') {
      const date = new Date(value);
      return /^\d{4}-\d{2}-\d{2}T/.test(value) && !Number.isNaN(date.getTime())
        ? date.toLocaleString('zh-CN')
        : value;
    }
    return JSON.stringify(value);
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
      addLabelOpen = false;
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

  // 依赖:ghost 展开选择器,选中即建「阻塞我」关系;返回全量列表直接回填。
  async function pickDependency(event: Event) {
    if (!task) return;
    const next = (event.currentTarget as HTMLSelectElement).value;
    if (!next) return;
    dependencyBusy = true;
    errorMessage = '';
    try {
      dependencies = (await addDependency(task.task_key, next)).data;
      dependencyTarget = '';
      addDepOpen = false;
      void refreshLogs();
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
      void refreshLogs();
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
      addParentOpen = false;
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

  // 选中文件后立刻上传:详情页文件直接落库,评论附件先进暂存区,提交时一并关联。
  async function uploadFiles(event: Event, into: 'task' | 'pending') {
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
      errorMessage = error instanceof ApiClientError ? error.message : '附件上传失败';
    } finally {
      uploading = false;
    }
  }

  async function removePending(item: Attachment) {
    pending = pending.filter((attachment) => attachment.id !== item.id);
    try {
      await deleteAttachment(item.id, '评论未提交，撤回暂存附件');
    } catch {
      errorMessage = '暂存附件已从评论移除，但服务端删除失败';
    }
  }

  async function removeAttachment(item: Attachment) {
    if (!(await confirmDialog({ title: '删除附件', message: `确定删除 ${item.file_name} 吗？`, confirmLabel: '删除', danger: true }))) return;
    try {
      await deleteAttachment(item.id, '用户从任务详情页删除附件');
      attachments = attachments.filter((attachment) => attachment.id !== item.id);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '附件删除失败';
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
  <div class="task-page">
    <nav class="breadcrumb" aria-label="任务路径">
      <a href="/tasks">任务</a>
      <span>/</span>
      <a href={`/projects/${projectKey}/board`}>{projectKey}</a>
      {#if parentTask}
        <span>/</span>
        <a href={`/tasks/${parentTask.task_key}`}>{parentTask.task_key}</a>
      {/if}
      <span>/</span>
      <span>{task.task_key}</span>
    </nav>

    <header class="task-header">
      <div class="title-row">
        {#if editingTitle}
          <input
            class="title-input"
            bind:value={titleDraft}
            bind:this={titleInputEl}
            maxlength="200"
            aria-label="任务标题"
            onkeydown={titleKeydown}
            onblur={() => void commitTitle()}
          />
        {:else}
          <button class="task-title" type="button" title="点击编辑标题" onclick={startEditTitle}>
            {task.title}
          </button>
        {/if}
        <div class="header-actions">
          <button class="icon-btn" type="button" onclick={duplicateTask} disabled={copying} title="复制字段与标签,评论/附件/依赖不随行">
            {copying ? '复制中…' : '复制'}
          </button>
          <button class="icon-btn danger" type="button" onclick={removeTask} disabled={deleting} title="逻辑删除,可在项目回收站恢复">
            {deleting ? '删除中…' : '删除'}
          </button>
        </div>
      </div>
      <div class="meta-row">
        <span class="status-pill cat-{currentStatus?.category ?? 'todo'}">
          <i class="dot"></i>{currentStatus?.name ?? statusName(task.status_id)}
        </span>
        <span class="meta-item">
          <Avatar name={task.assignee_name ?? '?'} size={20} />
          <span class="name">{task.assignee_name ?? '未分配'}</span> 负责
        </span>
        <span class="meta-item">
          <Avatar name={task.reviewer_name ?? '?'} size={20} />
          <span class="name">{task.reviewer_name ?? '未设置'}</span> 评审
        </span>
        {#if task.due_at}
          <span class="meta-item" class:overdue={isOverdue}>截止 {new Date(task.due_at).toLocaleDateString('zh-CN', { month: 'numeric', day: 'numeric' })}</span>
        {/if}
        <span class="meta-item mono">#{task.task_number}</span>
        <span class="meta-item">更新于 {fmtTime(task.updated_at)}</span>
      </div>
      {#if errorMessage}<div class="error-banner" role="alert">{errorMessage}</div>{/if}
    </header>

    <div class="layout">
      <main>
        <section class="block">
          <div class="block-head"><h2>描述</h2></div>
          {#if editingDesc}
            <div class="desc-editor">
              <textarea
                bind:value={descDraft}
                bind:this={descInputEl}
                rows="6"
                aria-label="任务描述"
                placeholder="补充背景、范围或验收标准"
                onkeydown={(event) => {
                  if (event.key === 'Escape') editingDesc = false;
                }}
              ></textarea>
              <div class="desc-editor-actions">
                <button class="secondary-button" type="button" onclick={() => (editingDesc = false)}>取消</button>
                <button class="primary-button" type="button" onclick={() => void commitDesc()}>保存</button>
              </div>
            </div>
          {:else if task.description}
            <p class="desc-body">{task.description}</p>
            <button class="ghost edit-trigger" type="button" onclick={startEditDesc}>编辑描述</button>
          {:else}
            <button class="ghost" type="button" onclick={startEditDesc}>＋ 添加描述</button>
          {/if}
        </section>

        <section class="block">
          <div class="block-head">
            <h2>子任务</h2>
            {#if subtasks.length}<span class="count">{doneSubtasks} / {subtasks.length}</span>{/if}
            <span class="spacer"></span>
            <button class="ghost" type="button" onclick={openSubtaskModal} disabled={submitting}>＋ 新建子任务</button>
          </div>
          {#if subtasks.length}
            <div class="subtask-progress">
              <span class="bar"><i style="width: {subtaskProgress}%"></i></span>
              <span class="ratio">{subtaskProgress}%</span>
            </div>
            <div class="subtask-list">
              {#each subtasks as subtask (subtask.id)}
                <div class="subtask" class:done={subtaskDone(subtask)}>
                  <span class="checkbox" class:checked={subtaskDone(subtask)} aria-hidden="true">{subtaskDone(subtask) ? '✓' : ''}</span>
                  <a class="title" href={`/tasks/${subtask.task_key}`} title={subtask.title}>{subtask.title}</a>
                  <span class="status-name">{statusName(subtask.status_id)}</span>
                </div>
              {/each}
            </div>
          {/if}
          <form class="add-inline" onsubmit={quickAddSubtask}>
            <input bind:value={quickSubtaskTitle} placeholder="输入子任务标题,回车创建" aria-label="快捷创建子任务" disabled={submitting} />
          </form>
        </section>

        <section class="block">
          <div class="block-head">
            <h2>附件</h2>
            {#if attachments.length}<span class="count">{attachments.length}</span>{/if}
            <span class="spacer"></span>
            <input type="file" multiple hidden bind:this={taskFileInput} onchange={(event) => uploadFiles(event, 'task')} />
            <button class="ghost" type="button" disabled={uploading} onclick={() => taskFileInput?.click()}>
              {uploading ? '上传中…' : '＋ 上传附件'}
            </button>
          </div>
          {#if attachments.some(isImageAttachment)}
            <div class="attachment-grid">
              {#each attachments.filter(isImageAttachment) as item (item.id)}
                <figure>
                  <a href={attachmentUrl(item.url)} target="_blank" rel="noreferrer">
                    <img src={attachmentUrl(item.url)} alt={item.file_name} loading="lazy" />
                  </a>
                  <figcaption>
                    <span title={item.file_name}>{item.file_name}</span>
                    <button class="text-button" type="button" onclick={() => removeAttachment(item)}>删除</button>
                  </figcaption>
                </figure>
              {/each}
            </div>
          {/if}
          {#if attachments.some((item) => !isImageAttachment(item))}
            <div class="file-list">
              {#each attachments.filter((item) => !isImageAttachment(item)) as item (item.id)}
                <div class="file-row">
                  <a class="file-name" href={attachmentUrl(item.url)} title={item.file_name}>
                    <span class="file-icon" aria-hidden="true">{fileIcon(item.file_name)}</span>
                    <span class="file-title">{item.file_name}</span>
                    <small>{formatBytes(item.byte_size)}</small>
                  </a>
                  <button class="text-button" type="button" onclick={() => removeAttachment(item)}>删除</button>
                </div>
              {/each}
            </div>
          {/if}
          {#if !attachments.length}
            <p class="block-hint">图片直接预览,日志和其他文件可下载查看,单个不超过 10MB。</p>
          {/if}
        </section>

        <section class="block">
          <div class="block-head"><h2>活动</h2></div>
          <div class="tabs" role="tablist" aria-label="活动过滤">
            <button class="tab" class:active={activityTab === 'all'} role="tab" aria-selected={activityTab === 'all'} type="button" onclick={() => (activityTab = 'all')}>全部</button>
            <button class="tab" class:active={activityTab === 'comments'} role="tab" aria-selected={activityTab === 'comments'} type="button" onclick={() => (activityTab = 'comments')}>评论<span class="n">{comments.length}</span></button>
            <button class="tab" class:active={activityTab === 'changes'} role="tab" aria-selected={activityTab === 'changes'} type="button" onclick={() => (activityTab = 'changes')}>变更记录<span class="n">{changeLogs.length}</span></button>
          </div>

          <form class="composer" onsubmit={addComment}>
            {#if pending.some(isImageAttachment)}
              <div class="pending-images">
                {#each pending.filter(isImageAttachment) as item (item.id)}
                  <span class="pending-image">
                    <img src={attachmentUrl(item.url)} alt={item.file_name} />
                    <button class="text-button" type="button" aria-label={`移除 ${item.file_name}`} onclick={() => removePending(item)}>×</button>
                  </span>
                {/each}
              </div>
            {/if}
            {#if pending.some((item) => !isImageAttachment(item))}
              <div class="file-list pending-files">
                {#each pending.filter((item) => !isImageAttachment(item)) as item (item.id)}
                  <div class="file-row">
                    <a class="file-name" href={attachmentUrl(item.url)} title={item.file_name} target="_blank" rel="noreferrer">
                      <span class="file-icon" aria-hidden="true">{fileIcon(item.file_name)}</span>
                      <span class="file-title">{item.file_name}</span>
                      <small>{formatBytes(item.byte_size)}</small>
                    </a>
                    <button class="text-button" type="button" aria-label={`移除 ${item.file_name}`} onclick={() => removePending(item)}>移除</button>
                  </div>
                {/each}
              </div>
            {/if}
            <textarea bind:value={commentBody} rows="3" placeholder="添加评论…支持 @提及项目成员" aria-label="评论内容"></textarea>
            <div class="composer-foot">
              <span class="hint">@名字 会给对方发站内通知；支持图片、日志和其他文件</span>
              <input type="file" multiple hidden bind:this={commentFileInput} onchange={(event) => uploadFiles(event, 'pending')} />
              <button class="secondary-button" type="button" disabled={uploading} onclick={() => commentFileInput?.click()}>
                {uploading ? '上传中…' : '添加文件'}
              </button>
              <button class="primary-button" type="submit" disabled={submitting}>评论</button>
            </div>
          </form>

          <div class="feed">
            {#each visibleFeed as item (item.kind === 'comment' ? item.comment.id : item.log.id)}
              {#if item.kind === 'comment'}
                <article class="event">
                  <span class="glyph comment-glyph"><Avatar name={item.comment.author_name} size={22} /></span>
                  <div class="body">
                    <div class="head">
                      <strong>{item.comment.author_name}</strong>
                      <span class="tag">评论</span>
                      <time>{fmtTime(item.comment.created_at)}</time>
                      <button class="text-button" type="button" onclick={() => removeComment(item.comment)}>删除</button>
                    </div>
                    <p class="content">{@html renderComment(item.comment.body)}</p>
                    {#if item.comment.attachments?.some(isImageAttachment)}
                      <div class="comment-images">
                        {#each item.comment.attachments.filter(isImageAttachment) as attachment (attachment.id)}
                          <a href={attachmentUrl(attachment.url)} target="_blank" rel="noreferrer" title={attachment.file_name}>
                            <img src={attachmentUrl(attachment.url)} alt={attachment.file_name} loading="lazy" />
                          </a>
                        {/each}
                      </div>
                    {/if}
                    {#if item.comment.attachments?.some((attachment) => !isImageAttachment(attachment))}
                      <div class="file-list comment-files">
                        {#each item.comment.attachments.filter((attachment) => !isImageAttachment(attachment)) as attachment (attachment.id)}
                          <div class="file-row">
                            <a class="file-name" href={attachmentUrl(attachment.url)} target="_blank" rel="noreferrer" title={attachment.file_name}>
                              <span class="file-icon" aria-hidden="true">{fileIcon(attachment.file_name)}</span>
                              <span class="file-title">{attachment.file_name}</span>
                              <small>{formatBytes(attachment.byte_size)}</small>
                            </a>
                          </div>
                        {/each}
                      </div>
                    {/if}
                  </div>
                </article>
              {:else}
                <article class="event">
                  <span class="glyph" aria-hidden="true">↻</span>
                  <div class="body">
                    <div class="head">
                      <strong>{actorName(item.log)}</strong>
                      <span class="event-text" title={item.log.summary}>{item.log.summary}</span>
                      <span class="tag">变更</span>
                      <time>{fmtTime(item.log.created_at)}</time>
                    </div>
                    {#if diffEntries(item.log).length}
                      <div class="detail">
                        {#each diffEntries(item.log) as [field, value] (field)}
                          <span><b>{fieldLabels[field] ?? field}</b>{renderValue(value)}</span>
                        {/each}
                      </div>
                    {/if}
                  </div>
                </article>
              {/if}
            {:else}
              <p class="block-hint">还没有相关活动。</p>
            {/each}
          </div>
        </section>
      </main>

      <aside>
        <div class="panel">
          <div class="panel-title">
            属性
            <span class="info tooltip" data-tip="点击即改、失焦即存;负责人/评审人/排期改动立即生效,并记入活动流。">i</span>
          </div>
          <div class="props">
            <label class="prop">
              <span class="prop-label">状态</span>
              <span class="prop-value">
                <select value={task.status_id} onchange={changeStatus} disabled={!statusControl.canChange || submitting} aria-label="任务状态" title={statusControl.canChange ? '' : '仅负责人或评审人可变更状态'}>
                  {#each statusControl.options as status (status.id)}
                    <option value={status.id}>{status.name}</option>
                  {/each}
                  {#if !statusControl.options.length}
                    <option value={task.status_id}>{statusName(task.status_id)}</option>
                  {/if}
                </select>
              </span>
            </label>
            <div class="prop">
              <span class="prop-label">负责人</span>
              <span class="prop-value picker">
                <MemberPicker
                  value={task.assignee_id}
                  {members}
                  disabled={propBusy}
                  onchange={(value) => saveField({ assignee_id: value })}
                  ariaLabel="设置负责人"
                />
              </span>
            </div>
            <div class="prop">
              <span class="prop-label">评审人</span>
              <span class="prop-value picker">
                <MemberPicker
                  value={task.reviewer_id}
                  {members}
                  disabled={propBusy}
                  onchange={(value) => saveField({ reviewer_id: value })}
                  ariaLabel="设置评审人"
                />
              </span>
            </div>
            <label class="prop">
              <span class="prop-label">类型</span>
              <span class="prop-value">
                <select value={task.task_type} onchange={(event) => saveField({ task_type: (event.currentTarget as HTMLSelectElement).value })} disabled={propBusy} aria-label="任务类型">
                  {#each taskTypeOptions as option (option.value)}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </span>
            </label>
            <label class="prop">
              <span class="prop-label">优先级</span>
              <span class="prop-value">
                <select value={task.priority} onchange={(event) => saveField({ priority: (event.currentTarget as HTMLSelectElement).value as Priority })} disabled={propBusy} aria-label="优先级">
                  {#each priorityOptions as option (option.value)}
                    <option value={option.value}>{option.label}</option>
                  {/each}
                </select>
              </span>
            </label>
            <label class="prop">
              <span class="prop-label">里程碑</span>
              <span class="prop-value">
                <select
                  value={task.milestone_id ?? ''}
                  onchange={(event) => {
                    const next = (event.currentTarget as HTMLSelectElement).value;
                    void saveField({ milestone_id: next || null });
                  }}
                  disabled={propBusy}
                  aria-label="关联里程碑"
                >
                  <option value="">未关联</option>
                  {#each milestones as milestone (milestone.id)}
                    <option value={milestone.id}>{milestone.name}{milestone.due_date ? ` · ${milestone.due_date}` : ''}</option>
                  {/each}
                </select>
              </span>
            </label>
            <div class="prop prop-labels">
              <span class="prop-label">标签</span>
              <span class="prop-value wrap">
                {#if task.labels.length}
                  {#each task.labels as label (label.id)}
                    <LabelPill name={label.name} onremove={labelBusy ? undefined : () => detachLabel(label.id)} />
                  {/each}
                {/if}
                {#if addLabelOpen}
                  <form class="label-form" onsubmit={submitLabel}>
                    <input bind:value={labelInput} placeholder="标签名,回车添加" aria-label="新标签名称" disabled={labelBusy} />
                  </form>
                  {#if labels.length}
                    <div class="label-suggestions">
                      {#each labels.filter((label) => !task!.labels.some((item) => item.id === label.id)) as label (label.id)}
                        <button class="label-suggest" type="button" disabled={labelBusy} onclick={() => (labelInput = label.name)}>+ {label.name}</button>
                      {/each}
                    </div>
                  {/if}
                {:else}
                  <button class="add-link" type="button" onclick={() => (addLabelOpen = true)}>＋ 添加</button>
                {/if}
              </span>
            </div>
            <label class="prop">
              <span class="prop-label">开始</span>
              <span class="prop-value">
                <input
                  class="date-input"
                  type="datetime-local"
                  value={task.start_at ? isoToLocalInput(task.start_at) : ''}
                  onchange={(event) => commitDate('start_at', event)}
                  disabled={propBusy}
                  aria-label="开始时间"
                />
              </span>
            </label>
            <label class="prop">
              <span class="prop-label">截止</span>
              <span class="prop-value">
                <input
                  class="date-input"
                  type="datetime-local"
                  value={task.due_at ? isoToLocalInput(task.due_at) : ''}
                  onchange={(event) => commitDate('due_at', event)}
                  disabled={propBusy}
                  aria-label="截止时间"
                />
              </span>
            </label>
          </div>
          <div class="panel-foot">字段改动会记入活动流</div>
        </div>

        <div class="panel">
          <div class="panel-title">
            依赖
            <span class="info tooltip" data-tip="仅支持同项目任务;系统会阻止自依赖与循环依赖。">i</span>
          </div>
          {#if dependencies}
            {#if blockedByIncomplete.length}
              <p class="dep-warning">还有 {blockedByIncomplete.length} 个未完成的依赖阻塞当前任务。</p>
            {/if}
            <div class="dep-list">
              <div class="dep-group">
                <span class="dep-dir">阻塞我</span>
                {#each dependencies.blocked_by as item (item.dependency_id)}
                  <div class="dep-row">
                    <a href={`/tasks/${item.task_key}`} class:done={item.is_done} title={item.title}>
                      <code>{item.task_key}</code>
                      <span class="dep-title">{item.title}</span>
                      <em>{item.status_name}</em>
                    </a>
                    <button class="text-button" type="button" disabled={dependencyBusy} onclick={() => detachDependency(item.dependency_id)}>移除</button>
                  </div>
                {:else}
                  <span class="dep-empty">无</span>
                {/each}
              </div>
              <div class="dep-group">
                <span class="dep-dir">我阻塞</span>
                {#each dependencies.blocks as item (item.dependency_id)}
                  <div class="dep-row">
                    <a href={`/tasks/${item.task_key}`} class:done={item.is_done} title={item.title}>
                      <code>{item.task_key}</code>
                      <span class="dep-title">{item.title}</span>
                      <em>{item.status_name}</em>
                    </a>
                    <button class="text-button" type="button" disabled={dependencyBusy} onclick={() => detachDependency(item.dependency_id)}>移除</button>
                  </div>
                {:else}
                  <span class="dep-empty">无</span>
                {/each}
              </div>
            </div>
            {#if addDepOpen}
              <select class="dep-select" bind:value={dependencyTarget} onchange={pickDependency} disabled={dependencyBusy} aria-label="选择依赖任务">
                <option value="">选择要依赖的任务…</option>
                {#each dependencyOptions as option (option.id)}
                  <option value={option.task_key}>{option.task_key} · {option.title}</option>
                {/each}
              </select>
            {:else}
              <button class="ghost panel-ghost" type="button" onclick={() => (addDepOpen = true)}>＋ 添加依赖</button>
            {/if}
          {/if}
        </div>

        <div class="panel">
          <div class="panel-title">
            任务归属
            <span class="info tooltip" data-tip="挂靠后成为所选任务的子任务,在看板卡片与列表行显示「↳ 父任务」标识;子任务不能再挂靠。">i</span>
          </div>
          {#if parentTask}
            <div class="dep-list">
              <div class="dep-group">
                <span class="dep-dir">父任务</span>
                <a class="dep-row parent-link" href={`/tasks/${parentTask.task_key}`}>
                  <code>{parentTask.task_key}</code>
                  <span class="dep-title">{parentTask.title}</span>
                </a>
              </div>
            </div>
            <button class="ghost panel-ghost" type="button" onclick={() => changeParent(null)} disabled={reparenting}>
              {reparenting ? '处理中…' : '脱离父任务'}
            </button>
          {:else if addParentOpen}
            <select
              class="dep-select"
              bind:value={attachParentId}
              disabled={reparenting}
              aria-label="选择父任务"
              onchange={(event) => {
                const next = (event.currentTarget as HTMLSelectElement).value;
                if (next) void changeParent(next);
              }}
            >
              <option value="">选择要挂靠的任务…</option>
              {#each rootTasks as root (root.id)}
                {#if root.id !== task.id}
                  <option value={root.id}>{root.task_key} · {root.title}</option>
                {/if}
              {/each}
            </select>
          {:else}
            <button class="ghost panel-ghost" type="button" onclick={() => (addParentOpen = true)}>＋ 设为子任务</button>
          {/if}
        </div>
      </aside>
    </div>
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
    <div class="subtask-attachments-field">
      <span>附件</span>
      {#if subtaskFiles.some((item) => item.previewUrl)}
        <div class="subtask-image-previews">
          {#each subtaskFiles.filter((item) => item.previewUrl) as item (item.file.name + item.file.lastModified)}
            <span class="subtask-image-preview">
              <img src={item.previewUrl ?? ''} alt={item.file.name} />
              <button type="button" aria-label={`移除 ${item.file.name}`} onclick={() => removeSubtaskFile(item)}>×</button>
            </span>
          {/each}
        </div>
      {/if}
      {#if subtaskFiles.some((item) => !item.previewUrl)}
        <div class="file-list subtask-files">
          {#each subtaskFiles.filter((item) => !item.previewUrl) as item (item.file.name + item.file.lastModified)}
            <div class="file-row">
              <span class="file-name"><span class="file-icon" aria-hidden="true">{fileIcon(item.file.name)}</span><span class="file-title">{item.file.name}</span><small>{formatBytes(item.file.size)}</small></span>
              <button class="text-button" type="button" aria-label={`移除 ${item.file.name}`} onclick={() => removeSubtaskFile(item)}>移除</button>
            </div>
          {/each}
        </div>
      {/if}
      <input type="file" multiple hidden bind:this={subtaskFileInput} onchange={addSubtaskFiles} />
      <button class="secondary-button" type="button" onclick={() => subtaskFileInput?.click()} disabled={submitting}>添加文件</button>
      <small>支持图片、日志及其他文件，单个文件不超过 10MB。填写评论时，附件会附在该评论中。</small>
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
  .task-page { display: grid; gap: 18px; }

  /* ── 面包屑与头部 ── */
  .breadcrumb { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--color-text-muted); }
  .breadcrumb a { color: var(--color-text-muted); }
  .breadcrumb a:hover { color: var(--color-text); text-decoration: none; }
  .task-header { display: grid; gap: 10px; }
  .title-row { display: flex; align-items: flex-start; gap: 12px; }
  .task-title {
    flex: 1; min-width: 0;
    margin: 0; padding: 2px 6px; margin-left: -6px;
    border: 0; background: transparent; text-align: left;
    font-family: inherit; font-size: 22px; font-weight: 600; line-height: 1.35;
    color: var(--color-text); cursor: text;
    border-radius: var(--radius-sm);
  }
  .task-title:hover { background: var(--color-hover); }
  .task-title:focus-visible { outline: none; box-shadow: var(--color-focus-ring); }
  .title-input {
    flex: 1; min-width: 0; padding: 2px 6px;
    font-size: 22px; font-weight: 600; line-height: 1.35;
    border: 1px solid var(--color-primary); border-radius: var(--radius-sm);
    background: var(--color-surface); color: var(--color-text);
    box-shadow: var(--color-focus-ring);
  }
  .header-actions { display: flex; gap: 8px; flex: none; }
  .icon-btn {
    padding: 6px 12px; border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: var(--color-surface); color: var(--color-text-secondary); font-size: 12px;
    transition: border-color var(--transition-fast), color var(--transition-fast);
  }
  .icon-btn:hover { border-color: var(--color-border-strong); color: var(--color-text); }
  .icon-btn.danger:hover { border-color: var(--color-danger); color: var(--color-danger); }
  .icon-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .meta-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; font-size: 13px; color: var(--color-text-muted); }
  .meta-item { display: inline-flex; align-items: center; gap: 6px; }
  .meta-item .name { color: var(--color-text-secondary); }
  .meta-item.overdue { color: var(--color-danger); font-weight: 500; }
  .mono { font-family: var(--font-mono); }
  .status-pill { display: inline-flex; align-items: center; gap: 7px; padding: 2px 10px; border-radius: 999px; font-size: 12px; }
  .status-pill .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--color-text-muted); }
  .status-pill.cat-todo { background: var(--color-hover); color: var(--color-text-secondary); }
  .status-pill.cat-in_progress { background: var(--color-primary-soft); color: var(--color-primary-strong); }
  .status-pill.cat-in_progress .dot { background: var(--color-primary); }
  .status-pill.cat-done { background: color-mix(in srgb, var(--color-success) 12%, transparent); color: var(--color-success); }
  .status-pill.cat-done .dot { background: var(--color-success); }
  .error-banner { padding: 8px 12px; border: 1px solid var(--color-danger); border-left-width: 3px; border-radius: var(--radius-md); color: var(--color-danger); font-size: 13px; }

  /* ── 双栏布局 ── */
  .layout { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 28px; align-items: start; }

  /* ── 主栏区块 ── */
  .block { padding: 18px 0; border-top: 1px solid var(--color-border-weak); }
  .block:first-child { border-top: 0; padding-top: 0; }
  .block-head { display: flex; align-items: baseline; gap: 8px; margin-bottom: 12px; }
  .block-head h2 { font-size: 13px; font-weight: 600; color: var(--color-text-secondary); letter-spacing: 0.02em; }
  .block-head .count { font-size: 12px; color: var(--color-text-muted); font-family: var(--font-mono); }
  .block-head .spacer { flex: 1; }
  .block-hint { color: var(--color-text-muted); font-size: 12px; }

  .ghost {
    display: inline-flex; align-items: center; gap: 6px;
    padding: 5px 10px; border: 1px dashed var(--color-border); border-radius: var(--radius-sm);
    color: var(--color-text-muted); font-size: 12px;
    transition: color var(--transition-fast), border-color var(--transition-fast);
  }
  .ghost:hover { color: var(--color-primary-strong); border-color: var(--color-primary); }
  .ghost:disabled { opacity: 0.5; cursor: not-allowed; }
  .edit-trigger { margin-top: 8px; }

  .desc-body { color: var(--color-text-secondary); font-size: 14px; line-height: 1.7; white-space: pre-wrap; }
  .desc-editor { display: grid; gap: 8px; }
  .desc-editor textarea { width: 100%; resize: vertical; }
  .desc-editor-actions { display: flex; justify-content: flex-end; gap: 8px; }

  .subtask-progress { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; }
  .subtask-progress .bar { width: 180px; height: 4px; border-radius: 2px; background: var(--color-hover); overflow: hidden; }
  .subtask-progress .bar i { display: block; height: 100%; background: var(--color-success); }
  .subtask-progress .ratio { font-size: 12px; color: var(--color-text-muted); font-family: var(--font-mono); }
  .subtask-list { display: grid; }
  .subtask { display: flex; align-items: center; gap: 10px; padding: 7px 8px; margin: 0 -8px; border-radius: var(--radius-sm); font-size: 13px; }
  .subtask:hover { background: var(--color-hover); }
  .subtask .checkbox { width: 16px; height: 16px; border-radius: 4px; border: 1.5px solid var(--color-text-muted); display: inline-flex; align-items: center; justify-content: center; color: #fff; font-size: 10px; flex: none; }
  .subtask .checkbox.checked { background: var(--color-success); border-color: var(--color-success); }
  .subtask .title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-text); text-decoration: none; }
  .subtask:hover .title { color: var(--color-primary); }
  .subtask.done .title { color: var(--color-text-muted); text-decoration: line-through; }
  .subtask .status-name { font-size: 12px; color: var(--color-text-muted); flex: none; }
  .add-inline { margin-top: 6px; }
  .add-inline input { width: 100%; padding: 7px 10px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-sunken); color: var(--color-text); font-size: 13px; }
  .add-inline input:focus-visible { outline: none; border-color: var(--color-primary); box-shadow: var(--color-focus-ring); }

  .attachment-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr)); gap: 12px; margin-bottom: 10px; }
  .attachment-grid figure { display: grid; gap: 6px; min-width: 0; margin: 0; }
  .attachment-grid img, .comment-images img, .pending-images img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .attachment-grid a { display: block; aspect-ratio: 4 / 3; border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; background: var(--color-surface-sunken); }
  .attachment-grid a:hover { border-color: var(--color-border-strong); }
  .attachment-grid figcaption { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: 12px; color: var(--color-text-muted); }
  .attachment-grid figcaption span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-list { display: grid; gap: 6px; margin-bottom: 10px; }
  .file-row { display: flex; align-items: center; gap: 10px; padding: 8px 10px; background: var(--color-surface-sunken); border: 1px solid var(--color-border-weak); border-radius: var(--radius-md); font-size: 13px; }
  .file-name { display: inline-flex; align-items: center; gap: 8px; flex: 1; min-width: 0; color: var(--color-text); text-decoration: none; }
  .file-name:hover .file-title { color: var(--color-primary); }
  .file-icon { color: var(--color-text-muted); }
  .file-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-name small { flex: none; color: var(--color-text-muted); font-size: 12px; }
  .text-button { border: 0; background: transparent; color: var(--color-danger); font-size: 12px; font-weight: 500; cursor: pointer; }
  .text-button:disabled { opacity: 0.5; cursor: not-allowed; }

  /* ── 活动流 ── */
  .tabs { display: flex; gap: 4px; margin-bottom: 14px; border-bottom: 1px solid var(--color-border-weak); }
  .tab { padding: 6px 12px 8px; border: 0; border-bottom: 2px solid transparent; margin-bottom: -1px; background: transparent; color: var(--color-text-muted); font-size: 13px; border-radius: var(--radius-sm) var(--radius-sm) 0 0; cursor: pointer; }
  .tab:hover { color: var(--color-text-secondary); }
  .tab.active { color: var(--color-text); border-bottom-color: var(--color-text); font-weight: 500; }
  .tab .n { font-size: 11px; color: var(--color-text-muted); margin-left: 4px; font-family: var(--font-mono); }
  .composer { display: grid; gap: 8px; margin-bottom: 18px; }
  .composer textarea { width: 100%; resize: vertical; }
  .composer-foot { display: flex; align-items: center; gap: 10px; }
  .composer-foot .hint { font-size: 12px; color: var(--color-text-muted); flex: 1; }
  .composer-foot .secondary-button, .composer-foot .primary-button { border: 0; }
  .pending-images { display: flex; flex-wrap: wrap; gap: 8px; }
  .pending-files, .comment-files, .subtask-files { margin-top: 8px; }
  .pending-image { position: relative; display: block; width: 96px; height: 72px; border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; }
  .pending-image button { position: absolute; top: 2px; right: 2px; width: 20px; height: 20px; display: grid; place-items: center; background: rgba(0, 0, 0, 0.6); color: #fff; font-size: 14px; border-radius: var(--radius-sm); }
  .feed { display: grid; gap: 2px; }
  .event { display: flex; gap: 12px; padding: 9px 8px; margin: 0 -8px; border-radius: var(--radius-sm); }
  .event:hover { background: var(--color-hover); }
  .event .glyph { flex: none; width: 26px; height: 26px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; font-size: 12px; background: var(--color-hover); color: var(--color-text-muted); overflow: hidden; }
  .event .glyph.comment-glyph { background: transparent; overflow: visible; }
  .event .body { flex: 1; min-width: 0; font-size: 13px; }
  .event .head { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .event .head strong { font-weight: 500; color: var(--color-text-secondary); flex: none; }
  .event .head .event-text { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-text-secondary); }
  .event .head time { font-size: 12px; color: var(--color-text-muted); margin-left: auto; flex: none; }
  .event .head .tag { flex: none; font-size: 11px; padding: 1px 7px; border-radius: 999px; border: 1px solid var(--color-border); color: var(--color-text-muted); }
  .event .content { margin: 4px 0 0; white-space: pre-wrap; color: var(--color-text-secondary); line-height: 1.6; }
  .event .content :global(.mention) { padding: 0 2px; border-radius: var(--radius-sm); background: var(--color-primary-soft); color: var(--color-primary-strong); font-weight: 500; }
  .comment-images { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
  .comment-images a { display: block; width: 96px; height: 72px; border: 1px solid var(--color-border); border-radius: var(--radius-md); overflow: hidden; }
  .comment-images a:hover { border-color: var(--color-border-strong); }
  .event .detail { display: grid; gap: 3px; margin-top: 6px; width: fit-content; max-width: 100%; padding: 6px 10px; border-radius: var(--radius-sm); background: var(--color-surface-sunken); font-family: var(--font-mono); font-size: 12px; color: var(--color-text-muted); }
  .event .detail b { color: var(--color-text-secondary); font-weight: 500; }
  .event .detail b::after { content: '：'; }

  /* ── 侧栏面板 ── */
  .panel { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-lg); overflow: hidden; }
  .panel + .panel { margin-top: 14px; }
  .panel-title { display: flex; align-items: center; gap: 6px; padding: 12px 14px 10px; border-bottom: 1px solid var(--color-border-weak); font-size: 12px; font-weight: 600; color: var(--color-text-secondary); letter-spacing: 0.04em; }
  .panel-title .info { margin-left: auto; cursor: help; color: var(--color-text-muted); font-weight: 500; font-style: italic; font-family: var(--font-mono); }
  .tooltip { position: relative; display: inline-flex; }
  .tooltip::after {
    content: attr(data-tip);
    position: absolute; right: 0; top: calc(100% + 6px); z-index: 5;
    width: 220px; padding: 8px 10px; border-radius: var(--radius-sm);
    background: var(--color-surface-sunken); border: 1px solid var(--color-border);
    color: var(--color-text-muted); font-size: 12px; font-weight: 400; letter-spacing: 0;
    line-height: 1.5; text-align: left; white-space: normal;
    opacity: 0; visibility: hidden; transition: opacity var(--transition-fast);
    pointer-events: none;
  }
  .tooltip:hover::after { opacity: 1; visibility: visible; }
  .props { display: grid; }
  .prop { display: flex; align-items: center; gap: 10px; padding: 7px 14px; font-size: 13px; }
  .prop:hover { background: var(--color-hover); }
  .prop-label { flex: none; width: 48px; color: var(--color-text-muted); font-size: 12px; }
  .prop-value { flex: 1; min-width: 0; display: inline-flex; align-items: center; }
  .prop-value.wrap { flex-wrap: wrap; gap: 6px; padding: 2px 0 6px; }
  .prop select, .prop .date-input { width: 100%; min-width: 0; padding: 4px 8px; border: 1px solid transparent; border-radius: var(--radius-sm); background: transparent; color: var(--color-text); font-size: 13px; }
  .prop select:hover, .prop .date-input:hover { border-color: var(--color-border); background: var(--color-surface); }
  .prop select:focus-visible, .prop .date-input:focus-visible { outline: none; border-color: var(--color-primary); box-shadow: var(--color-focus-ring); }
  .prop .picker { width: 100%; }
  .prop .picker :global(.member-picker) { width: 100%; }
  .prop .picker :global(select) { width: 100%; min-width: 0; padding: 4px 8px; border: 1px solid transparent; border-radius: var(--radius-sm); background: transparent; color: var(--color-text); font-size: 13px; }
  .prop .picker :global(.member-picker:hover select) { border-color: var(--color-border); background: var(--color-surface); }
  .panel-foot { padding: 8px 14px 10px; border-top: 1px solid var(--color-border-weak); font-size: 11px; color: var(--color-text-muted); }
  .label-form input { width: 110px; padding: 3px 8px; border: 1px solid var(--color-border); border-radius: var(--radius-sm); background: var(--color-surface); color: var(--color-text); font-size: 12px; }
  .label-form input:focus-visible { outline: none; border-color: var(--color-primary); box-shadow: var(--color-focus-ring); }
  .label-suggestions { display: flex; flex-wrap: wrap; gap: 4px; width: 100%; }
  .label-suggest { border: 0; background: transparent; color: var(--color-primary-strong); font-size: 12px; cursor: pointer; }
  .add-link { border: 0; background: transparent; color: var(--color-text-muted); font-size: 12px; cursor: pointer; padding: 0; }
  .add-link:hover { color: var(--color-primary-strong); }

  .dep-warning { margin: 10px 14px 0; color: var(--color-warning); font-size: 12px; }
  .dep-list { padding: 8px 14px; display: grid; gap: 8px; }
  .dep-group { display: grid; gap: 4px; min-width: 0; }
  .dep-dir { color: var(--color-text-muted); font-size: 12px; }
  .dep-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .dep-row > a { display: grid; grid-template-columns: 86px minmax(0, 1fr); grid-template-rows: auto auto; align-items: center; gap: 2px 8px; flex: 1; min-width: 0; color: var(--color-text); text-decoration: none; font-size: 13px; }
  .dep-row > a code { font-family: var(--font-mono); font-size: 11px; color: var(--color-primary-strong); }
  .dep-row .dep-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-text-muted); font-size: 12px; }
  .dep-row > a em { grid-column: 2; font-style: normal; font-size: 11px; color: var(--color-success); }
  .dep-row a.done { opacity: 0.55; }
  .dep-row .text-button { font-size: 11px; }
  .dep-empty { color: var(--color-text-muted); font-size: 12px; }
  .dep-select, .panel-ghost { width: calc(100% - 28px); margin: 4px 14px 12px; }
  .dep-select { padding: 6px 10px; border: 1px solid var(--color-border); border-radius: var(--radius-sm); background: var(--color-surface-sunken); color: var(--color-text); font-size: 12px; }
  .panel-ghost { justify-content: center; margin-top: 10px; }

  /* ── 子任务弹窗 ── */
  .subtask-modal-form { display: grid; gap: 14px; }
  .subtask-modal-form label, .subtask-attachments-field { display: grid; gap: 6px; }
  .subtask-modal-form label > span, .subtask-attachments-field > span { color: var(--color-text-secondary); font-size: 13px; font-weight: 500; }
  .subtask-modal-form em { color: var(--color-danger); font-style: normal; }
  .subtask-modal-form input, .subtask-modal-form textarea { width: 100%; min-width: 0; }
  .subtask-modal-form textarea { resize: vertical; }
  .subtask-form-row { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .subtask-image-previews { display: flex; flex-wrap: wrap; gap: 8px; }
  .subtask-image-preview { position: relative; display: block; width: 88px; height: 66px; overflow: hidden; border: 1px solid var(--color-border); border-radius: var(--radius-md); }
  .subtask-image-preview img { display: block; width: 100%; height: 100%; object-fit: cover; }
  .subtask-image-preview button { position: absolute; top: 2px; right: 2px; display: grid; width: 20px; height: 20px; place-items: center; border: 0; border-radius: var(--radius-sm); background: rgba(0, 0, 0, 0.65); color: #fff; cursor: pointer; }
  .subtask-attachments-field small { color: var(--color-text-muted); font-size: 12px; line-height: 1.5; }
  .error-message { color: var(--color-danger); font-size: 13px; }

  .state-box { display: grid; place-items: center; gap: 12px; min-height: 220px; }
  .error-state { color: var(--color-danger); }
  @media (max-width: 900px) {
    .layout { grid-template-columns: 1fr; }
  }
  @media (max-width: 560px) {
    .title-row { flex-direction: column; }
    .header-actions { justify-content: flex-start; }
    .subtask-form-row { grid-template-columns: 1fr; }
  }
</style>
