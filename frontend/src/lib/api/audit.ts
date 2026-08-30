import { apiDownload, apiGet } from './client';
import type { AuditListResponse } from './types';
const key = (value: string) => encodeURIComponent(value);
export function listProjectLogs(projectKey: string, page = 1, pageSize = 50) { return apiGet<AuditListResponse>(`/projects/${key(projectKey)}/logs?page=${page}&page_size=${pageSize}`); }
export function listTaskLogs(taskKey: string, page = 1, pageSize = 50) { return apiGet<AuditListResponse>(`/tasks/${key(taskKey)}/logs?page=${page}&page_size=${pageSize}`); }
export function downloadProjectLogs(projectKey: string) { return apiDownload(`/projects/${key(projectKey)}/logs/export`); }
export function downloadTaskLogs(taskKey: string) { return apiDownload(`/tasks/${key(taskKey)}/logs/export`); }
export function downloadAdminLogs() { return apiDownload('/admin/operation-logs/export'); }
