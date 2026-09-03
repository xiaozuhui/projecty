<script lang="ts">
  import { taskTypeLabel } from '$lib/features/task-types';

  let { taskType }: { taskType: string } = $props();
  const label = $derived(taskTypeLabel(taskType));
</script>

<span class={`task-type-pill type-${taskType}`}>{label}</span>

<style>
  .task-type-pill {
    display: inline-flex;
    align-items: center;
    padding: 1px 7px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: var(--color-surface);
    color: var(--color-text-secondary);
    font-size: 12px;
    white-space: nowrap;
  }
  .type-feature { color: var(--color-primary); }
  .type-bug { color: var(--color-danger); }
  .type-design { color: #8b5cf6; }
  .type-revert { color: var(--color-warning); }
  .type-improvement, .type-refactor { color: #0f766e; }
  .type-docs, .type-chore { color: var(--color-text-muted); }
</style>
