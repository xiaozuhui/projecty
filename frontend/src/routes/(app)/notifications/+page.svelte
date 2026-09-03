<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import { ApiClientError } from '$lib/api/client';
  import { listNotifications, markAllNotificationsRead, markNotificationRead } from '$lib/api/notifications';
  import type { Notification } from '$lib/api/types';

  type Filter = 'all' | 'unread';

  let filter = $state<Filter>('all');
  let items = $state<Notification[]>([]);
  let page = $state(1);
  let total = $state(0);
  let hasMore = $state(false);
  let unread = $state(0);
  let loading = $state(true);
  let appending = $state(false);
  let markingAll = $state(false);
  let openingId = $state<string | null>(null);
  let errorMessage = $state('');

  const typeMeta: Record<string, { glyph: string; label: string }> = {
    mentioned: { glyph: '@', label: '提及' },
    assigned: { glyph: '👤', label: '分配' },
    review_requested: { glyph: '✓', label: '评审' },
    commented: { glyph: '💬', label: '评论' },
    status_changed: { glyph: '↻', label: '流转' }
  };
  const metaOf = (type: string) => typeMeta[type] ?? { glyph: '•', label: '通知' };

  const timeLabel = (value: string) => {
    const diff = Date.now() - new Date(value).getTime();
    const minutes = Math.floor(diff / 60_000);
    if (minutes < 1) return '刚刚';
    if (minutes < 60) return `${minutes} 分钟前`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours} 小时前`;
    if (hours < 48) return `昨天 ${new Date(value).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days} 天前`;
    return new Date(value).toLocaleDateString('zh-CN');
  };

  // 摘要是整句文案,展示层再拆两级:首个词为操作人(加粗),任务编号片段用等宽字体。
  function summaryParts(item: Notification) {
    const actorEnd = item.summary.indexOf(' ');
    const actor = actorEnd > 0 ? item.summary.slice(0, actorEnd) : '';
    const rest = actor ? item.summary.slice(actorEnd + 1) : item.summary;
    const segments = rest.split(item.task_key);
    return { actor, segments };
  }

  const visible = $derived(filter === 'unread' ? items.filter((item) => !item.read_at) : items);

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
      total = response.data.total;
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

<header class="page-head">
  <h1>通知</h1>
  <div class="meta-row">
    <span class="meta-item">{unread > 0 ? `${unread} 条未读` : '没有未读通知'}</span><span class="sep">·</span>
    <span class="meta-item">共 {total} 条</span>
  </div>
</header>

<div class="toolbar">
  <div class="segmented" role="tablist" aria-label="通知过滤">
    <button class:active={filter === 'all'} role="tab" aria-selected={filter === 'all'} type="button" onclick={() => (filter = 'all')}>全部</button>
    <button class:active={filter === 'unread'} role="tab" aria-selected={filter === 'unread'} type="button" onclick={() => (filter = 'unread')}>未读 {unread}</button>
  </div>
  <span class="flex-fill"></span>
  <button class="secondary-button" type="button" onclick={markAll} disabled={markingAll || !unread}>
    {markingAll ? '处理中…' : '全部标为已读'}
  </button>
</div>

{#if errorMessage}
  <p class="error-message" role="alert">{errorMessage}</p>
{/if}

{#if loading}
  <div class="state-box">正在加载通知…</div>
{:else if !visible.length}
  <div class="empty-panel">
    <strong>{filter === 'unread' ? '没有未读通知' : '暂无通知'}</strong>
    <p>任务分配给你、请你评审或有人评论时,这里会出现提醒。</p>
  </div>
{:else}
  <section class="panel">
    {#each visible as item (item.id)}
      {@const parts = summaryParts(item)}
      {@const meta = metaOf(item.type)}
      <button class="notification-row" class:unread={!item.read_at} type="button" disabled={openingId === item.id} onclick={() => open(item)}>
        <span class="glyph g-{item.type}" aria-hidden="true">{meta.glyph}</span>
        <span class="content">
          <span class="head-line">
            {#if parts.actor}<strong>{parts.actor}</strong>{/if}
            {#each parts.segments as segment, index}
              {segment}{#if index < parts.segments.length - 1}<code>{item.task_key}</code>{/if}
            {/each}
            <time>{timeLabel(item.created_at)}</time>
          </span>
          <span class="type-line">{meta.label} · {item.project_key}</span>
        </span>
      </button>
    {/each}
  </section>
  {#if hasMore}
    <div class="pager">
      <button class="secondary-button" type="button" disabled={appending} onclick={() => void load(page + 1, true)}>
        {appending ? '加载中…' : '加载更多'}
      </button>
    </div>
  {/if}
{/if}

<style>
  h1, p { margin: 0; }
  .page-head { margin-bottom: 18px; }
  .page-head h1 { font-size: 22px; font-weight: 600; line-height: 1.35; }
  .meta-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 16px; margin-top: 8px; font-size: 13px; color: var(--color-text-muted); }
  .sep { color: var(--color-border); }

  .toolbar { display: flex; align-items: center; gap: 8px; margin-bottom: 14px; }
  .toolbar .secondary-button { border: 0; }
  .flex-fill { flex: 1; }
  .segmented { display: inline-flex; gap: 2px; padding: 2px; border: 1px solid var(--color-border); border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .segmented button { padding: 4px 12px; border-radius: calc(var(--radius-md) - 2px); background: transparent; color: var(--color-text-muted); font-size: 12px; cursor: pointer; transition: background-color var(--transition-fast), color var(--transition-fast); }
  .segmented button:hover { color: var(--color-text-secondary); }
  .segmented button.active { background: var(--color-surface-raised); color: var(--color-text); font-weight: 500; box-shadow: 0 0 0 1px var(--color-border-weak); }

  .panel { background: var(--color-surface); border: 1px solid var(--color-border); border-radius: var(--radius-lg); overflow: hidden; }
  .notification-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    width: 100%;
    padding: 12px 14px;
    border: 0;
    border-top: 1px solid var(--color-border-weak);
    border-left: 2px solid transparent;
    background: transparent;
    color: var(--color-text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--transition-fast);
  }
  .notification-row:first-child { border-top: 0; }
  .notification-row:hover { background: var(--color-hover); }
  .notification-row.unread { background: color-mix(in srgb, var(--color-primary) 6%, transparent); border-left-color: var(--color-primary); }
  .notification-row.unread:hover { background: color-mix(in srgb, var(--color-primary) 10%, transparent); }
  .notification-row:disabled { cursor: wait; }
  .glyph {
    flex: none;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--color-hover);
    color: var(--color-text-muted);
    font-size: 12px;
  }
  .g-mentioned { background: color-mix(in srgb, var(--color-primary) 14%, transparent); color: var(--color-primary); font-weight: 600; }
  .g-review_requested { background: color-mix(in srgb, var(--color-success) 14%, transparent); color: var(--color-success); }
  .g-commented { background: color-mix(in srgb, #0f766e 14%, transparent); color: #0f766e; }
  .g-status_changed { background: color-mix(in srgb, var(--color-warning) 16%, transparent); color: var(--color-warning); }
  .g-assigned { background: color-mix(in srgb, #9333ea 12%, transparent); color: #9333ea; }

  .content { flex: 1; min-width: 0; }
  .head-line { display: flex; align-items: baseline; gap: 6px; min-width: 0; flex-wrap: wrap; }
  .head-line strong { font-weight: 500; flex: none; }
  .head-line code { font-family: var(--font-mono); font-size: 12px; color: var(--color-primary-strong); }
  .notification-row time { margin-left: auto; font-size: 12px; color: var(--color-text-muted); white-space: nowrap; flex: none; }
  .type-line { display: block; margin-top: 2px; color: var(--color-text-muted); font-size: 12px; }

  .error-message { margin: 0 0 14px; color: var(--color-danger); font-size: 13px; }
  .state-box { display: grid; place-items: center; min-height: 220px; color: var(--color-text-muted); }
  .empty-panel { display: grid; place-items: center; gap: 8px; min-height: 220px; border: 1px solid var(--color-border); border-radius: var(--radius-lg); color: var(--color-text-muted); }
  .empty-panel strong { color: var(--color-text-secondary); font-size: 14px; font-weight: 500; }
  .empty-panel p { font-size: 13px; }
  .pager { display: flex; justify-content: center; margin-top: 14px; }

  @media (max-width: 640px) {
    .notification-row time { margin-left: 0; width: 100%; }
  }
</style>
