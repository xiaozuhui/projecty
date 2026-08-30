<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { listProjectLogs, downloadProjectLogs } from '$lib/api/audit';
  import { ApiClientError } from '$lib/api/client';
  import type { OperationLog } from '$lib/api/types';

  const projectKey = $derived(String(page.params.projectKey ?? ''));
  let logs = $state<OperationLog[]>([]);
  let loading = $state(true);
  let errorMessage = $state('');
  let exporting = $state(false);

  onMount(async () => {
    try {
      logs = (await listProjectLogs(projectKey, 1, 100)).data.items;
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '日志加载失败';
    } finally {
      loading = false;
    }
  });

  async function exportLogs() {
    exporting = true;
    errorMessage = '';
    try {
      const blob = await downloadProjectLogs(projectKey);
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `${projectKey}-operation-logs.csv`;
      anchor.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      errorMessage = error instanceof ApiClientError ? error.message : '导出失败';
    } finally {
      exporting = false;
    }
  }
</script>

<PageHeader
  title="操作日志"
  eyebrow="Audit"
  description="任务新增、删除、状态流转和权限变更都保留原始操作记录。"
/>

<div class="action-row">
  <button class="secondary-button" type="button" onclick={exportLogs} disabled={exporting}>
    {exporting ? '导出中…' : '下载项目日志'}
  </button>
</div>

{#if errorMessage}
  <section class="workspace-card error-state">{errorMessage}</section>
{/if}

{#if loading}
  <section class="workspace-card state-box">正在加载日志…</section>
{:else}
  <section class="workspace-card table-card">
    <div class="log-table">
      <div class="log-head">
        <span>时间</span>
        <span>模块 / 动作</span>
        <span>摘要</span>
        <span>目标</span>
      </div>
      {#each logs as log}
        <div class="log-row">
          <time>{new Date(log.created_at).toLocaleString('zh-CN')}</time>
          <div>
            <strong>{log.module}</strong>
            <small>{log.action}</small>
          </div>
          <span>{log.summary}</span>
          <code>{log.target_type} · {log.target_id || '-'}</code>
        </div>
      {:else}
        <div class="empty-inline">还没有操作日志。</div>
      {/each}
    </div>
  </section>
{/if}

<style>
  .action-row {
    display: flex;
    justify-content: flex-end;
    margin: -6px 0 16px;
  }

  .log-table {
    overflow: auto;
  }

  .log-head,
  .log-row {
    display: grid;
    grid-template-columns: 180px 160px minmax(260px, 1fr) 220px;
    gap: 14px;
    align-items: center;
    min-width: 820px;
  }

  .log-head {
    padding: 10px 0;
    color: var(--color-text-muted);
    font-size: 12px;
    font-weight: 800;
  }

  .log-row {
    padding: 14px 0;
    border-top: 1px solid var(--color-border);
    font-size: 13px;
  }

  .log-row time,
  .log-row small {
    color: var(--color-text-muted);
  }

  .log-row div {
    display: grid;
    gap: 4px;
  }

  .log-row code {
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .error-state {
    margin-bottom: 16px;
    color: var(--color-danger);
  }

  .state-box {
    color: var(--color-text-muted);
    text-align: center;
  }
</style>
