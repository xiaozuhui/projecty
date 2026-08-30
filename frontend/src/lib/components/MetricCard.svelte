<script lang="ts">
  let { label, value, hint } = $props<{ label: string; value: string; hint: string }>();
</script>
<article class="workspace-card metric-card"><span>{label}</span><strong>{value}</strong><small>{hint}</small></article>
<style>.metric-card{display:grid;gap:8px}.metric-card span,.metric-card small{color:var(--color-text-muted)}.metric-card strong{font-size:30px;letter-spacing:-0.04em}</style>
