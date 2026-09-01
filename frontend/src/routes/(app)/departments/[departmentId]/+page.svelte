<script lang="ts">
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import {
    createDepartment,
    deleteDepartment,
    listDepartmentMembers,
    listDepartmentProjects,
    listDepartments,
    updateDepartment
  } from '$lib/api/departments';
  import type { DepartmentMember, DepartmentView, ProjectView } from '$lib/api/types';
  import { meStore } from '$lib/features/auth/me.svelte';
  import { confirmDialog, promptDialog } from '$lib/features/ui/dialog.svelte';
  import { bindReload } from '$lib/features/ui/page-refresh.svelte';

  const id = $derived(String(page.params.departmentId ?? ''));

  let all = $state<DepartmentView[]>([]);
  let members = $state<DepartmentMember[]>([]);
  let projects = $state<ProjectView[]>([]);
  let loading = $state(true);
  let errorMessage = $state('');
  let tab = $state<'members' | 'projects'>('members');

  // 管理操作仅对超管可见,服务端仍以 require_admin 兜底。
  const isAdmin = $derived(meStore.isAdmin);

  const department = $derived(all.find((item) => item.id === id) ?? null);
  const parent = $derived(department?.parent_id ? all.find((item) => item.id === department.parent_id) ?? null : null);
  const children = $derived(
    all
      .filter((item) => item.parent_id === id)
      .sort((left, right) => left.sort_order - right.sort_order || left.name.localeCompare(right.name))
  );

  // 部门信息编辑(名称/编码/排序)
  let editing = $state(false);
  let editName = $state('');
  let editCode = $state('');
  let editSort = $state('0');
  let savingEdit = $state(false);

  // 新增下级部门
  let showChildForm = $state(false);
  let childName = $state('');
  let childCode = $state('');
  let childSort = $state('0');
  let savingChild = $state(false);

  let busyChildId = $state<string | null>(null);

  async function load() {
    loading = true;
    errorMessage = '';
    try {
      const [departmentResponse, projectResponse, memberResponse] = await Promise.all([
        listDepartments(),
        listDepartmentProjects(id),
        listDepartmentMembers(id)
      ]);
      all = departmentResponse.data.items;
      projects = projectResponse.data.items;
      members = memberResponse.data.items;
      editing = false;
      showChildForm = false;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '部门详情加载失败';
    } finally {
      loading = false;
    }
  }

  function startEdit() {
    if (!department) return;
    editName = department.name;
    editCode = department.code;
    editSort = String(department.sort_order);
    editing = true;
  }

  async function saveEdit(event: SubmitEvent) {
    event.preventDefault();
    if (!editName.trim() || !editCode.trim()) {
      errorMessage = '部门名称与部门编码不能为空';
      return;
    }
    savingEdit = true;
    errorMessage = '';
    try {
      const updated = (
        await updateDepartment(id, {
          name: editName.trim(),
          code: editCode.trim(),
          sort_order: Number(editSort) || 0
        })
      ).data;
      all = all.map((item) => (item.id === updated.id ? updated : item));
      editing = false;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '部门信息保存失败';
    } finally {
      savingEdit = false;
    }
  }

  async function addChild(event: SubmitEvent) {
    event.preventDefault();
    if (!childName.trim() || !childCode.trim()) {
      errorMessage = '下级部门名称与编码不能为空';
      return;
    }
    savingChild = true;
    errorMessage = '';
    try {
      const created = (
        await createDepartment({
          parent_id: id,
          name: childName.trim(),
          code: childCode.trim(),
          sort_order: Number(childSort) || 0
        })
      ).data;
      all = [...all, created];
      childName = '';
      childCode = '';
      childSort = '0';
      showChildForm = false;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '新增下级部门失败';
    } finally {
      savingChild = false;
    }
  }

  async function renameChild(child: DepartmentView) {
    const value = await promptDialog({ title: '重命名下级部门', label: '部门名称', initial: child.name });
    if (!value?.trim() || value.trim() === child.name) return;
    busyChildId = child.id;
    errorMessage = '';
    try {
      const updated = (await updateDepartment(child.id, { name: value.trim() })).data;
      all = all.map((item) => (item.id === updated.id ? updated : item));
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '修改下级部门失败';
    } finally {
      busyChildId = null;
    }
  }

  async function removeChild(child: DepartmentView) {
    if (
      !(await confirmDialog({
        title: '删除下级部门',
        message: `确定逻辑删除部门“${child.name}”吗？仍含下级或关联项目时会被拒绝。`,
        confirmLabel: '删除',
        danger: true
      }))
    ) {
      return;
    }
    busyChildId = child.id;
    errorMessage = '';
    try {
      await deleteDepartment(child.id);
      all = all.filter((item) => item.id !== child.id);
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '删除下级部门失败';
    } finally {
      busyChildId = null;
    }
  }

  const roleName = (role: string) => (role === 'super_admin' ? '超级管理员' : '成员');
  const formatDate = (value: string) =>
    new Date(value).toLocaleDateString('zh-CN', { year: 'numeric', month: 'numeric', day: 'numeric' });

  bindReload(() => void load());
