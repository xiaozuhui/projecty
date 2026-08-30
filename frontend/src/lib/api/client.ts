import { env } from '$env/dynamic/public';

const API_BASE_URL = env.PUBLIC_API_BASE_URL || '/api/v1';

export type ApiEnvelope<T> = {
  data: T;
  meta: { request_id: string };
};

type ApiError = {
  data?: { code?: string; message?: string };
};

async function request<T>(path: string, init: RequestInit = {}): Promise<ApiEnvelope<T>> {
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init.headers ?? {})
    }
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as ApiError | null;
    throw new Error(body?.data?.message || `API 请求失败：${response.status}`);
  }
  return response.json() as Promise<ApiEnvelope<T>>;
}

export function apiGet<T>(path: string, token?: string): Promise<ApiEnvelope<T>> {
  return request<T>(path, {
    headers: token ? { Authorization: `Bearer ${token}` } : {}
  });
}

export function apiPost<T>(path: string, body: unknown, token?: string): Promise<ApiEnvelope<T>> {
  return request<T>(path, {
    method: 'POST',
    body: JSON.stringify(body),
    headers: token ? { Authorization: `Bearer ${token}` } : {}
  });
}
