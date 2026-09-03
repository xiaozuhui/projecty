<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import { me, updateProfile, changePassword } from '$lib/api/auth';
  import { meStore } from '$lib/features/auth/me.svelte';
  import { session } from '$lib/features/auth/session.svelte';
  import type { MeResponse } from '$lib/api/types';

  let user = $state<MeResponse | null>(null);
  let loading = $state(true);
  let errorMessage = $state('');
  let editOpen = $state(false);
  let editName = $state('');
  let editEmail = $state('');
  let saving = $state(false);
  let editError = $state('');
  let resetOpen = $state(false);
  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let resetSaving = $state(false);
  let resetError = $state('');
  let resetDone = $state(false);

  const roleLabel = (value: string) => (value === 'super_admin' ? '超级管理员' : '普通用户');

  onMount(async () => {
    try {
      user = (await me()).data;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '资料加载失败';
    } finally {
      loading = false;
    }
  });

  function openEdit() {
    editName = user?.display_name ?? '';
    editEmail = user?.email ?? '';
    editError = '';
    editOpen = true;
  }

  async function saveEdit(event: SubmitEvent) {
    event.preventDefault();
    if (!editName.trim()) {
      editError = '姓名不能为空';
      return;
    }
    saving = true;
    editError = '';
    try {
      user = (await updateProfile(editName.trim(), editEmail.trim() || null)).data;
      meStore.set(user);
      editOpen = false;
      errorMessage = '';
    } catch (error) {
      editError = error instanceof Error ? error.message : '保存失败';
    } finally {
      saving = false;
    }
  }

  function openReset() {
    currentPassword = '';
    newPassword = '';
    confirmPassword = '';
    resetError = '';
    resetDone = false;
    resetOpen = true;
  }

  async function saveReset(event: SubmitEvent) {
    event.preventDefault();
    resetError = '';
    if (newPassword !== confirmPassword) {
      resetError = '两次输入的新密码不一致';
      return;
    }
    if (newPassword.length < 8) {
      resetError = '新密码至少需要 8 个字符';
      return;
    }
    resetSaving = true;
    try {
      await changePassword(currentPassword, newPassword);
      resetDone = true;
      session.clear();
      setTimeout(() => goto('/login'), 800);
    } catch (error) {
      resetError = error instanceof Error ? error.message : '密码修改失败';
    } finally {
      resetSaving = false;
    }
  }
</script>

<PageHeader title="个人资料" eyebrow="Profile" description="当前登录账号的身份信息。姓名、邮箱与密码可自行维护，账号与角色由系统管理员管理。" />

{#if loading}
  <section class="workspace-card state-box">正在加载个人资料…</section>
{:else if user}
  <section class="workspace-card profile">
    <div class="profile-avatar">{user.display_name.slice(0, 1)}</div>
    <div class="profile-main">
      <div class="profile-head">
        <dl>
          <div><dt>显示名称</dt><dd>{user.display_name}</dd></div>
          <div><dt>账号</dt><dd>{user.account}</dd></div>
          <div><dt>邮箱</dt><dd>{user.email || '未设置'}</dd></div>
          <div><dt>系统角色</dt><dd>{roleLabel(user.system_role)}</dd></div>
          <div><dt>用户 ID</dt><dd class="mono">{user.id}</dd></div>
        </dl>
        <div class="profile-actions">
          <button type="button" class="secondary-button" onclick={openReset}>重置密码</button>
          <button type="button" class="secondary-button" onclick={openEdit}>编辑资料</button>
        </div>
      </div>
    </div>
  </section>
{:else}
  <section class="workspace-card error-state">{errorMessage}</section>
{/if}

<Modal open={editOpen} title="编辑个人资料" onClose={() => (editOpen = false)}>
  <form id="profile-edit-form" class="edit-form" onsubmit={saveEdit}>
    <label>
      显示名称
      <input bind:value={editName} placeholder="姓名(不超过 80 个字符)" />
    </label>
    <label>
      邮箱
      <input bind:value={editEmail} type="email" placeholder="name@example.com(可选)" />
    </label>
    <small class="hint">邮箱用于身份识别，全局唯一；留空表示清除已绑定的邮箱。</small>
    {#if editError}<p class="error-message">{editError}</p>{/if}
  </form>
  {#snippet footer()}
    <button class="secondary-button" type="button" onclick={() => (editOpen = false)}>取消</button>
    <button class="primary-button" type="submit" form="profile-edit-form" disabled={saving}>
      {saving ? '保存中…' : '保存'}
    </button>
  {/snippet}
</Modal>

<Modal open={resetOpen} title="重置密码" onClose={() => (resetOpen = false)}>
  {#if resetDone}
    <p class="success-message">密码已修改，正在前往登录页…</p>
  {:else}
    <form id="password-reset-form" class="edit-form" onsubmit={saveReset}>
      <label>
        当前密码
        <input bind:value={currentPassword} type="password" autocomplete="current-password" />
      </label>
      <label>
        新密码
        <input bind:value={newPassword} type="password" autocomplete="new-password" placeholder="至少 8 个字符" />
      </label>
      <label>
        确认新密码
        <input bind:value={confirmPassword} type="password" autocomplete="new-password" />
      </label>
      <small class="hint">重置成功后所有登录会话都会失效，需要使用新密码重新登录。</small>
      {#if resetError}<p class="error-message">{resetError}</p>{/if}
    </form>
  {/if}
  {#snippet footer()}
    {#if !resetDone}
      <button class="secondary-button" type="button" onclick={() => (resetOpen = false)}>取消</button>
      <button class="primary-button" type="submit" form="password-reset-form" disabled={resetSaving}>
        {resetSaving ? '提交中…' : '确认重置'}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .profile { display: flex; align-items: start; gap: 24px; }
  .profile-avatar { display: grid; place-items: center; flex: 0 0 76px; height: 76px; border: 1px solid var(--color-border); border-radius: var(--radius-lg); color: var(--color-text-secondary); background: var(--color-hover); font-size: 26px; font-weight: 600; }
  .profile-main { flex: 1; min-width: 0; }
  .profile-head { display: flex; align-items: start; justify-content: space-between; gap: 20px; }
  dl { display: grid; gap: 12px; min-width: min(100%, 600px); margin: 0; }
  dl div { display: flex; justify-content: space-between; gap: 20px; padding: 11px 0; border-bottom: 1px solid var(--color-border); }
  dt { color: var(--color-text-muted); }
  dd { margin: 0; font-weight: 500; }
  .mono { font-family: var(--font-mono); font-size: 12px; }
  .profile-actions { display: flex; gap: 8px; flex: none; }
  .success-message { margin: 0; color: var(--color-success); }
  .edit-form { display: grid; gap: 14px; }
  .edit-form label { display: grid; gap: 6px; font-weight: 500; }
  .hint { color: var(--color-text-muted); }
  .error-message { margin: 0; color: var(--color-danger); }
  .state-box { text-align: center; color: var(--color-text-muted); }
  .error-state { color: var(--color-danger); }
  @media (max-width: 560px) {
    .profile { display: grid; }
    .profile-head { display: grid; gap: 14px; }
    dl div { display: grid; gap: 4px; }
  }
</style>
