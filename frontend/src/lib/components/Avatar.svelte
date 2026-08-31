<script lang="ts">
  // 首字头像:单色底 + 1px 边框,替代原渐变方块。
  let { name, size = 20 }: { name: string; size?: number } = $props();
</script>

<span class="avatar" style={`width:${size}px;height:${size}px;font-size:${Math.max(10, Math.round(size * 0.42))}px`} aria-hidden="true">{name.slice(0, 1)}</span>

<style>
  .avatar { display: inline-grid; place-items: center; flex: none; border-radius: 50%; border: 1px solid var(--color-border); background: var(--color-hover); color: var(--color-text-secondary); font-weight: 500; overflow: hidden; }
</style>
