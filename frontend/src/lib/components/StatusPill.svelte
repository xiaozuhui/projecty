<script lang="ts">
  // 状态徽章:按 status.category 选色,显示状态名(不再展示 UUID 片段)。
  import type { ProjectStatus } from '$lib/api/types';

  let { status }: { status: Pick<ProjectStatus, 'name' | 'category'> } = $props();

  const category = $derived(['todo', 'active', 'review', 'done'].includes(status.category) ? status.category : 'todo');
</script>

<span class={`status-pill cat-${category}`}>{status.name}</span>

<style>
  .status-pill { display: inline-flex; align-items: center; gap: 6px; padding: 2px 8px; border-radius: 999px; font-size: 12px; background: var(--color-hover); color: var(--color-text-secondary); white-space: nowrap; }
  .status-pill::before { content: ''; width: 6px; height: 6px; border-radius: 50%; background: currentColor; }
  .cat-todo { color: var(--color-text-muted); }
  .cat-active { color: var(--status-active); }
  .cat-review { color: var(--status-review); }
  .cat-done { color: var(--status-done); }
</style>
