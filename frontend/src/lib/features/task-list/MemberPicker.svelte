<script lang="ts">
  import Avatar from '$lib/components/Avatar.svelte';
  import type { ProjectMember } from '$lib/api/types';

  interface Props {
    value: string | null;
    members: ProjectMember[];
    onchange: (value: string | null) => void;
    ariaLabel?: string;
    disabled?: boolean;
  }

  let { value, members, onchange, ariaLabel = '选择负责人', disabled = false }: Props = $props();

  const selected = $derived(members.find((member) => member.user_id === value) ?? null);
</script>

<div class="member-picker">
  {#if selected}<Avatar name={selected.display_name} />{/if}
  <select
    value={value ?? ''}
    {disabled}
    aria-label={ariaLabel}
    onchange={(event) => {
      const next = event.currentTarget.value;
      onchange(next === '' ? null : next);
    }}
  >
    <option value="">未分配</option>
    {#each members as member (member.user_id)}
      <option value={member.user_id}>{member.display_name}</option>
    {/each}
  </select>
</div>

<style>
  .member-picker {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .member-picker select {
    flex: 1;
    min-width: 0;
  }
</style>
