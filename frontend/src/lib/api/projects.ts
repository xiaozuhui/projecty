import { apiGet, apiPost } from './client';
import type { DepartmentListResponse, ProjectListResponse, ProjectStatus, ProjectView } from './types';

export function listProjects(page = 1, pageSize = 12) {
  return apiGet<ProjectListResponse>(`/projects?page=${page}&page_size=${pageSize}`);
}
export function getProject(projectKey: string) { return apiGet<ProjectView>(`/projects/${encodeURIComponent(projectKey)}`); }
export function createProject(input: { project_key: string; name: string; description?: string; primary_department_id?: string }) {
  return apiPost<ProjectView>('/projects', input);
}
export function listStatuses(projectKey: string) { return apiGet<ProjectStatus[]>(`/projects/${encodeURIComponent(projectKey)}/statuses`); }
export function listDepartments() { return apiGet<DepartmentListResponse>('/departments'); }
