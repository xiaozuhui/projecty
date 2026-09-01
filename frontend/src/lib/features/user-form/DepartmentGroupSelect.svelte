<script lang="ts">
  // 部门下拉选择:optgroup 按顶级部门分组,子部门按层级缩进挂在分组下。
  // multiple 时 value 为 string[](多选),单选时为 string;placeholder 仅单选生效(值 '' 表示未选)。
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

  const isSelected = (id: string) => (Array.isArray(value) ? value.includes(id) : value === id);

  function change(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    onchange(
      multiple ? [...select.selectedOptions].map((option) => option.value) : select.value
    );
  }
</script>

<select class="dept-select" class:multiple {multiple} aria-label={ariaLabel} onchange={change}>
  {#if !multiple && placeholder}
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

  .dept-select.multiple {
    display: block;
    width: 100%;
    min-height: 148px;
    padding: 6px;
  }
</style>
