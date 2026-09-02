import { apiGet, apiPatch, apiPost } from './client';
import type { DepartmentListResponse, MemberCandidatesResponse, ProjectDepartmentGrantListResponse, ProjectListResponse, ProjectMemberListResponse, ProjectStatus, ProjectView } from './types';

const key = (value: string) => encodeURIComponent(value);
export function listProjects(page = 1, pageSize = 12) { return apiGet<ProjectListResponse>(`/projects?page=${page}&page_size=${pageSize}`); }
export function getProject(projectKey: string) { return apiGet<ProjectView>(`/projects/${key(projectKey)}`); }
export function createProject(input: { project_key: string; name: string; description?: string; primary_department_id?: string }) { return apiPost<ProjectView>('/projects', input); }
export function updateProject(projectKey: string, input: { name?: string; description?: string; primary_department_id?: string | null }) { return apiPatch<ProjectView>(`/projects/${key(projectKey)}`, input); }
export function archiveProject(projectKey: string) { return apiPost<ProjectView>(`/projects/${key(projectKey)}/archive`); }
export function restoreProject(projectKey: string) { return apiPost<ProjectView>(`/projects/${key(projectKey)}/restore`); }
export function deleteProject(projectKey: string, reason?: string) { return apiPost<{ message: string }>(`/projects/${key(projectKey)}/delete`, reason ? { reason } : {}); }
export function listStatuses(projectKey: string) { return apiGet<ProjectStatus[]>(`/projects/${key(projectKey)}/statuses`); }
export function reorderStatuses(projectKey: string, statusIds: string[]) { return apiPatch<ProjectStatus[]>(`/projects/${key(projectKey)}/statuses/order`, { status_ids: statusIds }); }
export function listProjectMembers(projectKey: string) { return apiGet<ProjectMemberListResponse>(`/projects/${key(projectKey)}/members`); }
export function listMemberCandidates(projectKey: string, search: string) { return apiGet<MemberCandidatesResponse>(`/projects/${key(projectKey)}/member-candidates?search=${encodeURIComponent(search)}`); }
export function addProjectMember(projectKey: string, input: { user_id: string; role: string }) { return apiPost<ProjectMemberListResponse>(`/projects/${key(projectKey)}/members`, input); }
export function updateProjectMember(projectKey: string, userId: string, role: string) { return apiPatch<ProjectMemberListResponse>(`/projects/${key(projectKey)}/members/${key(userId)}`, { role }); }
export function revokeProjectMember(projectKey: string, userId: string) { return apiPost<ProjectMemberListResponse>(`/projects/${key(projectKey)}/members/${key(userId)}/revoke`); }
export function listDepartmentGrants(projectKey: string) { return apiGet<ProjectDepartmentGrantListResponse>(`/projects/${key(projectKey)}/department-grants`); }
export function grantDepartment(projectKey: string, departmentId: string, role: 'member' | 'viewer') { return apiPost<ProjectDepartmentGrantListResponse>(`/projects/${key(projectKey)}/department-grants`, { department_id: departmentId, role }); }
export function revokeDepartmentGrant(projectKey: string, departmentId: string) { return apiPost<ProjectDepartmentGrantListResponse>(`/projects/${key(projectKey)}/department-grants/${key(departmentId)}/revoke`); }
export function listDepartments() { return apiGet<DepartmentListResponse>('/departments'); }
