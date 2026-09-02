<script lang="ts">
  // 添加项目成员的用户搜索选择框:输入姓名/账号,防抖搜索后弹出候选项。
  // 同名用户靠 账号 + 部门 标签区分;已在名单中的人会带标记。
  // 面板 position: fixed 定位,不会被父级 overflow 裁剪。
  import Avatar from '$lib/components/Avatar.svelte';
  import { listMemberCandidates } from '$lib/api/projects';
  import type { MemberCandidate } from '$lib/api/types';

  interface Props {
    projectKey: string;
    selected: MemberCandidate | null;
    onselect: (candidate: MemberCandidate | null) => void;
    placeholder?: string;
    ariaLabel?: string;
  }

  let { projectKey, selected, onselect, placeholder = '输入姓名或账号搜索', ariaLabel = '搜索用户' }: Props = $props();

  let input = $state<HTMLInputElement | null>(null);
  let panelNode = $state<HTMLElement | null>(null);
  let text = $state('');
  let results = $state<MemberCandidate[]>([]);
  let searching = $state(false);
  let searchError = $state('');
  let open = $state(false);
  let panelBox = $state<{ top: string; bottom: string; left: string; width: string; maxHeight: string } | null>(null);

  const label = (candidate: MemberCandidate) => `${candidate.display_name}（${candidate.account}）`;
  // 用户在已选中的文本上继续输入时不叫"父级重置",不能清空输入
  let clearedByTyping = $state(false);

  // 选中态由父级持有:选中时回填标签文本,父级重置(添加成功/点×)时清空
  $effect(() => {
    if (selected) {
      text = label(selected);
      results = [];
      open = false;
      clearedByTyping = false;
    } else if (!clearedByTyping) {
      text = '';
      results = [];
      open = false;
    }
  });

  // 防抖搜索;正在编辑已选中的文本时不搜
  $effect(() => {
    const query = text.trim();
    if (selected || !query) return;
    searching = true;
    const timer = setTimeout(async () => {
      try {
        results = (await listMemberCandidates(projectKey, query)).data.items;
        searchError = '';
        positionPanel();
        open = true;
      } catch (error) {
        searchError = error instanceof Error ? error.message : '搜索失败';
        results = [];
      } finally {
        searching = false;
      }
    }, 300);
    return () => clearTimeout(timer);
  });

  function positionPanel() {
    const rect = input?.getBoundingClientRect();
    if (!rect) return;
    const spaceBelow = window.innerHeight - rect.bottom;
    const upward = spaceBelow < 260 && rect.top > spaceBelow;
    panelBox = {
      top: upward ? '' : `${Math.round(rect.bottom + 6)}px`,
      bottom: upward ? `${Math.round(window.innerHeight - rect.top + 6)}px` : '',
      left: `${Math.round(rect.left)}px`,
      width: `${Math.max(Math.round(rect.width), 280)}px`,
      maxHeight: `${Math.min(upward ? rect.top - 12 : spaceBelow - 12, 300)}px`
    };
  }

  function onInput() {
    if (selected) {
      // 重新编辑即放弃已选中的人,但不清空输入
      clearedByTyping = true;
      onselect(null);
    }
    positionPanel();
    open = !!text.trim();
  }

  function choose(candidate: MemberCandidate) {
    onselect(candidate);
    open = false;
  }

  function clear() {
    onselect(null);
    input?.focus();
  }

  function onWindowPointerDown(event: MouseEvent) {
    if (!open) return;
    const target = event.target as Node;
    if (input?.contains(target) || panelNode?.contains(target)) return;
    open = false;
  }

  function onWindowKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && open) open = false;
  }

  // 滚动时关闭面板(fixed 定位不会跟随滚动,干脆收起)
  $effect(() => {
    if (!open) return;
    const close = () => (open = false);
    window.addEventListener('scroll', close, true);
    window.addEventListener('resize', close);
    return () => {
      window.removeEventListener('scroll', close, true);
      window.removeEventListener('resize', close);
    };
  });
</script>

<svelte:window onpointerdown={onWindowPointerDown} onkeydown={onWindowKeydown} />

<div class="user-search">
  <input
    bind:this={input}
    bind:value={text}
    {placeholder}
    aria-label={ariaLabel}
    type="search"
    oninput={onInput}
    onfocus={() => {
      if (results.length || searchError) {
        positionPanel();
        open = true;
      }
    }}
    onkeydown={(event) => {
      // 未选中具体的人前,Enter 不触发外层表单提交
      if (event.key === 'Enter' && !selected) event.preventDefault();
    }}
  />
  {#if selected}
    <button type="button" class="clear" aria-label="清除已选用户" onclick={clear}>×</button>
  {/if}
</div>

{#if open && panelBox && (searching || searchError || results.length || text.trim())}
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <div class="panel" bind:this={panelNode} role="listbox" aria-label="用户搜索结果" style={`top:${panelBox.top};bottom:${panelBox.bottom};left:${panelBox.left};width:${panelBox.width};max-height:${panelBox.maxHeight}`}>
    {#if searchError}
      <div class="hint bad">{searchError}</div>
    {:else if searching && !results.length}
      <div class="hint">搜索中…</div>
    {:else if !results.length}
      <div class="hint">没有匹配的用户</div>
    {:else}
      {#each results as candidate (candidate.user_id)}
        <button type="button" class="option" role="option" aria-selected={selected?.user_id === candidate.user_id} onclick={() => choose(candidate)}>
          <Avatar name={candidate.display_name} size={26} />
          <span class="who">
            <strong>{candidate.display_name}</strong>
            <small>@{candidate.account}</small>
          </span>
          {#if candidate.departments.length}
            <span class="tags">
              {#each candidate.departments as name (name)}<span class="tag">{name}</span>{/each}
            </span>
          {/if}
          {#if candidate.in_project}
            <span class="badge">已在项目中</span>
          {:else if candidate.via_department}
            <span class="badge">部门授权成员</span>
          {/if}
        </button>
      {/each}
    {/if}
  </div>
{/if}

<style>
  .user-search { position: relative; display: flex; align-items: center; width: 100%; min-width: 0; }
  .user-search input { flex: 1; min-width: 0; padding-right: 28px; }
  input[type='search']::-webkit-search-cancel-button { display: none; }

  .clear {
    position: absolute;
    right: 6px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--color-text-muted);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
  }
  .clear:hover { background: var(--color-hover); color: var(--color-text); }

  /* 面板:fixed 定位(z-index 高于 Modal 的 1000) */
  .panel {
    position: fixed;
    z-index: 1100;
    display: grid;
    gap: 2px;
    overflow-y: auto;
    padding: 6px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-popover);
  }

  .hint { padding: 10px; color: var(--color-text-muted); font-size: 13px; }
  .hint.bad { color: var(--color-danger); }

  .option {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .option:hover { background: var(--color-primary-soft); }

  .who { display: grid; gap: 1px; min-width: 0; }
  .who strong { font-weight: 500; }
  .who small { color: var(--color-text-muted); font-size: 12px; }

  .tags { display: flex; flex-wrap: wrap; gap: 4px; flex: 1; justify-content: flex-end; }
  .tag { padding: 1px 7px; border: 1px solid var(--color-border); border-radius: var(--radius-sm); color: var(--color-text-muted); font-size: 11px; white-space: nowrap; }

  .badge { flex: none; padding: 1px 7px; border-radius: var(--radius-sm); background: var(--color-primary-soft); color: var(--color-primary-strong); font-size: 11px; white-space: nowrap; }
</style>
