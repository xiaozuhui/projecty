import { apiDownload, apiGet, apiPatch, apiPost, apiUpload } from './client';
import type { UserImportReport, UserListResponse, UserView } from './types';

const key = (value: string) => encodeURIComponent(value);

export type UserListQuery = { search?: string; department_id?: string; include_inactive?: boolean; page?: number; page_size?: number };

export function listUsers(query: UserListQuery = {}) {
  const params = new URLSearchParams();
  if (query.search) params.set('search', query.search);
  if (query.department_id) params.set('department_id', query.department_id);
  if (query.include_inactive) params.set('include_inactive', 'true');
  if (query.page) params.set('page', String(query.page));
  if (query.page_size) params.set('page_size', String(query.page_size));
  const qs = params.toString();
  return apiGet<UserListResponse>(`/users${qs ? `?${qs}` : ''}`);
}

export function createUser(input: { account: string; password: string; display_name: string; system_role?: 'user' | 'super_admin'; department_ids?: string[] }) {
  return apiPost<UserView>('/users', input);
}

export function updateUser(id: string, input: { display_name?: string; is_active?: boolean; password?: string; department_ids?: string[] }) {
  return apiPatch<UserView>(`/users/${key(id)}`, input);
}

export function importUsers(file: File) {
  const form = new FormData();
  form.append('file', file);
  return apiUpload<UserImportReport>('/users/import', form);
}

export function downloadUserTemplate() {
  return apiDownload('/users/import-template');
}
