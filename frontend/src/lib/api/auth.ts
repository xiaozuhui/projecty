import { apiPost, apiGet, apiPatch } from './client';
import type { MeResponse } from './types';
import type { AuthSession } from '$lib/features/auth/session.svelte';

export function login(account: string, password: string) {
  return apiPost<AuthSession>('/auth/login', { account, password });
}
export function refresh(refreshToken: string) {
  return apiPost<AuthSession>('/auth/refresh', { refresh_token: refreshToken });
}
export function logout(refreshToken: string) {
  return apiPost<{ message: string }>('/auth/logout', { refresh_token: refreshToken });
}
export function me(token?: string) {
  return apiGet<MeResponse>('/me', token);
}

export function updateProfile(display_name: string, email: string | null) {
  return apiPatch<MeResponse>('/me', { display_name, email });
}

export function changePassword(current_password: string, new_password: string) { return apiPatch<{ message: string }>('/me/password', { current_password, new_password }); }
