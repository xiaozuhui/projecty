import { apiDownload, apiGet, apiPatch, apiPost } from './client';
import type { Comment, CrossProjectTaskListResponse, DeletedTaskListResponse, LabelView, ProjectDependencyListResponse, TaskDependencies, TaskListResponse, TaskView } from './types';
const key = (value: string) => encodeURIComponent(value);
export type TaskFilters = {
  statusId?: string;
  parentTaskId?: string;
  taskType?: string;
  keyword?: string;
  assigneeId?: string;
  unassigned?: boolean;
  priority?: string;
  milestoneId?: string;
  labelId?: string;
  overdue?: boolean;
  dueSoon?: boolean;
};
function filterParams(filters: TaskFilters) {
  const params = new URLSearchParams();
  if (filters.statusId) params.set('status_id', filters.statusId);
  if (filters.parentTaskId) params.set('parent_task_id', filters.parentTaskId);
  if (filters.taskType) params.set('task_type', filters.taskType);
  if (filters.keyword) params.set('keyword', filters.keyword);
  if (filters.assigneeId) params.set('assignee_id', filters.assigneeId);
  if (filters.unassigned) params.set('unassigned', 'true');
  if (filters.priority) params.set('priority', filters.priority);
  if (filters.milestoneId) params.set('milestone_id', filters.milestoneId);
  if (filters.labelId) params.set('label_id', filters.labelId);
  if (filters.overdue) params.set('overdue', 'true');
  if (filters.dueSoon) params.set('due_soon', 'true');
  return params;
}
export function listTasks(projectKey: string, page = 1, pageSize = 20, filters: TaskFilters = {}) {
  const params = filterParams(filters);
  params.set('page', String(page));
  params.set('page_size', String(pageSize));
  return apiGet<TaskListResponse>(`/projects/${key(projectKey)}/tasks?${params}`);
}
export function getTask(taskKey: string) { return apiGet<TaskView>(`/tasks/${key(taskKey)}`); }
export function getSubtasks(taskKey: string) { return apiGet<TaskView[]>(`/tasks/${key(taskKey)}/subtasks`); }
export function createTask(projectKey: string, input: { title: string; description?: string; priority?: string; task_type?: string; status_id?: string; assignee_id?: string | null; reviewer_id?: string | null; start_at?: string | null; due_at?: string | null; milestone_id?: string | null }) { return apiPost<TaskView>(`/projects/${key(projectKey)}/tasks`, input); }
export function createSubtask(taskKey: string, input: { title: string; description?: string; priority?: string; task_type?: string; status_id?: string; assignee_id?: string | null; reviewer_id?: string | null; start_at?: string | null; due_at?: string | null; milestone_id?: string | null }) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/subtasks`, input); }
export function updateTask(taskKey: string, input: { title?: string; description?: string; priority?: string; task_type?: string; assignee_id?: string | null; reviewer_id?: string | null; start_at?: string | null; due_at?: string | null; parent_task_id?: string | null; milestone_id?: string | null }) { return apiPatch<TaskView>(`/tasks/${key(taskKey)}`, input); }
export function transitionTask(taskKey: string, statusId: string) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/transition`, { status_id: statusId }); }
export function moveTask(taskKey: string, statusId: string, position: number) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/move`, { status_id: statusId, position }); }
export function deleteTask(taskKey: string, reason?: string) { return apiPost<{ message: string }>(`/tasks/${key(taskKey)}/delete`, reason ? { reason } : {}); }
export function listComments(taskKey: string) { return apiGet<Comment[]>(`/tasks/${key(taskKey)}/comments`); }
export function createComment(taskKey: string, body: string, attachmentIds: string[] = []) { return apiPost<Comment>(`/tasks/${key(taskKey)}/comments`, { body, attachment_ids: attachmentIds.length ? attachmentIds : undefined }); }
export function deleteComment(commentId: string, reason?: string) { return apiPost<{ message: string }>(`/comments/${key(commentId)}/delete`, reason ? { reason } : {}); }

export type MyTaskScope = 'assignee' | 'reporter' | 'reviewer' | 'all';
export function listMyTasks(scope: MyTaskScope, page = 1, pageSize = 30, options: { keyword?: string; overdue?: boolean; dueSoon?: boolean } = {}) {
  const params = new URLSearchParams({ scope, page: String(page), page_size: String(pageSize) });
  if (options.keyword) params.set('keyword', options.keyword);
  if (options.overdue) params.set('overdue', 'true');
  if (options.dueSoon) params.set('due_soon', 'true');
  return apiGet<CrossProjectTaskListResponse>(`/tasks?${params}`);
}

export function listLabels(projectKey: string) { return apiGet<LabelView[]>(`/projects/${key(projectKey)}/labels`); }
export function addTaskLabel(taskKey: string, name: string) { return apiPost<LabelView>(`/tasks/${key(taskKey)}/labels`, { name }); }
export function removeTaskLabel(taskKey: string, labelId: string) { return apiPost<{ message: string }>(`/tasks/${key(taskKey)}/labels/${key(labelId)}/delete`, {}); }

export function listDependencies(taskKey: string) { return apiGet<TaskDependencies>(`/tasks/${key(taskKey)}/dependencies`); }
export function addDependency(taskKey: string, dependsOnTaskKey: string) { return apiPost<TaskDependencies>(`/tasks/${key(taskKey)}/dependencies`, { depends_on_task_key: dependsOnTaskKey }); }
export function removeDependency(taskKey: string, dependencyId: string) { return apiPost<TaskDependencies>(`/tasks/${key(taskKey)}/dependencies/${key(dependencyId)}/delete`, {}); }
export function listProjectDependencies(projectKey: string) { return apiGet<ProjectDependencyListResponse>(`/projects/${key(projectKey)}/task-dependencies`); }

export function restoreTask(taskKey: string) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/restore`, {}); }
export function copyTask(taskKey: string) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/copy`, {}); }
export function listDeletedTasks(projectKey: string) { return apiGet<DeletedTaskListResponse>(`/projects/${key(projectKey)}/tasks/deleted`); }
export function downloadTaskExport(projectKey: string, filters: TaskFilters = {}) {
  const params = filterParams(filters);
  const query = params.toString();
  return apiDownload(`/projects/${key(projectKey)}/tasks/export${query ? `?${query}` : ''}`);
}
