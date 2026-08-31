<script lang="ts">
  let {
    title,
    eyebrow,
    crumbs,
    description,
    actionHref,
    actionLabel,
  } = $props<{
    title: string;
    eyebrow?: string;
    crumbs?: { label: string; href?: string }[];
    description?: string;
    actionHref?: string;
    actionLabel?: string;
  }>();
</script>

<header class="page-title">
  <div>
    {#if crumbs && crumbs.length > 0}
      <nav class="breadcrumb" aria-label="面包屑">
        {#each crumbs as crumb, index}
          {#if index > 0}<span class="crumb-sep">/</span>{/if}
          {#if crumb.href && index < crumbs.length - 1}
            <a href={crumb.href}>{crumb.label}</a>
          {:else}
            <span class="crumb-current" aria-current="page">{crumb.label}</span>
          {/if}
        {/each}
      </nav>
    {:else if eyebrow}
      <span class="status-pill">{eyebrow}</span>
    {/if}
    <h1>{title}</h1>
    {#if description}<p>{description}</p>{/if}
  </div>
  {#if actionHref && actionLabel}<a class="primary-button" href={actionHref}>{actionLabel}</a>{/if}
</header>
