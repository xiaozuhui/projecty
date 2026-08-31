import { apiGet, apiPatch, apiPost } from './client';
import type { Comment, CrossProjectTaskListResponse, TaskListResponse, TaskView } from './types';
const key = (value: string) => encodeURIComponent(value);
export function listTasks(projectKey: string, page = 1, pageSize = 20, filters: { statusId?: string; parentTaskId?: string } = {}) { const params = new URLSearchParams({ page: String(page), page_size: String(pageSize) }); if (filters.statusId) params.set('status_id', filters.statusId); if (filters.parentTaskId) params.set('parent_task_id', filters.parentTaskId); return apiGet<TaskListResponse>(`/projects/${key(projectKey)}/tasks?${params}`); }
export function getTask(taskKey: string) { return apiGet<TaskView>(`/tasks/${key(taskKey)}`); }
export function getSubtasks(taskKey: string) { return apiGet<TaskView[]>(`/tasks/${key(taskKey)}/subtasks`); }
export function createTask(projectKey: string, input: { title: string; description?: string; priority?: string; status_id?: string; assignee_id?: string | null; due_at?: string | null }) { return apiPost<TaskView>(`/projects/${key(projectKey)}/tasks`, input); }
export function createSubtask(taskKey: string, input: { title: string; description?: string; priority?: string; status_id?: string; assignee_id?: string | null; due_at?: string | null }) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/subtasks`, input); }
export function updateTask(taskKey: string, input: { title?: string; description?: string; priority?: string; assignee_id?: string | null; due_at?: string | null }) { return apiPatch<TaskView>(`/tasks/${key(taskKey)}`, input); }
export function transitionTask(taskKey: string, statusId: string) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/transition`, { status_id: statusId }); }
export function moveTask(taskKey: string, statusId: string, position: number) { return apiPost<TaskView>(`/tasks/${key(taskKey)}/move`, { status_id: statusId, position }); }
export function deleteTask(taskKey: string, reason?: string) { return apiPost<{ message: string }>(`/tasks/${key(taskKey)}/delete`, reason ? { reason } : {}); }
export function listComments(taskKey: string) { return apiGet<Comment[]>(`/tasks/${key(taskKey)}/comments`); }
export function createComment(taskKey: string, body: string, attachmentIds: string[] = []) { return apiPost<Comment>(`/tasks/${key(taskKey)}/comments`, { body, attachment_ids: attachmentIds.length ? attachmentIds : undefined }); }
export function deleteComment(commentId: string, reason?: string) { return apiPost<{ message: string }>(`/comments/${key(commentId)}/delete`, reason ? { reason } : {}); }

export function listMyTasks(scope: 'assignee' | 'reporter' | 'all', page = 1, pageSize = 30) { const params = new URLSearchParams({ scope, page: String(page), page_size: String(pageSize) }); return apiGet<CrossProjectTaskListResponse>(`/tasks?${params}`); }
