<script lang="ts">
  // 标签徽章:无后端颜色字段,按名称哈希从固定色板取色,同名稳定。
  let { name, onremove }: { name: string; onremove?: () => void } = $props();

  const palette = ['#2563eb', '#0f766e', '#b45309', '#9333ea', '#be123c', '#475569'];
  const color = $derived.by(() => {
    let hash = 0;
    for (const char of name) hash = (hash * 31 + char.codePointAt(0)!) >>> 0;
    return palette[hash % palette.length];
  });
</script>

<span class="label-pill" style="--label-color: {color}">
  {name}
  {#if onremove}
    <button type="button" aria-label={`移除标签 ${name}`} onclick={onremove}>×</button>
  {/if}
</span>

<style>
  .label-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 7px;
    border: 1px solid color-mix(in srgb, var(--label-color) 35%, transparent);
    border-radius: 999px;
    background: color-mix(in srgb, var(--label-color) 12%, transparent);
    color: var(--label-color);
    font-size: 12px;
    white-space: nowrap;
    line-height: 1.6;
  }
  .label-pill button {
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: inherit;
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
  }
  .label-pill button:hover { background: color-mix(in srgb, var(--label-color) 20%, transparent); }
</style>
