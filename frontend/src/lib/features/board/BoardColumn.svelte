<script lang="ts">
  import type { ProjectStatus, TaskView } from '$lib/api/types';
  import BoardCard from './BoardCard.svelte';

  /** 渲染行:泳道标题或卡片(卡片带 data-card-id,供插入指示与落点换算)。 */
  type Row =
    | { kind: 'lane'; name: string; count: number }
    | { kind: 'task'; task: TaskView };

  interface Props {
    status: ProjectStatus;
    tasks: TaskView[];
    /** 拖拽悬停的列与「插到哪张卡之前」(null=列尾),外层 null 表示无拖拽。 */
    dropTarget: { statusId: string; beforeTaskId: string | null } | null;
    draggingId: string | null;
    ondragcardstart: (event: DragEvent, task: TaskView) => void;
    ondragcardend: () => void;
    onover: (statusId: string, beforeTaskId: string | null) => void;
    onleave: (statusId: string) => void;
    ondrop: (statusId: string, beforeTaskId: string | null) => void;
    onquickadd: (statusId: string, title: string) => Promise<boolean>;
    /** 由父任务 id 查父任务 Key,子任务卡片据此展示归属。 */
    parentKeyOf: (task: TaskView) => string | null;
    /** 卡片是否可拖:按当前用户是否负责人/评审人/豁免角色判断。 */
    candrag: (task: TaskView) => boolean;
    /** 该列是否允许快捷新建:完成列仅项目管理员。 */
    canquickadd: boolean;
    /** 泳道分组函数,null 表示不分组。 */
    groupOf?: ((task: TaskView) => string) | null;
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
    canquickadd,
    groupOf = null
  }: Props = $props();

  let body = $state<HTMLDivElement | null>(null);
  let adding = $state(false);
  let newTitle = $state('');
  let submitting = $state(false);

  const active = $derived(dropTarget?.statusId === status.id);
  const indicatorBefore = $derived(active ? dropTarget!.beforeTaskId : undefined);

  // 分组时按组首次出现顺序排列,组内保持原顺序;平铺为渲染行。
  const rows = $derived.by(() => {
    if (!groupOf) return tasks.map((task) => ({ kind: 'task', task }) as Row);
    const order: string[] = [];
    const buckets = new Map<string, TaskView[]>();
    for (const task of tasks) {
      const name = groupOf(task);
      if (!buckets.has(name)) {
        buckets.set(name, []);
        order.push(name);
      }
      buckets.get(name)!.push(task);
    }
    const result: Row[] = [];
    for (const name of order) {
      const bucket = buckets.get(name)!;
      result.push({ kind: 'lane', name, count: bucket.length });
      for (const task of bucket) result.push({ kind: 'task', task });
    }
    return result;
  });

  // 落点按卡片中点划分:指针在卡片上半区则插到其前,越过最后一张则落列尾(null)。
  function insertionBefore(event: DragEvent): string | null {
    const cards = Array.from(body?.querySelectorAll<HTMLElement>('[data-card-id]') ?? []);
    for (const card of cards) {
      const rect = card.getBoundingClientRect();
      if (event.clientY < rect.top + rect.height / 2) return card.dataset.cardId ?? null;
    }
    return null;
  }

  function dragOver(event: DragEvent) {
    if (!draggingId) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move';
    onover(status.id, insertionBefore(event));
  }

  function dragLeave(event: DragEvent) {
    if (!body?.contains(event.relatedTarget as Node)) onleave(status.id);
  }

  function drop(event: DragEvent) {
    event.preventDefault();
    ondrop(status.id, insertionBefore(event));
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
    <i class="cat-dot cat-{status.category === 'in_progress' || status.category === 'done' ? status.category : 'todo'}"></i>
    <h2>{status.name}</h2>
    <span class="count">{tasks.length}</span>
  </header>
  <div class="column-body" role="list" bind:this={body} ondragover={dragOver} ondragleave={dragLeave} ondrop={drop}>
    {#if active && indicatorBefore === null}<div class="drop-indicator" aria-hidden="true"></div>{/if}
    {#each rows as row}
      {#if row.kind === 'lane'}
        <div class="lane-head"><span>{row.name}</span><small>{row.count}</small></div>
      {:else}
        {#if active && row.task.id === indicatorBefore}<div class="drop-indicator" aria-hidden="true"></div>{/if}
        <BoardCard
          task={row.task}
          parentKey={parentKeyOf(row.task)}
          dragging={draggingId === row.task.id}
          draggable={candrag(row.task)}
          ondragstart={ondragcardstart}
          ondragend={ondragcardend}
        />
      {/if}
    {/each}
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
        <button type="button" class="quick-add-toggle" onclick={() => (adding = true)}>＋ 添加任务</button>
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
  header { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; padding: 2px 4px; }
  .cat-dot { width: 8px; height: 8px; border-radius: 50%; flex: none; background: var(--color-text-muted); }
  .cat-dot.cat-in_progress { background: var(--color-primary); }
  .cat-dot.cat-done { background: var(--color-success); }
  h2 { margin: 0; flex: 1; min-width: 0; font-size: 13px; font-weight: 600; color: var(--color-text-secondary); }
  .count { padding: 1px 8px; border-radius: 999px; background: var(--color-surface-raised); color: var(--color-text-muted); font-size: 11px; font-family: var(--font-mono); }
  .column-body { display: flex; flex-direction: column; gap: 8px; flex: 1; min-height: 60px; }
  .lane-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 6px;
    border-radius: var(--radius-sm);
    background: var(--color-hover);
    color: var(--color-text-secondary);
    font-size: 12px;
    font-weight: 500;
  }
  .lane-head small { color: var(--color-text-muted); font-size: 11px; }
  .drop-indicator { height: 2px; border-radius: 1px; background: var(--color-primary); margin: -1px 0; }
  .empty-column { padding: 20px 8px; color: var(--color-text-muted); font-size: 12px; text-align: center; border: 1px dashed var(--color-border-strong); border-radius: var(--radius-md); }
  .quick-add { margin-top: 8px; }
  .quick-add-toggle {
    width: 100%;
    padding: 6px 8px;
    border: 1px dashed transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-muted);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
    opacity: 0;
    transition: opacity var(--transition-fast), background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
  }
  .board-column:hover .quick-add-toggle,
  .quick-add-toggle:focus-visible { opacity: 1; }
  .quick-add-toggle:hover { color: var(--color-primary-strong); border-color: var(--color-primary); }
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
