<script lang="ts">
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  import PageHeader from '$lib/components/PageHeader.svelte';
  import { ApiClientError } from '$lib/api/client';
  import { createProject, listDepartments } from '$lib/api/projects';
  import type { DepartmentView } from '$lib/api/types';

  let projectKey = $state(''); let name = $state(''); let description = $state(''); let departmentId = $state('');
  let departments = $state<DepartmentView[]>([]); let loadingDepartments = $state(true); let submitting = $state(false); let errorMessage = $state('');
  onMount(async () => { try { departments = (await listDepartments()).data.items; } catch (error) { errorMessage = error instanceof ApiClientError ? error.message : '部门加载失败'; } finally { loadingDepartments = false; } });
  async function submit(event: SubmitEvent) { event.preventDefault(); errorMessage = ''; if (!projectKey.trim() || !name.trim()) { errorMessage = '项目 Key 和项目名称不能为空'; return; } submitting = true; try { const project = (await createProject({ project_key: projectKey.trim(), name: name.trim(), description: description.trim() || undefined, primary_department_id: departmentId || undefined })).data; await goto(`/projects/${project.project_key}/board`); } catch (error) { errorMessage = error instanceof ApiClientError ? error.message : '项目创建失败'; } finally { submitting = false; } }
</script>
<PageHeader title="新建项目" eyebrow="Project" description="创建项目后添加多个负责人、成员和部门授权。" />
<form class="workspace-card project-form" onsubmit={submit}>
  <div class="form-intro"><strong>项目基本信息</strong><span>创建者会自动成为第一位项目负责人。</span></div>
  <label>项目 Key<input bind:value={projectKey} maxlength="32" placeholder="例如 PROJ-OPS" /></label>
  <label>项目名称<input bind:value={name} placeholder="请输入项目名称" /></label>
  <label>主属部门<select bind:value={departmentId} disabled={loadingDepartments}><option value="">暂不指定</option>{#each departments as department}<option value={department.id}>{department.name}（{department.code}）</option>{/each}</select></label>
  <label>描述<textarea bind:value={description} rows="5" placeholder="项目目标与边界"></textarea></label>
  {#if errorMessage}<p class="form-error" role="alert">{errorMessage}</p>{/if}
  <div class="form-actions"><a class="secondary-button" href="/projects">取消</a><button class="primary-button" type="submit" disabled={submitting}>{submitting ? '创建中…' : '创建项目'}</button></div>
</form>
<style>
  .project-form{display:grid;gap:16px;max-width:760px}.form-intro{display:grid;gap:4px;padding-bottom:5px;border-bottom:1px solid var(--color-border)}.form-intro span{color:var(--color-text-muted);font-size:13px}label{display:grid;gap:7px;font-weight:500}input,select,textarea{width:100%;border:1px solid var(--color-border);border-radius:var(--radius-md);padding:11px 12px;color:var(--color-text);background:var(--color-surface);resize:vertical}input:focus,select:focus,textarea:focus{border-color:var(--color-primary)}.form-error{margin:0;color:var(--color-danger);font-size:14px}.form-actions{display:flex;justify-content:flex-end;gap:10px;padding-top:4px}.form-actions button{border:0}.form-actions a{min-width:72px}
</style>
