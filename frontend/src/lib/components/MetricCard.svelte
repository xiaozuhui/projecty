<script lang="ts">
  let { label, value, hint } = $props<{ label: string; value: string; hint: string }>();
</script>

<article class="workspace-card metric-card"><span>{label}</span><strong>{value}</strong><small>{hint}</small></article>

<style>
  .metric-card { display: grid; gap: 6px; }
  .metric-card span { color: var(--color-text-muted); font-size: 12px; }
  .metric-card strong { font-size: 24px; font-weight: 500; letter-spacing: -0.01em; }
  .metric-card small { color: var(--color-text-muted); font-size: 12px; }
</style>
