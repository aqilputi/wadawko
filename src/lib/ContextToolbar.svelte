<script lang="ts">
  import { store, appendSection, setSubdivision, addTrack, paletteInstruments } from './store.svelte'
  import { cellsPerMeasure } from './types'

  const SUBDIVISIONS = [
    { value: 2, name: 'colcheia' },
    { value: 4, name: 'semicolcheia' },
    { value: 8, name: 'fusa' },
  ]
  const currentSub = $derived(store.song.measures[0]?.subdivision ?? 2)
  const palette = $derived(paletteInstruments())
  let newInstrument = $state('chu')

  const people = $derived(store.song.people ?? [])
  const activeColor = $derived(people.find((p) => p.id === store.activePersonId)?.color ?? null)
</script>

<div class="toolbar" role="toolbar" aria-label="Ferramentas do editor">
  <!-- ferramentas -->
  <div class="group seg" role="group" aria-label="Modo">
    <button class:on={store.mode === 'edit'} title="Digitar nas células" onclick={() => (store.mode = 'edit')}>✎ editar</button>
    <button class:on={store.mode === 'select'} title="Selecionar / mover / copiar" onclick={() => (store.mode = 'select')}>⬚ selecionar</button>
  </div>

  <span class="divider"></span>

  <!-- estrutura -->
  <div class="group seg" role="group" aria-label="Subdivisão">
    {#each SUBDIVISIONS as sub (sub.value)}
      <button class:on={currentSub === sub.value} title="{sub.value} células por tempo" onclick={() => setSubdivision(sub.value)}>{sub.name}</button>
    {/each}
  </div>

  <button class="solo" title="Adicionar seção no fim" onclick={() => appendSection()}>+ seção</button>

  <span class="divider"></span>

  <!-- faixas -->
  <div class="group" role="group" aria-label="Faixas">
    <select bind:value={newInstrument} aria-label="Instrumento da nova linha">
      {#each palette as def (def.id)}
        <option value={def.id}>{def.name}</option>
      {/each}
    </select>
    <button title="Adicionar linha (instrumento)" onclick={() => addTrack(newInstrument)}>+ linha</button>
  </div>

  {#if people.length}
    <span class="divider"></span>
    <div class="group caneta" role="group" aria-label="Caneta de pessoa">
      <span class="cl" title="Notas digitadas em instrumento compartilhado ganham esta pessoa">✎ caneta</span>
      {#if activeColor}<span class="dot" style:background={activeColor}></span>{/if}
      <select
        aria-label="Pessoa ativa (caneta)"
        value={store.activePersonId ?? ''}
        onchange={(e) => (store.activePersonId = (e.currentTarget as HTMLSelectElement).value || null)}
      >
        <option value="">— sem dono —</option>
        {#each people as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </div>
  {/if}

  <span class="divider"></span>

  <div class="group" role="group" aria-label="Zoom">
    <button title="Menos zoom" onclick={() => (store.zoom = Math.max(0.5, +(store.zoom - 0.25).toFixed(2)))}>🔍−</button>
    <span class="zoomval">{Math.round(store.zoom * 100)}%</span>
    <button title="Mais zoom" onclick={() => (store.zoom = Math.min(2, +(store.zoom + 0.25).toFixed(2)))}>🔍+</button>
  </div>

  <span class="spacer"></span>

  {#if store.mode === 'select'}
    <span class="hint">arraste = move · Shift = seleciona · Alt = clona · Ctrl+C/X/V · Ctrl+Z · Del</span>
  {/if}
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    padding: 0.4rem 0.5rem;
    margin-bottom: 0.9rem;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: 10px;
  }

  .group {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .divider {
    width: 1px;
    align-self: stretch;
    background: var(--line);
    margin: 0.1rem 0.15rem;
  }

  .spacer {
    flex: 1;
  }

  button {
    font: inherit;
    font-size: 0.78rem;
    font-weight: 700;
    padding: 0.35rem 0.7rem;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--paper-raise);
    color: var(--ink-soft);
    cursor: pointer;
  }

  button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .solo {
    color: var(--ink);
  }

  /* grupos segmentados: botões colados */
  .seg {
    gap: 0;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    overflow: hidden;
  }

  .seg button {
    border: none;
    border-radius: 0;
    border-right: 1px solid var(--line-strong);
  }

  .seg button:last-child {
    border-right: none;
  }

  select {
    font: inherit;
    font-size: 0.78rem;
    padding: 0.35rem 0.4rem;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--paper-raise);
    color: var(--ink);
  }

  .hint {
    font-size: 0.7rem;
    color: var(--ink-soft);
  }

  .zoomval {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--ink-soft);
    min-width: 2.6rem;
    text-align: center;
  }

  .caneta .cl {
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--ink-soft);
  }

  .caneta .dot {
    width: 0.8rem;
    height: 0.8rem;
    border-radius: 50%;
    border: 1px solid var(--line-strong);
  }
</style>
