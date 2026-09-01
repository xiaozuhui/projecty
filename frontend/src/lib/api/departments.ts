import { apiGet, apiPatch, apiPost } from './client';
import type { DepartmentListResponse, DepartmentMemberListResponse, DepartmentView, ProjectListResponse } from './types';
const key = (value: string) => encodeURIComponent(value);
export function listDepartments(includeDeleted = false) { return apiGet<DepartmentListResponse>(`/departments?include_deleted=${includeDeleted}`); }
export function createDepartment(input: { parent_id?: string | null; name: string; code: string; sort_order?: number }) { return apiPost<DepartmentView>('/departments', input); }
export function updateDepartment(id: string, input: { name?: string; code?: string; sort_order?: number }) { return apiPatch<DepartmentView>(`/departments/${key(id)}`, input); }
export function deleteDepartment(id: string, reason?: string) { return apiPost<{ message: string }>(`/departments/${key(id)}/delete`, reason ? { reason } : {}); }
export function listDepartmentProjects(id: string) { return apiGet<{ department_id: string; items: ProjectListResponse['items'] }>(`/departments/${key(id)}/projects`); }
export function listDepartmentMembers(id: string) { return apiGet<DepartmentMemberListResponse>(`/departments/${key(id)}/users`); }
