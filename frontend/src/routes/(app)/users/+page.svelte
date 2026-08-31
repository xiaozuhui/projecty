<script lang="ts">
  import { onMount } from 'svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { listDepartments } from '$lib/api/departments';
  import { me } from '$lib/api/auth';
  import { createUser, downloadUserTemplate, importUsers, listUsers, updateUser, type UserListQuery } from '$lib/api/users';
  import type { DepartmentView, MeResponse, UserImportReport, UserView } from '$lib/api/types';

  let currentUser = $state<MeResponse | null>(null);
  let users = $state<UserView[]>([]);
  let departments = $state<DepartmentView[]>([]);
  let loading = $state(true);
  let errorMessage = $state('');
  let search = $state('');
  let departmentFilter = $state('');
  let includeInactive = $state(false);
  let account = $state('');
  let displayName = $state('');
  let password = $state('');
  let role = $state<'user' | 'super_admin'>('user');
  let selectedDepartments = $state<string[]>([]);
  let saving = $state(false);
  let importFile = $state<File | null>(null);
  let importing = $state(false);
  let importReport = $state<UserImportReport | null>(null);

  const roleLabel = (value: string) => (value === 'super_admin' ? '超级管理员' : '员工');

  async function load() {
    const query: UserListQuery = {};
    if (search.trim()) query.search = search.trim();
    if (departmentFilter) query.department_id = departmentFilter;
    if (includeInactive) query.include_inactive = true;
    try {
      users = (await listUsers(query)).data.items;
      errorMessage = '';
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '用户加载失败';
    }
  }

  onMount(async () => {
    try {
      currentUser = (await me()).data;
      if (currentUser.system_role !== 'super_admin') return;
      departments = (await listDepartments()).data.items;
      await load();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '页面初始化失败';
    } finally {
      loading = false;
    }
  });

  async function submitCreate(event: SubmitEvent) {
    event.preventDefault();
    if (!account.trim() || !displayName.trim() || !password) return;
    saving = true;
    try {
      const created = (await createUser({ account: account.trim(), password, display_name: displayName.trim(), system_role: role, department_ids: selectedDepartments })).data;
      users = [created, ...users];
      account = ''; displayName = ''; password = ''; role = 'user'; selectedDepartments = [];
      errorMessage = '';
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '新增用户失败';
    } finally {
      saving = false;
    }
  }

  function toggleDepartment(id: string) {
    selectedDepartments = selectedDepartments.includes(id) ? selectedDepartments.filter((item) => item !== id) : [...selectedDepartments, id];
  }

  async function rename(user: UserView) {
    const value = prompt('新的姓名', user.display_name);
    if (!value?.trim() || value.trim() === user.display_name) return;
    try {
      const updated = (await updateUser(user.id, { display_name: value.trim() })).data;
      users = users.map((item) => (item.id === user.id ? { ...item, ...updated } : item));
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '修改姓名失败';
    }
  }

  async function resetPassword(user: UserView) {
    const value = prompt(`为「${user.display_name}」设置新密码(8-128 位)`);
    if (!value) return;
    try {
      await updateUser(user.id, { password: value });
      alert('密码已重置，该账号的所有登录会话已失效。');
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '重置密码失败';
    }
  }

  async function changeDepartments(user: UserView) {
    const current = user.departments.map((item) => item.name).join('/');
    const value = prompt('调整部门归属：多个部门用 / 分隔，留空表示移出全部部门', current);
    if (value === null) return;
    const names = value.split('/').map((name) => name.trim()).filter(Boolean);
    const ids: string[] = [];
    for (const name of names) {
      const department = departments.find((item) => item.name === name);
      if (!department) { alert(`部门不存在：${name}`); return; }
      ids.push(department.id);
    }
    try {
      const updated = (await updateUser(user.id, { department_ids: ids })).data;
      users = users.map((item) => (item.id === user.id ? { ...item, ...updated } : item));
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '调整部门失败';
    }
  }

  async function toggleActive(user: UserView) {
    const action = user.is_active ? '停用' : '启用';
    if (!confirm(`确定${action}「${user.display_name}」吗？`)) return;
    try {
      const updated = (await updateUser(user.id, { is_active: !user.is_active })).data;
      if (!includeInactive && !updated.is_active) users = users.filter((item) => item.id !== user.id);
      else users = users.map((item) => (item.id === user.id ? { ...item, ...updated } : item));
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : `${action}失败`;
    }
  }

  async function downloadTemplate() {
    try {
      const blob = await downloadUserTemplate();
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = 'projecty-users-template.xlsx';
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '模板下载失败';
    }
  }

  async function submitImport() {
    if (!importFile) return;
    importing = true;
    importReport = null;
    try {
      importReport = (await importUsers(importFile)).data;
      errorMessage = '';
      await load();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '导入失败';
    } finally {
      importing = false;
    }
  }
</script>

<PageHeader title="用户管理" eyebrow="Users" description="超级管理员可以创建员工、调整部门归属，并通过 Excel 模板批量导入。" />

