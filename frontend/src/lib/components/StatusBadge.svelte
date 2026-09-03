<script lang="ts">
  // 状态胶囊:按类别着色(待办灰/进行中蓝/完成绿),未知类别回落为灰。
  let { name, category }: { name: string; category?: string | null } = $props();

  const key = $derived(category === 'in_progress' || category === 'done' ? category : 'todo');
</script>

<span class={`status-badge cat-${key}`}><i class="dot"></i>{name}</span>

<style>
  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--color-hover);
    color: var(--color-text-secondary);
    font-size: 12px;
    white-space: nowrap;
  }
  .status-badge .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--color-text-muted); }
  .status-badge.cat-in_progress { background: var(--color-primary-soft); color: var(--color-primary-strong); }
  .status-badge.cat-in_progress .dot { background: var(--color-primary); }
  .status-badge.cat-done { background: color-mix(in srgb, var(--color-success) 12%, transparent); color: var(--color-success); }
  .status-badge.cat-done .dot { background: var(--color-success); }
</style>
