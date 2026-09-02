<script lang="ts">
  // 部门下拉选择:单选保留原生 select(带 optgroup 分组);
  // 多选为触发框 + 复选下拉面板,点击即选/取消,无需按住修饰键。
  // 面板 position: fixed 定位,可在 Modal 内使用而不被卡片 overflow 裁剪。
  import type { DepartmentView } from '$lib/api/types';

  interface Props {
    departments: DepartmentView[];
    value: string | string[];
    multiple?: boolean;
    placeholder?: string;
    ariaLabel?: string;
    onchange: (value: string | string[]) => void;
  }

  let { departments, value, multiple = false, placeholder, ariaLabel = '部门选择', onchange }: Props = $props();

  let trigger = $state<HTMLButtonElement | null>(null);
  let panelNode = $state<HTMLElement | null>(null);
  let open = $state(false);
  let panel = $state<{ top: string; bottom: string; left: string; width: string; maxHeight: string } | null>(null);

  // 平铺列表 → 以顶级部门为根的分组树;父部门被软删/不可见的孤儿按根展示,避免选不到。
  const groups = $derived.by(() => {
    const byParent = new Map<string | null, DepartmentView[]>();
    for (const department of departments) {
      const list = byParent.get(department.parent_id) ?? [];
      list.push(department);
      byParent.set(department.parent_id, list);
    }
    const exists = new Set(departments.map((department) => department.id));
    const roots = [
      ...(byParent.get(null) ?? []),
      ...departments.filter((department) => department.parent_id && !exists.has(department.parent_id))
    ];
    const rowsOf = (parent: string, depth: number): { department: DepartmentView; depth: number }[] =>
      (byParent.get(parent) ?? []).flatMap((department) => [
        { department, depth },
        ...rowsOf(department.id, depth + 1)
      ]);
    return roots.map((root) => ({ root, rows: [{ department: root, depth: 0 }, ...rowsOf(root.id, 1)] }));
  });

  const rows = $derived(groups.flatMap((group) => group.rows));
  const selectedIds = $derived((Array.isArray(value) ? value : value ? [value] : []).filter((id) => rows.some((row) => row.department.id === id)));
  const summary = $derived(selectedIds.map((id) => rows.find((row) => row.department.id === id)?.department.name ?? '').filter(Boolean).join('、'));

  const isSelected = (id: string) => (Array.isArray(value) ? value.includes(id) : value === id);

  function change(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    onchange(select.value);
  }

  function togglePanel() {
    if (open) {
      open = false;
      return;
    }
    const rect = trigger?.getBoundingClientRect();
    if (!rect) return;
    const spaceBelow = window.innerHeight - rect.bottom;
    const upward = spaceBelow < 240 && rect.top > spaceBelow;
    panel = {
      top: upward ? '' : `${Math.round(rect.bottom + 6)}px`,
      bottom: upward ? `${Math.round(window.innerHeight - rect.top + 6)}px` : '',
      left: `${Math.round(rect.left)}px`,
      width: `${Math.max(Math.round(rect.width), 220)}px`,
      maxHeight: `${Math.min(upward ? rect.top - 12 : spaceBelow - 12, 260)}px`
    };
    open = true;
  }

  function toggle(id: string) {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onchange([...next]);
  }

  function onWindowPointerDown(event: MouseEvent) {
    if (!open) return;
    const target = event.target as Node;
    if (trigger?.contains(target) || panelNode?.contains(target)) return;
    open = false;
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if (open && event.key === 'Escape') open = false;
  }

  // 滚动时关闭面板(fixed 定位不会跟随滚动,干脆收起)
  $effect(() => {
    if (!open) return;
    const close = () => (open = false);
    window.addEventListener('scroll', close, true);
    window.addEventListener('resize', close);
    return () => {
      window.removeEventListener('scroll', close, true);
      window.removeEventListener('resize', close);
    };
  });
</script>

<svelte:window onpointerdown={onWindowPointerDown} onkeydown={onWindowKeydown} />

{#if multiple}
  <button
    type="button"
    class="dept-trigger"
    bind:this={trigger}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={ariaLabel}
    onclick={togglePanel}
  >
    <span class="dept-trigger-text" class:empty={!summary}>{summary || placeholder || '选择部门'}</span>
    <span class="chevron" class:open aria-hidden="true"></span>
  </button>
  {#if open && panel}
    <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
    <div class="dept-panel" bind:this={panelNode} role="listbox" aria-multiselectable="true" aria-label={ariaLabel} style={`top:${panel.top};bottom:${panel.bottom};left:${panel.left};width:${panel.width};max-height:${panel.maxHeight}`}>
      {#if !rows.length}
        <div class="dept-empty">还没有部门</div>
      {:else}
        {#each rows as row (row.department.id)}
          <button type="button" class="dept-option" role="option" aria-selected={selectedIds.includes(row.department.id)} style={`padding-left:${10 + row.depth * 20}px`} onclick={() => toggle(row.department.id)}>
            <span class="checkbox" class:checked={selectedIds.includes(row.department.id)} aria-hidden="true">{#if selectedIds.includes(row.department.id)}✓{/if}</span>
            <span class="dept-option-name">{row.department.name}</span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
{:else}
  <select class="dept-select" aria-label={ariaLabel} onchange={change}>
    {#if placeholder}
      <option value="">{placeholder}</option>
    {/if}
    {#each groups as group (group.root.id)}
      <optgroup label={group.root.name}>
        {#each group.rows as row (row.department.id)}
          <option value={row.department.id} selected={isSelected(row.department.id)}>
            {'　'.repeat(row.depth)}{row.department.name}
          </option>
        {/each}
      </optgroup>
    {/each}
  </select>
{/if}

<style>
  .dept-select {
    min-width: 160px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: 7px 10px;
    background: var(--color-surface);
    color: var(--color-text);
    font-size: 13px;
  }

  .dept-select:focus { border-color: var(--color-primary); outline: none; }

  /* 触发框:视觉与原生 select 对齐(globals 里 input/select 的基础样式) */
  .dept-trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 33px;
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface-sunken);
    color: var(--color-text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition: border-color var(--transition-fast);
  }

  .dept-trigger:focus-visible { border-color: var(--color-primary); outline: none; }
  .dept-trigger-text { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dept-trigger-text.empty { color: var(--color-text-muted); }

  .chevron {
    flex: none;
    width: 8px;
    height: 8px;
    border-right: 1.5px solid var(--color-text-muted);
    border-bottom: 1.5px solid var(--color-text-muted);
    transform: rotate(45deg) translateY(-2px);
    transition: transform var(--transition-fast);
  }
  .chevron.open { transform: rotate(225deg) translateY(-1px); }

  /* 面板:fixed 定位,脱离父级 overflow 限制(z-index 高于 Modal 的 1000) */
  .dept-panel {
    position: fixed;
    z-index: 1100;
    overflow-y: auto;
    padding: 6px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-popover);
  }

  .dept-option {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .dept-option:hover { background: var(--color-primary-soft); }

  .checkbox {
    flex: none;
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    border: 1px solid var(--color-border-strong);
    border-radius: 3px;
    color: #fff;
    font-size: 11px;
    line-height: 1;
  }
  .checkbox.checked { background: var(--color-primary); border-color: var(--color-primary); }

  .dept-empty { padding: 10px; color: var(--color-text-muted); font-size: 13px; }
</style>
