<script lang="ts">
  import type { ProjectStatus, TaskView } from '$lib/api/types';
  import BoardCard from './BoardCard.svelte';

  interface Props {
    status: ProjectStatus;
    tasks: TaskView[];
    /** 拖拽悬停的列与插入下标,null 表示无拖拽。 */
    dropTarget: { statusId: string; index: number } | null;
    draggingId: string | null;
    ondragcardstart: (event: DragEvent, task: TaskView) => void;
    ondragcardend: () => void;
    onover: (statusId: string, index: number) => void;
    onleave: (statusId: string) => void;
    ondrop: (statusId: string, index: number) => void;
    onquickadd: (statusId: string, title: string) => Promise<boolean>;
    /** 由父任务 id 查父任务 Key,子任务卡片据此展示归属。 */
    parentKeyOf: (task: TaskView) => string | null;
    /** 卡片是否可拖:按当前用户是否负责人/评审人/豁免角色判断。 */
    candrag: (task: TaskView) => boolean;
    /** 该列是否允许快捷新建:完成列仅项目管理员。 */
    canquickadd: boolean;
  }

  let {
    status,
    tasks,
    dropTarget,
    draggingId,
    ondragcardstart,
    ondragcardend,
    onover,
    onleave,
    ondrop,
    onquickadd,
    parentKeyOf,
    candrag,
    canquickadd
  }: Props = $props();

  let body = $state<HTMLDivElement | null>(null);
  let adding = $state(false);
  let newTitle = $state('');
  let submitting = $state(false);

  const active = $derived(dropTarget?.statusId === status.id);
  const indicatorIndex = $derived(dropTarget?.statusId === status.id ? dropTarget.index : -1);

  // 插入下标按卡片中点划分:指针在卡片上半区则插到其前,越过最后一张则落尾。
  function insertionIndex(event: DragEvent): number {
    const cards = Array.from(body?.querySelectorAll<HTMLElement>('[data-card-id]') ?? []);
    for (let index = 0; index < cards.length; index += 1) {
      const rect = cards[index].getBoundingClientRect();
      if (event.clientY < rect.top + rect.height / 2) return index;
    }
    return cards.length;
  }

  function dragOver(event: DragEvent) {
    if (!draggingId) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    onover(status.id, insertionIndex(event));
  }

  function dragLeave(event: DragEvent) {
    if (!body?.contains(event.relatedTarget as Node)) onleave(status.id);
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    ondrop(status.id, insertionIndex(event));
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const title = newTitle.trim();
    if (!title || submitting) return;
    submitting = true;
    const ok = await onquickadd(status.id, title);
    submitting = false;
    if (ok) newTitle = '';
  }
</script>

<article class="board-column" class:active>
  <header>
    <div>
      <h2>{status.name}</h2>
      <small>{status.category}</small>
    </div>
    <span class="count">{tasks.length}</span>
  </header>
  <div class="column-body" role="list" bind:this={body} ondragover={dragOver} ondragleave={dragLeave} ondrop={drop}>
    {#each tasks as task, index}
      {#if active && index === indicatorIndex}<div class="drop-indicator" aria-hidden="true"></div>{/if}
      <BoardCard
        {task}
        parentKey={parentKeyOf(task)}
        dragging={draggingId === task.id}
        draggable={candrag(task)}
        ondragstart={ondragcardstart}
        ondragend={ondragcardend}
      />
    {/each}
    {#if active && tasks.length === indicatorIndex}<div class="drop-indicator" aria-hidden="true"></div>{/if}
    {#if !tasks.length && !active}<div class="empty-column">拖拽卡片到这里</div>{/if}
  </div>
  <div class="quick-add">
    {#if canquickadd}
      {#if adding}
        <form onsubmit={submit}>
          <input
            bind:value={newTitle}
            placeholder="任务标题,回车创建"
            aria-label={`在 ${status.name} 添加任务`}
            disabled={submitting}
            onkeydown={(event) => { if (event.key === 'Escape') { adding = false; newTitle = ''; } }}
            onblur={() => { if (!newTitle.trim()) adding = false; }}
          />
        </form>
      {:else}
        <button type="button" class="quick-add-toggle" onclick={() => (adding = true)}>+ 添加任务</button>
      {/if}
    {/if}
  </div>
</article>

<style>
  .board-column {
    display: flex;
    flex-direction: column;
    min-height: 420px;
    width: 300px;
    flex-shrink: 0;
    padding: 12px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border-weak);
    border-radius: var(--radius-lg);
    transition: border-color var(--transition-fast);
  }
  .board-column.active { border-color: var(--color-primary); }
  header { display: flex; justify-content: space-between; align-items: start; margin-bottom: 10px; }
  h2 { margin: 0 0 2px; font-size: 14px; font-weight: 500; }
  header small { color: var(--color-text-muted); font-size: 11px; }
  .count { padding: 1px 8px; border-radius: var(--radius-sm); background: var(--color-hover); color: var(--color-text-muted); font-size: 12px; }
  .column-body { display: flex; flex-direction: column; gap: 8px; flex: 1; min-height: 60px; }
  .drop-indicator { height: 2px; border-radius: 1px; background: var(--color-primary); margin: -1px 0; }
  .empty-column { padding: 20px 8px; color: var(--color-text-muted); font-size: 12px; text-align: center; border: 1px dashed var(--color-border-strong); border-radius: var(--radius-md); }
  .quick-add { margin-top: 8px; }
  .quick-add-toggle {
    width: 100%;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-muted);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--transition-fast), background var(--transition-fast);
  }
  .board-column:hover .quick-add-toggle,
  .quick-add-toggle:focus-visible { opacity: 1; }
  .quick-add-toggle:hover { background: var(--color-hover); color: var(--color-text-secondary); }
  .quick-add form { display: block; }
  .quick-add input {
    width: 100%;
    padding: 7px 9px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text);
    font-size: 13px;
  }
  .quick-add input:focus-visible { outline: none; border-color: var(--color-primary); box-shadow: var(--color-focus-ring); }
</style>
