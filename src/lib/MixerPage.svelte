<script lang="ts">
  import { store, instrumentLabel } from './store.svelte'
  import { type Track, type Person } from './types'

  const people = $derived(store.song.people ?? [])
  const ungrouped = $derived(store.song.tracks.filter((t) => !(t.personIds?.length)))

  function tracksOf(personId: string): Track[] {
    return store.song.tracks.filter((t) => (t.personIds ?? []).includes(personId))
  }

  function setVol(id: string, v: number) {
    const t = store.song.tracks.find((x) => x.id === id)
    if (t) t.volume = v
  }
  function setPan(id: string, v: number) {
    const t = store.song.tracks.find((x) => x.id === id)
    if (t) t.pan = v
  }
</script>

{#snippet strip(t: Track, shared: boolean)}
  <div class="strip" class:shared>
    <div class="name" title={instrumentLabel(t)}>
      <span class="n">{t.name}</span>
      <span class="i">{instrumentLabel(t)}{shared ? ' · compart.' : ''}</span>
    </div>

    <div class="fader">
      <input
        class="vol"
        type="range"
        min="-40"
        max="6"
        step="1"
        value={t.volume ?? -6}
        oninput={(e) => setVol(t.id, +(e.currentTarget as HTMLInputElement).value)}
        aria-label="Volume {t.name}"
      />
      <span class="db">{(t.volume ?? -6).toFixed(0)} dB</span>
    </div>

    <div class="pan">
      <input
        type="range"
        min="-1"
        max="1"
        step="0.1"
        value={t.pan ?? 0}
        oninput={(e) => setPan(t.id, +(e.currentTarget as HTMLInputElement).value)}
        aria-label="Pan {t.name}"
      />
      <span class="plabel">{(t.pan ?? 0) === 0 ? 'C' : (t.pan ?? 0) < 0 ? `E${Math.round(-(t.pan ?? 0) * 100)}` : `D${Math.round((t.pan ?? 0) * 100)}`}</span>
    </div>

    <div class="ms">
      <button class="mini" class:active={t.muted} aria-pressed={t.muted} title="Mudo" onclick={() => (t.muted = !t.muted)}>M</button>
      <button class="mini" class:active={t.solo} aria-pressed={t.solo} title="Solo" onclick={() => (t.solo = !t.solo)}>S</button>
    </div>
  </div>
{/snippet}

{#snippet personGroup(person: Person)}
  <section class="group" style:--pc={person.color ?? 'var(--accent)'}>
    <header class="group-head">
      <span class="swatch"></span>
      <span class="gname">{person.name}</span>
      <div class="bus">
        <input
          class="bus-vol"
          type="range"
          min="-40"
          max="6"
          step="1"
          value={person.volume ?? 0}
          oninput={(e) => (person.volume = +(e.currentTarget as HTMLInputElement).value)}
          aria-label="Volume da pessoa {person.name}"
        />
        <span class="db">{(person.volume ?? 0).toFixed(0)} dB</span>
        <button class="mini" class:active={person.muted} aria-pressed={person.muted ?? false} title="Mudo (pessoa)" onclick={() => (person.muted = !person.muted)}>M</button>
        <button class="mini" class:active={person.solo} aria-pressed={person.solo ?? false} title="Solo (pessoa)" onclick={() => (person.solo = !person.solo)}>S</button>
      </div>
    </header>
    <div class="group-strips">
      {#each tracksOf(person.id) as t (t.id)}
        {@render strip(t, (t.personIds?.length ?? 0) >= 2)}
      {:else}
        <p class="empty small">Sem instrumentos.</p>
      {/each}
    </div>
  </section>
{/snippet}

<div class="mixer-page">
  {#each people as person (person.id)}
    {@render personGroup(person)}
  {/each}

  {#if ungrouped.length}
    <section class="group ungrouped">
      <header class="group-head">
        <span class="gname">Sem pessoa</span>
      </header>
      <div class="group-strips">
        {#each ungrouped as t (t.id)}
          {@render strip(t, false)}
        {/each}
      </div>
    </section>
  {/if}

  {#if store.song.tracks.length === 0}
    <p class="empty">Sem faixas. Adicione uma linha no Editor.</p>
  {/if}
</div>

<style>
  .mixer-page {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding: 0.5rem 0 1.5rem;
  }

  .group {
    border: 1px solid var(--line);
    border-left: 4px solid var(--pc, var(--line-strong));
    border-radius: 10px;
    padding: 0.5rem 0.6rem 0.7rem;
    background: var(--paper);
  }

  .group.ungrouped {
    border-left-color: var(--line-strong);
  }

  .group-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
    flex-wrap: wrap;
  }

  .swatch {
    width: 0.9rem;
    height: 0.9rem;
    border-radius: 3px;
    background: var(--pc);
    border: 1px solid var(--line-strong);
  }

  .gname {
    font-weight: 800;
    font-size: 0.9rem;
    color: var(--ink);
  }

  .bus {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-left: 0.6rem;
    padding-left: 0.6rem;
    border-left: 1px solid var(--line);
  }

  .bus-vol {
    width: 8rem;
    accent-color: var(--pc);
  }

  .group-strips {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
    width: 6.5rem;
    padding: 0.75rem 0.5rem;
    background: var(--paper-raise);
    border: 1px solid var(--line);
    border-radius: 10px;
  }

  .strip.shared {
    border-style: dashed;
    border-color: var(--pc, var(--line-strong));
  }

  .name {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    min-height: 2.4rem;
  }

  .name .n {
    font-weight: 700;
    font-size: 0.78rem;
    color: var(--ink);
  }

  .name .i {
    font-size: 0.62rem;
    color: var(--ink-soft);
  }

  .fader {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    height: 9rem;
  }

  .vol {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 1.4rem;
    height: 8rem;
    accent-color: var(--accent);
  }

  .db {
    font-size: 0.62rem;
    font-weight: 700;
    color: var(--ink-soft);
  }

  .pan {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15rem;
  }

  .pan input {
    width: 5rem;
    accent-color: var(--accent-2);
  }

  .plabel {
    font-size: 0.62rem;
    font-weight: 700;
    color: var(--ink-soft);
  }

  .ms {
    display: flex;
    gap: 0.3rem;
  }

  .mini {
    font: inherit;
    font-size: 0.68rem;
    font-weight: 700;
    width: 1.7rem;
    height: 1.7rem;
    border: 1px solid var(--line-strong);
    background: var(--paper-raise);
    color: var(--ink-soft);
    border-radius: 5px;
    cursor: pointer;
  }

  .mini.active {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .empty {
    color: var(--ink-soft);
    font-size: 0.85rem;
  }

  .empty.small {
    font-size: 0.72rem;
  }
</style>
