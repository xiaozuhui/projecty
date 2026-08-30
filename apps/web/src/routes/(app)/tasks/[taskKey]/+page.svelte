<script lang="ts">import PageHeader from '$lib/components/PageHeader.svelte';</script>
<PageHeader title="任务详情" eyebrow="Task" description="独立任务详情页包含描述、父任务/子任务、评论和操作日志时间线。" />
<div class="task-detail"><section class="workspace-card"><h2>任务内容</h2><p>状态、优先级、负责人、里程碑、截止日期。</p></section><aside class="workspace-card"><h2>操作日志</h2><p>状态修改与逻辑删除将同事务记录。</p></aside></div><style>.task-detail{display:grid;grid-template-columns:minmax(0,1fr) 360px;gap:18px}@media(max-width:1024px){.task-detail{grid-template-columns:1fr}}</style>
