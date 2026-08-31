<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { confirmDialog } from '$lib/features/ui/dialog.svelte';
  import { listStatuses, listProjectMembers } from '$lib/api/projects';
  import { deleteAttachment, listTaskAttachments, uploadTaskAttachment, attachmentUrl } from '$lib/api/attachments';
  import { createComment, createSubtask, deleteComment, deleteTask, getSubtasks, getTask, listComments, transitionTask, updateTask } from '$lib/api/tasks';
  import type { Attachment, Comment, ProjectMember, ProjectStatus, TaskView } from '$lib/api/types';
  import MemberPicker from '$lib/features/task-list/MemberPicker.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';

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
  let selectedStatus = $state('');
  let subtaskTitle = $state('');
  let loading = $state(true);
  let submitting = $state(false);
  let uploading = $state(false);
  let deleting = $state(false);
  let errorMessage = $state('');
  let taskFileInput = $state<HTMLInputElement | null>(null);
  let commentFileInput = $state<HTMLInputElement | null>(null);
  const statusName = (id: string) => statuses.find((status) => status.id === id)?.name || id.slice(0, 8);
  const priorityName: Record<string, string> = { urgent: '紧急', high: '高', medium: '中', low: '低', none: '无' };

  async function load() {
    loading = true;
    errorMessage = '';
    try {
      const taskResponse = await getTask(taskKey);
      task = taskResponse.data;
      const [subtaskResponse, statusResponse, commentResponse, attachmentResponse, memberResponse] = await Promise.all([
        getSubtasks(taskKey),
        listStatuses(projectKey),
        listComments(taskKey),
        listTaskAttachments(taskKey),
        listProjectMembers(projectKey)
      ]);
      subtasks = subtaskResponse.data;
      statuses = statusResponse.data;
      comments = commentResponse.data;
      attachments = attachmentResponse.data;
      members = memberResponse.data.items;
      selectedStatus = task.status_id;
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

  async function addSubtask(event: SubmitEvent) {
    event.preventDefault();
    if (!subtaskTitle.trim()) return;
    submitting = true;
    try {
      await createSubtask(taskKey, { title: subtaskTitle.trim(), status_id: selectedStatus || undefined });
      subtaskTitle = '';
      subtasks = (await getSubtasks(taskKey)).data;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '子任务创建失败';
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

  async function changeAssignee(assigneeId: string | null) {
    if (!task || assigneeId === task.assignee_id) return;
    submitting = true;
    try {
      task = (await updateTask(task.task_key, { assignee_id: assigneeId })).data;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '负责人修改失败';
    } finally {
      submitting = false;
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
      { label: task.task_key }
    ]}
    description={task.description || '暂无描述。'}
  />
  <div class="detail-grid">
    <section class="workspace-card main-card">
      <div class="field-grid">
        <div>
          <span class="field-label">任务状态</span>
          <select bind:value={selectedStatus} onchange={changeStatus} disabled={submitting}>
            {#each statuses as status}<option value={status.id}>{status.name}</option>{/each}
            {#if !statuses.length}<option value={task.status_id}>{statusName(task.status_id)}</option>{/if}
          </select>
        </div>
        <div>
          <span class="field-label">负责人</span>
          <MemberPicker value={task.assignee_id} {members} disabled={submitting} onchange={changeAssignee} ariaLabel={`设置 ${task.title} 的负责人`} />
        </div>
        <div><span class="field-label">优先级</span><strong class="priority">{priorityName[task.priority]}</strong></div>
        <div><span class="field-label">任务编号</span><strong class="mono">#{task.task_number}</strong></div>
        <div><span class="field-label">更新时间</span><strong>{new Date(task.updated_at).toLocaleString('zh-CN')}</strong></div>
      </div>
      <div class="description-block">
        <span class="field-label">任务描述</span>
        <p>{task.description || '暂无描述内容。'}</p>
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
        <span>{subtasks.length} 项</span>
      </div>
      <div class="subtask-list">
        {#each subtasks as subtask}
          <a href={`/tasks/${subtask.task_key}`}>
            <span class="task-key">{subtask.task_key}</span>
            <strong>{subtask.title}</strong>
            <span class="status-pill">{statusName(subtask.status_id)}</span>
          </a>
        {:else}
          <div class="empty-inline">还没有子任务。</div>
        {/each}
      </div>
      <form class="subtask-form" onsubmit={addSubtask}>
        <input bind:value={subtaskTitle} placeholder="添加一个子任务" aria-label="子任务标题" />
        <button class="primary-button" type="submit" disabled={submitting}>{submitting ? '添加中…' : '添加子任务'}</button>
      </form>
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
      <h2>操作</h2>
      <p>删除采用逻辑删除，动作会写入项目操作日志。</p>
      <button class="danger-button" type="button" onclick={removeTask} disabled={deleting}>{deleting ? '删除中…' : '逻辑删除任务'}</button>
      {#if errorMessage}<p class="error-message">{errorMessage}</p>{/if}
    </aside>
  </div>
{/if}

<style>
  h2, p { margin: 0; }
  .detail-grid { display: grid; grid-template-columns: minmax(0, 1fr) 290px; gap: 18px; }
  .main-card { display: grid; gap: 24px; }
  .field-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px; padding-bottom: 20px; border-bottom: 1px solid var(--color-border); }
  .field-grid > div { display: grid; gap: 7px; min-width: 0; }
  .field-label { color: var(--color-text-muted); font-size: 12px; font-weight: 500; }
  .field-grid select { min-width: 0; }
  .priority { color: var(--color-warning); }
  .mono { font-family: var(--font-mono); }
  .description-block { display: grid; gap: 8px; }
  .description-block p { white-space: pre-wrap; color: var(--color-text-secondary); line-height: 1.7; }
  .subtask-heading { display: flex; justify-content: space-between; gap: 12px; align-items: flex-start; }
  .subtask-heading h2, .side-card h2 { font-size: 18px; }
  .subtask-heading p, .side-card p { margin-top: 5px; color: var(--color-text-muted); font-size: 13px; }
  .subtask-heading > span { color: var(--color-text-muted); font-size: 13px; }
  .subtask-list { display: grid; }
  .subtask-list > a { display: grid; grid-template-columns: 105px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 12px 0; border-top: 1px solid var(--color-border); }
  .subtask-list > a:hover strong { color: var(--color-primary); }
  .task-key { color: var(--color-primary-strong); font-family: var(--font-mono); font-size: 12px; }
  .empty-inline { padding: 12px 0; color: var(--color-text-muted); font-size: 13px; }
  .subtask-form { display: flex; gap: 8px; }
  .subtask-form input { flex: 1; min-width: 0; }
  .subtask-form button { border: 0; white-space: nowrap; }
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
  .danger-button { border: 0; }
  .error-message { color: var(--color-danger); font-size: 13px; }
  .state-box { display: grid; place-items: center; gap: 12px; min-height: 220px; }
  .error-state { color: var(--color-danger); }
  @media (max-width: 900px) {
    .detail-grid { grid-template-columns: 1fr; }
  }
  @media (max-width: 560px) {
    .subtask-list > a { grid-template-columns: 1fr; gap: 5px; }
    .subtask-form { display: grid; }
    .subtask-form button { width: 100%; }
    .comment-actions { justify-content: stretch; }
    .comment-actions button { flex: 1; }
  }
</style>
