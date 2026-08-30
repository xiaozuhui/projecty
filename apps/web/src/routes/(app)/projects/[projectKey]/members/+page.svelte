<script lang="ts">import PageHeader from '$lib/components/PageHeader.svelte';</script>
<PageHeader title="成员与部门授权" eyebrow="Members" description="显式成员、多个项目负责人和部门 member/viewer 授权。" />
<section class="workspace-card"><h2>权限原则</h2><p>最终权限=max(超级管理员、项目直接角色、部门层级授权)。</p></section>
