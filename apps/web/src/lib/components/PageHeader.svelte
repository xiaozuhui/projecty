<script lang="ts">
  let { title, eyebrow, description, actionHref, actionLabel } = $props<{ title: string; eyebrow?: string; description?: string; actionHref?: string; actionLabel?: string }>();
</script>
<header class="page-title">
  <div>
    {#if eyebrow}<span class="status-pill">{eyebrow}</span>{/if}
    <h1>{title}</h1>
    {#if description}<p>{description}</p>{/if}
  </div>
  {#if actionHref && actionLabel}<a class="primary-button" href={actionHref}>{actionLabel}</a>{/if}
</header>