{#if errorMessage}<div class="workspace-card error-state">{errorMessage}</div>{/if}

{#if currentUser && currentUser.system_role !== 'super_admin'}
  <section class="workspace-card state-box">只有超级管理员可以访问用户管理。</section>
{:else if !loading}
  <section class="workspace-card create-card">
    <h2>新增员工</h2>
    <form onsubmit={submitCreate}>
      <input bind:value={account} placeholder="登录账号(2-64 字符)" />
      <input bind:value={displayName} placeholder="姓名" />
      <input bind:value={password} type="password" placeholder="初始密码(8-128 位)" />
      <select bind:value={role}>
        <option value="user">员工</option>
        <option value="super_admin">超级管理员</option>
      </select>
      <div class="department-picker">
        {#each departments as department}<label><input type="checkbox" checked={selectedDepartments.includes(department.id)} onchange={() => toggleDepartment(department.id)} />{department.name}</label>{/each}
        {#if !departments.length}<small class="muted">还没有部门，可先到「部门」页面创建。</small>{/if}
      </div>
      <button class="primary-button" disabled={saving}>{saving ? '保存中…' : '创建用户'}</button>
    </form>
  </section>

  <section class="workspace-card import-card">
    <h2>批量导入</h2>
    <p class="muted">先下载 Excel 模板，按说明填写后上传；部门填写系统中已存在的名称，多个部门用 / 分隔。已存在的账号所在行会失败，不影响其他行。</p>
    <div class="import-actions">
      <button type="button" class="text-button" onclick={downloadTemplate}>下载导入模板</button>
      <input type="file" accept=".xlsx" onchange={(event) => (importFile = (event.currentTarget as HTMLInputElement).files?.[0] ?? null)} />
      <button class="primary-button" onclick={submitImport} disabled={!importFile || importing}>{importing ? '导入中…' : '开始导入'}</button>
    </div>
    {#if importReport}
      <div class="import-summary">
        共 {importReport.total} 行，成功 {importReport.succeeded}，失败 {importReport.failed}
      </div>
      {#if importReport.rows.length}
        <div class="table-wrap">
          <table>
            <thead><tr><th>行号</th><th>账号</th><th>结果</th><th>说明</th></tr></thead>
            <tbody>
              {#each importReport.rows as row}
                <tr><td>{row.row_number}</td><td>{row.account || '—'}</td><td class={row.success ? 'ok' : 'bad'}>{row.success ? '成功' : '失败'}</td><td>{row.message}</td></tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {/if}
  </section>

  <section class="workspace-card list-card">
    <div class="filters">
      <input bind:value={search} placeholder="搜索账号或姓名" onkeydown={(event) => event.key === 'Enter' && load()} />
      <select bind:value={departmentFilter} onchange={load}>
        <option value="">全部部门</option>
        {#each departments as department}<option value={department.id}>{department.name}</option>{/each}
      </select>
      <label class="muted"><input type="checkbox" bind:checked={includeInactive} onchange={load} />包含已停用</label>
      <button type="button" class="text-button" onclick={load}>查询</button>
    </div>
    <div class="table-wrap">
      <table>
        <thead><tr><th>账号</th><th>姓名</th><th>角色</th><th>部门</th><th>状态</th><th>创建时间</th><th>操作</th></tr></thead>
        <tbody>
          {#each users as user}
            <tr>
              <td>{user.account}</td>
              <td>{user.display_name}</td>
              <td>{roleLabel(user.system_role)}</td>
              <td>{user.departments.map((item) => item.name).join(' / ') || '—'}</td>
              <td>{user.is_active ? '在职' : '已停用'}</td>
              <td class="muted">{new Date(user.created_at).toLocaleDateString()}</td>
              <td class="actions">
                <button class="text-button" onclick={() => rename(user)}>改名</button>
                <button class="text-button" onclick={() => changeDepartments(user)}>部门</button>
                <button class="text-button" onclick={() => resetPassword(user)}>重置密码</button>
                <button class="text-button danger-text" onclick={() => toggleActive(user)}>{user.is_active ? '停用' : '启用'}</button>
              </td>
            </tr>
          {:else}
            <tr><td colspan="7" class="muted empty-inline">没有匹配的用户。</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>
{/if}

<style>
  .create-card h2, .import-card h2 { margin: 0 0 14px; }
  .create-card form { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; align-items: center; }
  .department-picker { grid-column: 1 / 4; display: flex; flex-wrap: wrap; gap: 10px 16px; }
  .department-picker label { display: inline-flex; align-items: center; gap: 6px; font-size: 13px; }
  .create-card form > button { justify-self: end; }
  .import-card .import-actions { display: flex; flex-wrap: wrap; align-items: center; gap: 14px; margin-top: 10px; }
  .import-summary { margin-top: 12px; font-weight: 650; }
  .filters { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-bottom: 12px; }
  .table-wrap { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  th { text-align: left; padding: 8px 10px; color: var(--color-text-muted); font-size: 12px; border-bottom: 1px solid var(--color-border); white-space: nowrap; }
  td { padding: 10px; border-bottom: 1px solid var(--color-border); vertical-align: top; }
  td.actions { white-space: nowrap; }
  td.actions .text-button { margin-right: 6px; }
  .ok { color: #137a3d; font-weight: 650; }
  .bad { color: var(--color-danger); font-weight: 650; }
  .muted { color: var(--color-text-muted); font-size: 12px; }
  .empty-inline { text-align: center; padding: 22px 0; }
  .text-button { border: 0; background: transparent; cursor: pointer; font-weight: 700; color: var(--color-primary-strong); }
  .danger-text { color: var(--color-danger); }
  .error-state { color: var(--color-danger); margin-bottom: 16px; }
  .state-box { text-align: center; color: var(--color-text-muted); }
  .workspace-card { margin-bottom: 18px; }
  @media (max-width: 900px) { .create-card form { grid-template-columns: 1fr 1fr; } .department-picker { grid-column: auto; } }
</style>
