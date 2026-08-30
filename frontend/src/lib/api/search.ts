import { apiGet } from './client';
import type { SearchResult } from './types';
export function searchAll(query: string) { return apiGet<SearchResult>(`/search?q=${encodeURIComponent(query)}`); }
