import { apiGet, apiPatch, apiPost } from './client';
import type { Milestone, MilestoneListResponse } from './types';
const key = (value: string) => encodeURIComponent(value);
export function listMilestones(projectKey: string) { return apiGet<MilestoneListResponse>(`/projects/${key(projectKey)}/milestones`); }
export function createMilestone(projectKey: string, input: { name: string; due_date?: string | null }) { return apiPost<Milestone>(`/projects/${key(projectKey)}/milestones`, input); }
export function updateMilestone(id: string, input: { name?: string; due_date?: string | null; is_reached?: boolean }) { return apiPatch<Milestone>(`/milestones/${key(id)}`, input); }
export function deleteMilestone(id: string, reason?: string) { return apiPost<{ message: string }>(`/milestones/${key(id)}/delete`, reason ? { reason } : {}); }
