import { env } from '$env/dynamic/public';
import { session, type AuthSession } from '$lib/features/auth/session.svelte';

const API_BASE_URL = (env.PUBLIC_API_BASE_URL || '/api/v1').replace(/\/$/, '');

export type ApiEnvelope<T> = {
  data: T;
  meta: { request_id: string };
};

export class ApiClientError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ApiClientError';
    this.status = status;
    this.code = code;
  }
}

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null;
}

function parseEnvelope<T>(value: unknown): ApiEnvelope<T> {
  if (!isRecord(value) || !('data' in value) || !isRecord(value.meta) || typeof value.meta.request_id !== 'string') {
    throw new ApiClientError(502, 'invalid_response', '服务器返回了无法识别的数据');
  }
  return { data: value.data as T, meta: { request_id: value.meta.request_id } };
}

async function parseError(response: Response): Promise<ApiClientError> {
  const body = (await response.json().catch(() => null)) as unknown;
  if (isRecord(body) && isRecord(body.data)) {
    const code = typeof body.data.code === 'string' ? body.data.code : 'api_error';
    const message = typeof body.data.message === 'string' ? body.data.message : `API 请求失败：${response.status}`;
    return new ApiClientError(response.status, code, message);
  }
  return new ApiClientError(response.status, 'api_error', `API 请求失败：${response.status}`);
}

let refreshPromise: Promise<boolean> | null = null;

async function refreshSession(): Promise<boolean> {
  if (!session.refreshToken) return false;
  if (!refreshPromise) {
    const refreshToken = session.refreshToken;
    refreshPromise = fetch(`${API_BASE_URL}/auth/refresh`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refresh_token: refreshToken })
    })
      .then(async (response) => {
        if (!response.ok) return false;
        const payload = parseEnvelope<AuthSession>(await response.json());
        session.set(payload.data);
        return true;
      })
      .catch(() => false)
      .finally(() => { refreshPromise = null; });
  }
  return refreshPromise;
}

async function request<T>(path: string, init: RequestInit = {}, allowRefresh = true): Promise<ApiEnvelope<T>> {
  const headers = new Headers(init.headers);
  if (init.body && !headers.has('Content-Type')) headers.set('Content-Type', 'application/json');
  const token = headers.get('Authorization') ? null : session.accessToken;
  if (token) headers.set('Authorization', `Bearer ${token}`);

  const response = await fetch(`${API_BASE_URL}${path}`, { ...init, headers });
  if (response.status === 401 && allowRefresh && session.refreshToken && path !== '/auth/refresh') {
    const refreshed = await refreshSession();
    if (refreshed) return request<T>(path, init, false);
    session.clear();
  }
  if (!response.ok) throw await parseError(response);
  return parseEnvelope<T>(await response.json());
}

export function apiGet<T>(path: string, token?: string): Promise<ApiEnvelope<T>> {
  return request<T>(path, { headers: token ? { Authorization: `Bearer ${token}` } : undefined });
}

export function apiPost<T>(path: string, body?: unknown, token?: string): Promise<ApiEnvelope<T>> {
  return request<T>(path, {
    method: 'POST',
    body: body === undefined ? undefined : JSON.stringify(body),
    headers: token ? { Authorization: `Bearer ${token}` } : undefined
  });
}

export function apiPatch<T>(path: string, body: unknown, token?: string): Promise<ApiEnvelope<T>> {
  return request<T>(path, {
    method: 'PATCH',
    body: JSON.stringify(body),
    headers: token ? { Authorization: `Bearer ${token}` } : undefined
  });
}

export async function apiDownload(path: string, token?: string, allowRefresh = true): Promise<Blob> {
  const headers = new Headers(token ? { Authorization: `Bearer ${token}` } : undefined);
  if (!token && session.accessToken) headers.set('Authorization', `Bearer ${session.accessToken}`);
  const response = await fetch(`${API_BASE_URL}${path}`, { headers });
  if (response.status === 401 && allowRefresh && session.refreshToken) {
    const refreshed = await refreshSession();
    if (refreshed) return apiDownload(path, undefined, false);
    session.clear();
  }
  if (!response.ok) throw await parseError(response);
  return response.blob();
}
