<script lang="ts">
  export interface Command {
    label: string
    hint?: string
    run: () => void
  }

  let { open = false, commands = [], onClose }: { open?: boolean; commands?: Command[]; onClose: () => void } = $props()

  let query = $state('')
  let idx = $state(0)
  let inputEl: HTMLInputElement | undefined = $state()

  const filtered = $derived(commands.filter((c) => c.label.toLowerCase().includes(query.trim().toLowerCase())))

  $effect(() => {
    if (open) {
      query = ''
      idx = 0
      queueMicrotask(() => inputEl?.focus())
    }
  })

  function run(c: Command | undefined) {
    if (!c) return
    onClose()
    c.run()
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      idx = Math.min(filtered.length - 1, idx + 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      idx = Math.max(0, idx - 1)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      run(filtered[idx])
    } else if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    } else {
      idx = 0
    }
  }
</script>

{#if open}
  <div
    class="backdrop"
    role="presentation"
    onclick={(e) => e.target === e.currentTarget && onClose()}
    onkeydown={(e) => e.key === 'Escape' && onClose()}
  >
    <div class="palette" role="dialog" aria-label="Paleta de comandos" tabindex="-1">
      <input bind:this={inputEl} bind:value={query} onkeydown={onKey} placeholder="Digite um comando…" spellcheck="false" />
      <ul role="listbox" aria-label="Comandos">
        {#each filtered as c, i (c.label)}
          <li role="option" aria-selected={i === idx} class:on={i === idx} onmousedown={() => run(c)} onmouseenter={() => (idx = i)}>
            <span class="lbl">{c.label}</span>
            {#if c.hint}<span class="k">{c.hint}</span>{/if}
          </li>
        {/each}
        {#if !filtered.length}
          <li class="empty" role="option" aria-selected="false">nenhum comando</li>
        {/if}
      </ul>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
  }

  .palette {
    width: min(90vw, 34rem);
    background: var(--paper);
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
    overflow: hidden;
  }

  input {
    width: 100%;
    font: inherit;
    font-size: 1rem;
    padding: 0.85rem 1rem;
    border: none;
    border-bottom: 1px solid var(--line);
    background: transparent;
    color: var(--ink);
    outline: none;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0.3rem;
    max-height: 50vh;
    overflow-y: auto;
  }

  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0.7rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.88rem;
    color: var(--ink);
  }

  li.on {
    background: var(--accent-wash);
    color: var(--accent);
  }

  li .k {
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--ink-faint);
  }

  li.empty {
    color: var(--ink-faint);
    cursor: default;
    justify-content: center;
  }
</style>
