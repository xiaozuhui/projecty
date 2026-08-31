<script lang="ts">
  import { onMount } from 'svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { listProjects } from '$lib/api/projects';
  import type { ProjectView } from '$lib/api/types';

  let projects = $state<ProjectView[]>([]);
  let page = $state(1);
  let hasMore = $state(false);
  let loading = $state(true);
  let errorMessage = $state('');

  async function loadProjects(targetPage = page) {
    loading = true; errorMessage = '';
    try { const response = await listProjects(targetPage); projects = response.data.items; page = response.data.page; hasMore = response.data.has_more; }
    catch (error) { errorMessage = error instanceof ApiClientError ? error.message : '项目加载失败'; }
    finally { loading = false; }
  }
  onMount(() => { void loadProjects(); });
</script>

<PageHeader title="项目" eyebrow="Projects" description="项目可有多个负责人，成员与部门授权可以交叉。" actionHref="/projects/new" actionLabel="新建项目" />
<section class="workspace-card">
  <div class="section-heading"><div><h2>项目空间</h2><p>按最近更新时间查看你有权限访问的项目。</p></div><span class="count-label">第 {page} 页</span></div>
  {#if loading}<div class="state-box">正在加载项目…</div>
  {:else if errorMessage}<div class="state-box error-state"><strong>{errorMessage}</strong><button class="secondary-button" type="button" onclick={() => loadProjects()}>重新加载</button></div>
  {:else if projects.length === 0}<div class="state-box"><strong>还没有可访问的项目</strong><p>可以先创建一个项目，或者联系项目负责人添加成员。</p></div>
  {:else}<div class="project-list">{#each projects as project}<a class="project-card" href={`/projects/${project.project_key}`}><div class="project-card-top"><strong>{project.project_key}</strong>{#if project.archived_at}<span class="archived-label">已归档</span>{/if}</div><h3>{project.name}</h3><p>{project.description || '暂未填写项目描述'}</p><div class="project-meta"><span>任务编号 {project.task_number_seed}</span><span>更新于 {new Date(project.updated_at).toLocaleDateString('zh-CN')}</span></div></a>{/each}</div>{/if}
  <div class="pager"><button class="secondary-button" type="button" disabled={loading || page <= 1} onclick={() => loadProjects(page - 1)}>上一页</button><button class="secondary-button" type="button" disabled={loading || !hasMore} onclick={() => loadProjects(page + 1)}>下一页</button></div>
</section>
<style>
  h2,h3,p{margin:0}.section-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:16px;margin-bottom:16px}.section-heading h2{font-size:18px}.section-heading p{margin-top:5px;color:var(--color-text-muted);font-size:13px}.count-label{color:var(--color-text-muted);font-size:13px}.project-list{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:14px}.project-card{display:grid;gap:10px;min-width:0;padding:17px;border:1px solid var(--color-border);border-radius:var(--radius-md);transition:transform .18s ease,box-shadow .18s ease,border-color .18s ease}.project-card:hover{border-color:#b9c9f8;transform:translateY(-2px)}.project-card-top{display:flex;align-items:center;justify-content:space-between;gap:8px}.project-card-top strong{color:var(--color-primary-strong);font-family:var(--font-mono);font-size:13px}.project-card h3{font-size:18px}.project-card p{min-height:42px;color:var(--color-text-muted);font-size:13px;line-height:1.6}.project-meta{display:flex;justify-content:space-between;gap:8px;padding-top:10px;border-top:1px solid var(--color-border);color:var(--color-text-muted);font-size:12px}.archived-label{padding:3px 8px;border-radius:999px;color:var(--color-warning);background:var(--color-surface)6e8;font-size:12px}.state-box{display:grid;place-items:center;gap:8px;min-height:180px;color:var(--color-text-muted);text-align:center}.state-box p{font-size:13px}.error-state{color:var(--color-danger)}.pager{display:flex;justify-content:flex-end;gap:8px;margin-top:18px}.secondary-button:disabled{cursor:not-allowed;opacity:.45}@media(max-width:1100px){.project-list{grid-template-columns:repeat(2,minmax(0,1fr))}}@media(max-width:640px){.project-list{grid-template-columns:1fr}.section-heading{display:block}.count-label{display:block;margin-top:8px}.project-meta{display:grid}}
</style>
