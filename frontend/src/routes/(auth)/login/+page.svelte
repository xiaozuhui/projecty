<script lang="ts">
  import { goto } from '$app/navigation';
  import { login } from '$lib/api/auth';
  import { session } from '$lib/features/auth/session.svelte';

  let account = $state('');
  let password = $state('');
  let submitting = $state(false);
  let errorMessage = $state('');

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    errorMessage = '';
    if (!account.trim() || !password) {
      errorMessage = '请输入账号和密码';
      return;
    }

    submitting = true;
    try {
      const response = await login(account.trim(), password);
      session.set(response.data);
      await goto('/');
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : '登录失败，请稍后再试';
    } finally {
      submitting = false;
    }
  }
</script>

<main class="login-page">
  <section class="login-card" aria-labelledby="login-title">
    <div class="brand">Projecty</div>
    <h1 id="login-title">账号密码登录</h1>

    <form onsubmit={submit}>
      <label>
        账号
        <input bind:value={account} autocomplete="username" placeholder="请输入账号" />
      </label>
      <label>
        密码
        <input bind:value={password} autocomplete="current-password" type="password" placeholder="请输入密码" />
      </label>
      {#if errorMessage}
        <p class="error" role="alert">{errorMessage}</p>
      {/if}
      <button class="primary-button" type="submit" disabled={submitting}>
        {submitting ? '登录中…' : '登录'}
      </button>
    </form>
  </section>
</main>

<style>
  .login-page { display: grid; place-items: center; min-height: 100vh; padding: 24px; background: var(--color-bg); }
  .login-card { display: grid; gap: 14px; width: min(100%, 420px); padding: 28px; background: var(--color-surface); border: 1px solid var(--color-border); border-radius:var(--radius-lg); box-shadow: var(--shadow-md); }
  .brand { color: var(--color-primary); font-weight:600; font-size: 24px; letter-spacing: .02em; }
  h1 { margin: 0; }
  form { display: grid; gap: 14px; }
  label { display: grid; gap: 6px; font-weight:500; }
  input { border: 1px solid var(--color-border); border-radius:var(--radius-md); padding: 11px 12px; color: var(--color-text); background: var(--color-surface); }
  input:focus { outline: 3px solid var(--color-primary-soft); border-color: var(--color-primary); }
  button { border: 0; cursor: pointer; }
  button:disabled { cursor: wait; opacity: .65; }
  .error { color: var(--color-danger); font-size: 14px; margin: 0; }
</style>
