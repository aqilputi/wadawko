<script lang="ts">
  import { store } from './store.svelte'
  let { open = true }: { open?: boolean } = $props()

  const pages = [
    { id: 'editor', icon: '📝', label: 'Editor', active: true },
    { id: 'people', icon: '👥', label: 'Pessoas', active: true },
    { id: 'mixer', icon: '🎚', label: 'Mixer', active: true },
    { id: 'kit', icon: '🥁', label: 'Kit', active: true },
    { id: 'settings', icon: '⚙', label: 'Config', active: false },
  ] as const
</script>

<nav class="rail" class:collapsed={!open} aria-label="Páginas">
  {#each pages as p (p.id)}
    <button
      class="item"
      class:on={p.active && store.page === p.id}
      disabled={!p.active}
      title={p.active ? p.label : `${p.label} (em breve)`}
      onclick={() => p.active && (store.page = p.id as 'editor' | 'mixer' | 'people' | 'kit')}
    >
      <span class="icon">{p.icon}</span>
      {#if open}<span class="label">{p.label}</span>{/if}
    </button>
  {/each}
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.6rem 0.4rem;
    width: 200px;
    background: var(--paper);
    border-right: 1px solid var(--line);
    transition: width 140ms ease;
  }

  .rail.collapsed {
    width: 52px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--ink-soft);
    font: inherit;
    font-size: 0.85rem;
    font-weight: 700;
    cursor: pointer;
    text-align: left;
    white-space: nowrap;
  }

  .item .icon {
    font-size: 1.1rem;
    line-height: 1;
    width: 1.3rem;
    text-align: center;
  }

  .item:hover:not(:disabled):not(.on) {
    background: var(--paper-raise);
    color: var(--ink);
  }

  .item.on {
    background: var(--accent-wash);
    color: var(--accent);
    border-color: var(--accent);
  }

  .item:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
