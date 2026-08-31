<script lang="ts">
  // 优先级徽章:urgent/high/medium/low/none 映射中文与语义色。
  const labels: Record<string, string> = { urgent: '紧急', high: '高', medium: '中', low: '低', none: '无' };

  let { priority }: { priority: string } = $props();

  const key = $derived(priority in labels ? priority : 'none');
</script>

<span class={`priority-pill p-${key}`}>{labels[key]}</span>

<style>
  .priority-pill { display: inline-flex; align-items: center; padding: 1px 7px; border-radius: 999px; font-size: 12px; background: var(--color-hover); color: var(--color-text-muted); white-space: nowrap; }
  .p-urgent { color: var(--priority-urgent); }
  .p-high { color: var(--priority-high); }
  .p-medium { color: var(--color-text-secondary); }
</style>
