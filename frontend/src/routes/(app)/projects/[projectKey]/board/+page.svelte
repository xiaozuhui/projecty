<script lang="ts">
  import { page } from '$app/state';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import Board from '$lib/features/board/Board.svelte';

  const projectKey = $derived(String(page.params.projectKey ?? ''));
</script>

<PageHeader
  title="任务看板"
  eyebrow="Board"
  description="拖拽卡片跨列改状态、列内换顺序,列底回车快速建任务。"
/>
<Board {projectKey} />