</script>

{#if loading}
  <section class="workspace-card state-box">正在加载部门详情…</section>
{:else if department}
  <PageHeader
    title={department.name}
    crumbs={[
      { label: '部门', href: '/departments' },
      ...(parent ? [{ label: parent.name, href: `/departments/${parent.id}` }] : []),
      { label: department.name }
    ]}
    description="部门信息、下级部门、员工与关联项目统一在这里维护。"
  />

  {#if errorMessage}<div class="workspace-card error-state" role="alert">{errorMessage}</div>{/if}

  <section class="workspace-card">
    <div class="info-toolbar">
      <h2 class="section-heading">部门信息</h2>
      {#if isAdmin && !editing}
        <button class="secondary-button" type="button" onclick={startEdit}>编辑</button>
      {/if}
    </div>

    {#if editing}
      <form class="edit-form" onsubmit={saveEdit}>
        <label>
          <span>部门名称</span>
          <input class="field" bind:value={editName} placeholder="例如 研发中心" />
        </label>
        <label>
          <span>部门编码</span>
          <input class="field" bind:value={editCode} placeholder="例如 RD" />
        </label>
        <label>
          <span>排序</span>
          <input class="field" type="number" bind:value={editSort} />
        </label>
        <button class="primary-button" type="submit" disabled={savingEdit}>{savingEdit ? '保存中…' : '保存'}</button>
        <button class="ghost-button" type="button" onclick={() => (editing = false)}>取消</button>
      </form>
    {:else}
      <dl class="info-list">
        <div><dt>部门编码</dt><dd>{department.code}</dd></div>
        <div><dt>上级部门</dt><dd>{parent ? parent.name : '顶级部门'}</dd></div>
        <div><dt>下级部门</dt><dd>{children.length} 个</dd></div>
        <div><dt>部门员工</dt><dd>{members.length} 人</dd></div>
        <div><dt>创建时间</dt><dd>{formatDate(department.created_at)}</dd></div>
      </dl>
    {/if}

    <div class="child-section">
      <div class="info-toolbar">
        <h3 class="section-heading">下级部门</h3>
        {#if isAdmin}
          <button class="secondary-button" type="button" onclick={() => (showChildForm = !showChildForm)}>
            {showChildForm ? '收起' : '新增下级部门'}
          </button>
        {/if}
      </div>

      {#if isAdmin && showChildForm}
        <form class="edit-form" onsubmit={addChild}>
          <label>
            <span>部门名称</span>
            <input class="field" bind:value={childName} placeholder="例如 前端组" />
          </label>
          <label>
            <span>部门编码</span>
            <input class="field" bind:value={childCode} placeholder="例如 FE" />
          </label>
          <label>
            <span>排序</span>
            <input class="field" type="number" bind:value={childSort} />
          </label>
          <button class="primary-button" type="submit" disabled={savingChild}>{savingChild ? '创建中…' : '创建'}</button>
        </form>
      {/if}

      <div class="child-list">
        {#each children as child (child.id)}
          <div class="child-row">
            <a href={`/departments/${child.id}`}>
              <strong>{child.name}</strong>
              <span>{child.code}</span>
            </a>
            {#if isAdmin}
              <span class="child-actions">
                <button class="text-button" type="button" disabled={busyChildId === child.id} onclick={() => renameChild(child)}>编辑</button>
                <button class="text-button danger-text" type="button" disabled={busyChildId === child.id} onclick={() => removeChild(child)}>删除</button>
              </span>
            {/if}
          </div>
        {:else}
          <p class="empty-inline">没有下级部门。</p>
        {/each}
      </div>
    </div>
  </section>

  <nav class="detail-tabs" aria-label="部门数据导航">
    <button type="button" class:active={tab === 'members'} aria-current={tab === 'members' ? 'page' : undefined} onclick={() => (tab = 'members')}>
      员工 <small>{members.length}</small>
    </button>
    <button type="button" class:active={tab === 'projects'} aria-current={tab === 'projects' ? 'page' : undefined} onclick={() => (tab = 'projects')}>
      关联项目 <small>{projects.length}</small>
    </button>
  </nav>

  {#if tab === 'members'}
    <section class="workspace-card">
      {#if members.length}
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr><th>姓名</th><th>账号</th><th>系统角色</th><th>状态</th><th>加入时间</th></tr>
            </thead>
            <tbody>
              {#each members as member (member.user_id)}
                <tr>
                  <td class="cell-strong">{member.display_name}</td>
                  <td>{member.account}</td>
                  <td>{roleName(member.system_role)}</td>
                  <td><span class:offline={!member.is_active}>{member.is_active ? '在职' : '已停用'}</span></td>
                  <td>{formatDate(member.joined_at)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <p class="empty-inline">还没有员工加入该部门，可在「用户」页为账号分配部门。</p>
      {/if}
    </section>
  {:else}
    <section class="workspace-card">
      <div class="project-list">
        {#each projects as project (project.id)}
          <a href={`/projects/${project.project_key}`}>
            <strong>{project.project_key} · {project.name}</strong>
            <span>{project.description || '暂无描述'}</span>
          </a>
        {:else}
          <p class="empty-inline">没有可见的关联项目。</p>
        {/each}
      </div>
    </section>
  {/if}
{:else}
  <section class="workspace-card error-state">{errorMessage || '部门不存在'}</section>
{/if}

<style>
  .workspace-card { margin-bottom: 16px; }

  .info-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .info-list {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 12px;
    margin: 0;
  }

  .info-list div {
    display: grid;
    gap: 4px;
    padding: 10px 12px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border-weak);
    border-radius: var(--radius-md);
  }

  .info-list dt { color: var(--color-text-muted); font-size: 12px; }
  .info-list dd { margin: 0; font-size: 13px; font-weight: 500; }

  .edit-form {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 10px;
    padding: 12px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border-weak);
    border-radius: var(--radius-md);
  }

  .edit-form label {
    display: grid;
    gap: 6px;
    min-width: 150px;
    color: var(--color-text-muted);
    font-size: 12px;
    font-weight: 500;
  }

  .child-section { margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--color-border); }

  .child-list { display: grid; gap: 8px; }

  .child-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border-left: 2px solid var(--color-primary-soft);
    border-radius: var(--radius-md);
    background: var(--color-surface-sunken);
  }

  .child-row a { display: grid; gap: 3px; text-decoration: none; color: var(--color-text); }
  .child-row a:hover strong { color: var(--color-primary-strong); }
  .child-row span { color: var(--color-text-muted); font-size: 12px; }
  .child-actions { display: flex; gap: 4px; }
  .child-actions .text-button:disabled { cursor: not-allowed; opacity: 0.45; text-decoration: none; }

  .detail-tabs {
    display: flex;
    gap: 2px;
    margin-bottom: 0;
    border-bottom: 1px solid var(--color-border);
  }

  .detail-tabs button {
    padding: 8px 10px;
    margin-bottom: -1px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 13px;
    cursor: pointer;
    transition: color var(--transition-fast), border-color var(--transition-fast);
  }

  .detail-tabs button:hover { color: var(--color-text); }
  .detail-tabs button.active { color: var(--color-text); border-bottom-color: var(--color-primary); }
  .detail-tabs small { margin-left: 4px; font-size: 11px; }

  .table-wrap { overflow-x: auto; }
  .table-wrap table { min-width: 560px; }
  .cell-strong { color: var(--color-text); font-weight: 500; }
  .offline { color: var(--color-danger); }

  .project-list { display: grid; gap: 8px; }
  .project-list a {
    display: grid;
    gap: 4px;
    padding: 11px 12px;
    border-radius: var(--radius-md);
    background: var(--color-surface-sunken);
    text-decoration: none;
    color: var(--color-text);
  }
  .project-list a:hover strong { color: var(--color-primary-strong); }
  .project-list span { color: var(--color-text-muted); font-size: 13px; }
</style>
