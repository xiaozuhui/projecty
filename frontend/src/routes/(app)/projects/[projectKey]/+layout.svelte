<script lang="ts">
  import { page } from '$app/state';

  let { children } = $props();

  // 链接必须用绝对路径:相对 href 会按当前 URL 解析,
  // /projects/KEY + ./board 会被解析成 /projects/board(KEY 段被当作文件名丢掉)。
  const projectKey = $derived(decodeURIComponent(page.params.projectKey ?? ''));
  const projectHref = $derived(`/projects/${encodeURIComponent(projectKey)}`);
  const tabs = $derived([
    { label: '概览', href: projectHref },
    { label: '看板', href: `${projectHref}/board` },
    { label: '列表', href: `${projectHref}/list` },
    { label: '时间线', href: `${projectHref}/timeline` },
    { label: '日历', href: `${projectHref}/calendar` },
    { label: '子任务', href: `${projectHref}/subtasks` },
    { label: '里程碑', href: `${projectHref}/milestones` },
    { label: '成员', href: `${projectHref}/members` },
    { label: '日志', href: `${projectHref}/logs` },
    { label: '设置', href: `${projectHref}/settings` },
  ]);

  const pathname = $derived(page.url.pathname.replace(/\/+$/, ''));
  const isActive = (href: string) => pathname === href.replace(/\/+$/, '');
</script>

<div class="project-layout">
  <nav class="breadcrumb" aria-label="面包屑">
    <a href="/projects">项目</a>
    <span class="crumb-sep">/</span>
    <a href={projectHref}>{projectKey}</a>
  </nav>
  <nav class="project-tabs" aria-label="项目导航">
    {#each tabs as tab}
      <a href={tab.href} class:active={isActive(tab.href)} aria-current={isActive(tab.href) ? 'page' : undefined}>{tab.label}</a>
    {/each}
  </nav>
  {@render children()}
</div>

<style>
  .breadcrumb { display: flex; align-items: center; gap: 6px; margin-bottom: 8px; font-size: 13px; }
  .breadcrumb a { color: var(--color-text-muted); }
  .breadcrumb a:hover { color: var(--color-text); }
  .crumb-sep { color: var(--color-border-strong); }
  .project-tabs { display: flex; gap: 2px; margin-bottom: 16px; overflow-x: auto; border-bottom: 1px solid var(--color-border); }
  .project-tabs a { flex: 0 0 auto; padding: 8px 10px; margin-bottom: -1px; border-bottom: 2px solid transparent; color: var(--color-text-muted); font-size: 13px; transition: color var(--transition-fast), border-color var(--transition-fast); }
  .project-tabs a:hover { color: var(--color-text); }
  .project-tabs a.active { color: var(--color-text); border-bottom-color: var(--color-primary); }
</style>
