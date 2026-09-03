import { apiGet, apiPost } from './client';
import type { NotificationListResponse } from './types';

export function listNotifications(page = 1, pageSize = 30, unreadOnly = false) {
  const params = new URLSearchParams({ page: String(page), page_size: String(pageSize) });
  if (unreadOnly) params.set('unread_only', 'true');
  return apiGet<NotificationListResponse>(`/notifications?${params}`);
}

export function unreadCount() {
  return apiGet<{ count: number }>('/notifications/unread-count');
}

export function markNotificationRead(id: string) {
  return apiPost<{ message: string }>(`/notifications/${encodeURIComponent(id)}/read`, {});
}

export function markAllNotificationsRead() {
  return apiPost<{ message: string; updated: number }>('/notifications/read-all', {});
}
