<script lang="ts">
  // 通用模态:遮罩 + surface-raised 卡片,Esc/点遮罩关闭,内容与底部按钮由调用方以 snippet 传入。
  let {
    open = false,
    title = '',
    onClose,
    children,
    footer,
  }: {
    open?: boolean;
    title?: string;
    onClose?: () => void;
    children?: import('svelte').Snippet;
    footer?: import('svelte').Snippet;
  } = $props();
</script>

<svelte:window onkeydown={(event) => { if (open && event.key === 'Escape') onClose?.(); }} />

{#if open}
  <div class="modal-backdrop" role="presentation" onclick={() => onClose?.()} onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ' || event.key === 'Escape') onClose?.(); }}>
    <div class="modal-card" role="dialog" aria-modal="true" aria-label={title} tabindex="-1" onclick={(event) => event.stopPropagation()} onkeydown={(event) => event.stopPropagation()}>
      {#if title}<h2>{title}</h2>{/if}
      <div class="modal-body">{@render children?.()}</div>
      {#if footer}<div class="modal-footer">{@render footer()}</div>{/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; padding: 24px; background: rgba(8, 9, 10, 0.6); }
  .modal-card { width: min(100%, 420px); max-height: 80vh; overflow-y: auto; background: var(--color-surface-raised); border: 1px solid var(--color-border); border-radius: var(--radius-lg); box-shadow: var(--shadow-popover); padding: 20px; }
  .modal-card h2 { margin: 0 0 12px; font-size: 15px; font-weight: 500; color: var(--color-text); }
  .modal-body { color: var(--color-text-secondary); font-size: 13px; line-height: 1.6; }
  .modal-footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
</style>
