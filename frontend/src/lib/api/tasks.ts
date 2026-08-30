import { apiGet, apiPatch, apiPost } from './client';
import type { TaskListResponse, TaskView } from './types';

export function listTasks(projectKey: string, page = 1, pageSize = 20) {
  return apiGet<TaskListResponse>(`/projects/${encodeURIComponent(projectKey)}/tasks?page=${page}&page_size=${pageSize}`);
}
export function getTask(taskKey: string) { return apiGet<TaskView>(`/tasks/${encodeURIComponent(taskKey)}`); }
export function getSubtasks(taskKey: string) { return apiGet<TaskView[]>(`/tasks/${encodeURIComponent(taskKey)}/subtasks`); }
export function createTask(projectKey: string, input: { title: string; description?: string; priority?: string; status_id?: string }) {
  return apiPost<TaskView>(`/projects/${encodeURIComponent(projectKey)}/tasks`, input);
}
export function createSubtask(taskKey: string, input: { title: string; description?: string; priority?: string; status_id?: string }) {
  return apiPost<TaskView>(`/tasks/${encodeURIComponent(taskKey)}/subtasks`, input);
}
export function updateTask(taskKey: string, input: { title?: string; description?: string; priority?: string }) {
  return apiPatch<TaskView>(`/tasks/${encodeURIComponent(taskKey)}`, input);
}
export function transitionTask(taskKey: string, statusId: string) {
  return apiPost<TaskView>(`/tasks/${encodeURIComponent(taskKey)}/transition`, { status_id: statusId });
}
export function deleteTask(taskKey: string, reason?: string) {
  return apiPost<{ message: string }>(`/tasks/${encodeURIComponent(taskKey)}/delete`, reason ? { reason } : {});
}
