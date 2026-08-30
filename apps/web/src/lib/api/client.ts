import { PUBLIC_API_BASE_URL } from '$env/static/public';

export type ApiEnvelope<T> = {
  data: T;
  meta: { request_id: string };
};

export async function apiGet<T>(path: string, token?: string): Promise<ApiEnvelope<T>> {
  const response = await fetch(`${PUBLIC_API_BASE_URL}${path}`, {
    headers: token ? { Authorization: `Bearer ${token}` } : {}
  });
  if (!response.ok) throw new Error(`API 请求失败：${response.status}`);
  return response.json() as Promise<ApiEnvelope<T>>;
}
