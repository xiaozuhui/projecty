import { API_BASE_URL, apiGet, apiPost, apiUpload } from './client';
import type { Attachment } from './types';

const key = (value: string) => encodeURIComponent(value);

export function listTaskAttachments(taskKey: string) {
  return apiGet<Attachment[]>(`/tasks/${key(taskKey)}/attachments`);
}

export function uploadTaskAttachment(taskKey: string, file: File) {
  const form = new FormData();
  form.append('file', file);
  return apiUpload<Attachment>(`/tasks/${key(taskKey)}/attachments`, form);
}

export function deleteAttachment(attachmentId: string, reason?: string) {
  return apiPost<{ message: string }>(`/attachments/${key(attachmentId)}/delete`, reason ? { reason } : {});
}

/// 后端返回相对路径 /attachments/{object_key}/content,拼上 API 前缀供 <img> 直接使用。
export function attachmentUrl(path: string) {
  return `${API_BASE_URL}${path}`;
}
