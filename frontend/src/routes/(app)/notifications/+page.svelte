<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { listNotifications, markAllNotificationsRead, markNotificationRead } from '$lib/api/notifications';
  import { unreadCount } from '$lib/api/notifications';
  import type { Notification } from '$lib/api/types';

  let items = $state<Notification[]>([]);
  let page = $state(1);
  let hasMore = $state(false);
  let unread = $state(0);
  let loading = $state(true);
  let appending = $state(false);
  let markingAll = $state(false);
  let openingId = $state<string | null>(null);
  let errorMessage = $state('');

  const typeLabel: Record<string, string> = {
    assigned: '分配',
    review_requested: '评审',
    commented: '评论',
    status_changed: '流转'
  };
  const timeLabel = (value: string) => {
    const diff = Date.now() - new Date(value).getTime();
    const minutes = Math.floor(diff / 60_000);
    if (minutes < 1) return '刚刚';
    if (minutes < 60) return `${minutes} 分钟前`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours} 小时前`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days} 天前`;
    return new Date(value).toLocaleDateString('zh-CN');
  };

  async function load(targetPage = 1, append = false) {
    if (append) appending = true;
    else {
      loading = true;
      errorMessage = '';
    }
    try {
      const response = await listNotifications(targetPage);
      items = append ? [...items, ...response.data.items] : response.data.items;
      page = response.data.page;
      hasMore = response.data.page * response.data.page_size < response.data.total;
      unread = response.data.unread_count;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '通知加载失败';
    } finally {
      loading = false;
      appending = false;
    }
  }

  async function markAll() {
    markingAll = true;
    errorMessage = '';
    try {
      await markAllNotificationsRead();
      items = items.map((item) => ({ ...item, read_at: item.read_at ?? new Date().toISOString() }));
      unread = 0;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '操作失败';
    } finally {
      markingAll = false;
    }
  }

  // 点通知:先标记已读再跳任务详情,失败不影响跳转。
  async function open(item: Notification) {
    openingId = item.id;
    try {
      if (!item.read_at) await markNotificationRead(item.id);
    } catch {
      /* 已读标记失败不阻塞跳转 */
    } finally {
      openingId = null;
      void goto(`/tasks/${item.task_key}`);
    }
  }

  onMount(() => { void load(1); });
</script>

<PageHeader
  title="通知"
  eyebrow="Notifications"
  description="任务分配、评审邀请、评论与状态流转的站内通知。"
/>

<section class="workspace-card">
  <div class="toolbar">
    <div>
      <h2>通知列表</h2>
      <p>{unread > 0 ? `${unread} 条未读` : '没有未读通知'}</p>
    </div>
    <button class="secondary-button" type="button" onclick={markAll} disabled={markingAll || !unread}>
      {markingAll ? '处理中…' : '全部标为已读'}
    </button>
  </div>

  {#if errorMessage}
    <p class="error-message" role="alert">{errorMessage}</p>
  {/if}

  {#if loading}
    <div class="state-box">正在加载通知…</div>
  {:else if !items.length}
    <div class="state-box">
      <strong>暂无通知</strong>
      <p>任务分配给你、请你评审或有人评论时,这里会出现提醒。</p>
    </div>
  {:else}
    <div class="notification-list">
      {#each items as item (item.id)}
        <button
          class="notification-row"
          class:unread={!item.read_at}
          type="button"
          disabled={openingId === item.id}
          onclick={() => open(item)}
        >
          <span class="dot" aria-hidden="true"></span>
          <span class="type-badge t-{item.type}">{typeLabel[item.type] ?? '通知'}</span>
          <span class="summary">{item.summary}</span>
          <time>{timeLabel(item.created_at)}</time>
        </button>
      {/each}
    </div>
    {#if hasMore}
      <div class="pager">
        <button class="secondary-button" type="button" disabled={appending} onclick={() => void load(page + 1, true)}>
          {appending ? '加载中…' : '加载更多'}
        </button>
      </div>
    {/if}
  {/if}
</section>

<style>
  .toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 14px; }
  .toolbar h2 { margin: 0; font-size: 18px; }
  .toolbar p { margin: 4px 0 0; color: var(--color-text-muted); font-size: 13px; }
  .toolbar button { border: 0; }

  .notification-list { display: grid; }
  .notification-row {
    display: grid;
    grid-template-columns: 10px 44px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 12px 4px;
    border: 0;
    border-bottom: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text);
    font-size: 13px;
    text-align: left;
    text-decoration: none;
    cursor: pointer;
    transition: background-color var(--transition-fast);
  }
  .notification-row:last-child { border-bottom: 0; }
  .notification-row:hover { background: var(--color-hover); }
  .notification-row .dot { width: 6px; height: 6px; border-radius: 50%; background: transparent; }
  .notification-row.unread .dot { background: var(--color-primary); }
  .notification-row.unread .summary { font-weight: 500; }
  .type-badge { padding: 1px 7px; border: 1px solid var(--color-border); border-radius: 999px; font-size: 12px; color: var(--color-text-muted); text-align: center; }
  .t-assigned { color: var(--color-primary); }
  .t-review_requested { color: #8b5cf6; }
  .t-commented { color: #0f766e; }
  .t-status_changed { color: var(--color-warning); }
  .summary { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .notification-row time { color: var(--color-text-muted); font-size: 12px; white-space: nowrap; }

  .state-box { display: grid; place-items: center; gap: 8px; min-height: 220px; color: var(--color-text-muted); }
  .state-box strong { color: var(--color-text-secondary); font-size: 14px; font-weight: 500; }
  .state-box p { font-size: 13px; }
  .pager { display: flex; justify-content: center; margin-top: 14px; }
  .error-message { margin: 0 0 14px; color: var(--color-danger); font-size: 13px; }

  @media (max-width: 640px) {
    .notification-row { grid-template-columns: 10px minmax(0, 1fr); row-gap: 4px; }
    .notification-row .type-badge { display: none; }
    .notification-row time { grid-column: 2; }
  }
</style>
