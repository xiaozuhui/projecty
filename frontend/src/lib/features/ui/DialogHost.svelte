<script lang="ts">
  // 对话框宿主:挂在 (app) 布局,把 dialog store 的当前状态渲染成 Modal。
  import Modal from '$lib/components/Modal.svelte';
  import { dialog } from '$lib/features/ui/dialog.svelte';

  let promptValue = $state('');
  let inputElement = $state<HTMLInputElement | null>(null);

  const current = $derived(dialog.current);

  const confirmButton = $derived.by(() => {
    if (current.kind === 'closed') return { danger: false, label: '确定' };
    return { danger: current.danger, label: current.confirmLabel ?? (current.kind === 'alert' ? '知道了' : '确定') };
  });

  $effect(() => {
    if (current.kind === 'prompt') {
      promptValue = current.initial ?? '';
      queueMicrotask(() => inputElement?.focus());
    }
  });

  function dismiss() {
    if (current.kind === 'confirm') current.resolve(false);
    else if (current.kind === 'prompt') current.resolve(null);
    else if (current.kind === 'alert') current.resolve();
    dialog.current = { kind: 'closed' };
  }

  function accept() {
    if (current.kind === 'confirm') current.resolve(true);
    else if (current.kind === 'prompt') current.resolve(promptValue);
    else if (current.kind === 'alert') current.resolve();
    dialog.current = { kind: 'closed' };
  }
</script>

<Modal open={current.kind !== 'closed'} title={current.kind === 'closed' ? '' : current.title} onClose={dismiss}>
  {#if current.kind === 'confirm' || current.kind === 'alert'}
    {#if current.message}<p class="dialog-message">{current.message}</p>{/if}
  {:else if current.kind === 'prompt'}
    {#if current.message}<p class="dialog-message">{current.message}</p>{/if}
    {#if current.label}<label class="dialog-label" for="dialog-input">{current.label}</label>{/if}
    <input
      id="dialog-input"
      class="dialog-input"
      bind:this={inputElement}
      bind:value={promptValue}
      placeholder={current.placeholder ?? ''}
      onkeydown={(event) => { if (event.key === 'Enter') accept(); }}
    />
  {/if}
  {#snippet footer()}
    {#if current.kind === 'confirm' || current.kind === 'prompt'}
      <button class="secondary-button" type="button" onclick={dismiss}>取消</button>
    {/if}
    <button class={confirmButton.danger ? 'danger-button' : 'primary-button'} type="button" onclick={accept}>
      {confirmButton.label}
    </button>
  {/snippet}
</Modal>

<style>
  .dialog-message { margin: 0; white-space: pre-wrap; }
  .dialog-label { display: block; margin: 12px 0 6px; color: var(--color-text-secondary); font-size: 13px; font-weight: 500; }
  .dialog-input { width: 100%; }
</style>
