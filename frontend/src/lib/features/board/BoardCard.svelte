<script lang="ts">
  import Avatar from '$lib/components/Avatar.svelte';
  import PriorityPill from '$lib/components/PriorityPill.svelte';
  import TaskTypePill from '$lib/components/TaskTypePill.svelte';
  import type { TaskView } from '$lib/api/types';

  interface Props {
    task: TaskView;
    /** 父任务 Key,根任务为 null;子任务卡片用它展示归属。 */
    parentKey: string | null;
    dragging: boolean;
    /** 无流转权限的成员禁止拖动卡片(后端仍兜底校验)。 */
    draggable: boolean;
    ondragstart: (event: DragEvent, task: TaskView) => void;
    ondragend: () => void;
  }

  let { task, parentKey, dragging, draggable, ondragstart, ondragend }: Props = $props();

  // 部分浏览器在拖拽结束后仍会补发一次 click,用标志位吞掉,避免拖完直接跳详情。
  let justDragged = $state(false);

  function start(event: DragEvent) {
    ondragstart(event, task);
  }

  function end() {
    justDragged = true;
    setTimeout(() => (justDragged = false), 0);
    ondragend();
  }

  function click(event: MouseEvent) {
    if (justDragged) {
      event.preventDefault();
      event.stopPropagation();
      justDragged = false;
    }
  }
</script>

<a
  class="board-card"
  class:dragging
  class:readonly-card={!draggable}
  href={`/tasks/${task.task_key}`}
  draggable={draggable}
  data-card-id={task.id}
  ondragstart={start}
  ondragend={end}
  onclick={click}
>
  <span class="card-top">
    <code>{task.task_key}</code>
    <span class="card-pills"><TaskTypePill taskType={task.task_type} /><PriorityPill priority={task.priority} /></span>
  </span>
  {#if parentKey}<span class="parent-chip" title="所属父任务">↳ {parentKey}</span>{/if}
  <strong>{task.title}</strong>
  <span class="card-bottom">
    <small>{task.due_at ? `截止 ${new Date(task.due_at).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })}` : '未设置截止时间'}</small>
    {#if task.assignee_name}
      <span class="assignee"><Avatar name={task.assignee_name} size={18} />{task.assignee_name}</span>
    {/if}
  </span>
</a>

<style>
  .board-card {
    display: grid;
    gap: 8px;
    padding: 10px 12px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    cursor: grab;
    text-decoration: none;
    color: var(--color-text);
    transition: border-color var(--transition-fast), background var(--transition-fast);
  }
  .board-card:hover { border-color: var(--color-border-strong); }
  .board-card:active { cursor: grabbing; }
  .board-card.readonly-card, .board-card.readonly-card:active { cursor: pointer; }
  .board-card.dragging { opacity: 0.4; }
  .card-top { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .card-pills { display: inline-flex; align-items: center; gap: 4px; }
  .card-top code { font-family: var(--font-mono); font-size: 11px; color: var(--color-text-muted); }
  .parent-chip { margin-top: -2px; color: var(--color-text-muted); font-family: var(--font-mono); font-size: 11px; }
  .board-card strong { font-size: 14px; font-weight: 500; line-height: 1.45; }
  .card-bottom { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
  .board-card small { color: var(--color-text-muted); font-size: 12px; }
  .assignee { display: inline-flex; align-items: center; gap: 5px; color: var(--color-text-muted); font-size: 12px; min-width: 0; }
</style>
