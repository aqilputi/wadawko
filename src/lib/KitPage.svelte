<script lang="ts">
  import {
    store,
    paletteInstruments,
    addInstrument,
    updateInstrumentZone,
    renameInstrument,
    duplicateInstrument,
    deleteInstrument,
  } from './store.svelte'
  import { engine } from './audio'
  import { INSTRUMENTS, type InstrumentDef, type InstrumentId, type ZoneParams } from './types'

  const palette = $derived(paletteInstruments())
  const custom = $derived(palette.filter((d) => !d.builtin))
  const builtins = $derived(palette.filter((d) => d.builtin))

  let newBase: InstrumentId = $state('shime')

  const FILTERS = [
    { v: 'none', label: 'nenhum' },
    { v: 'lowpass', label: 'passa-baixa' },
    { v: 'highpass', label: 'passa-alta' },
    { v: 'bandpass', label: 'passa-banda' },
  ] as const

  function usedBy(defId: string): number {
    return store.song.tracks.filter((t) => (t.instrumentId ?? t.instrument) === defId).length
  }

  function play(def: InstrumentDef, zone: 'head' | 'rim') {
    engine.previewInstrument($state.snapshot(def), zone)
  }

  function setTone(def: InstrumentDef, zone: 'head' | 'rim', type: string) {
    const cur = def[zone]?.tone
    if (type === 'none') updateInstrumentZone(def.id, zone, { tone: undefined })
    else
      updateInstrumentZone(def.id, zone, {
        tone: { type: type as 'lowpass' | 'highpass' | 'bandpass', cutoff: cur?.cutoff ?? 1200, q: cur?.q ?? 1 },
      })
  }

  function setToneField(def: InstrumentDef, zone: 'head' | 'rim', field: 'cutoff' | 'q', value: number) {
    const cur = def[zone]?.tone
    if (!cur) return
    updateInstrumentZone(def.id, zone, { tone: { ...cur, [field]: value } })
  }

  function onDelete(def: InstrumentDef) {
    const n = usedBy(def.id)
    const warn = n > 0 ? ` ${n} linha(s) usam — voltarão ao som base.` : ''
    if (!confirm(`Excluir "${def.name}"?${warn}`)) return
    deleteInstrument(def.id)
  }
</script>

{#snippet zoneEditor(def: InstrumentDef, zone: 'head' | 'rim')}
  {@const p = (def[zone] ?? {}) as ZoneParams}
  <div class="zone">
    <div class="zone-head">
      <span class="zt">{zone === 'head' ? 'couro' : 'borda (fuchi)'}</span>
      <button class="play" title="Ouvir esta batida" onclick={() => play(def, zone)}>▶</button>
    </div>

    <label class="ctl">
      <span>pitch <b>{p.pitch ?? 0} st</b></span>
      <input type="range" min="-24" max="24" step="1" value={p.pitch ?? 0}
        oninput={(e) => updateInstrumentZone(def.id, zone, { pitch: +(e.currentTarget as HTMLInputElement).value })} />
    </label>

    <label class="ctl">
      <span>ganho <b>{p.gain ?? 0} dB</b></span>
      <input type="range" min="-12" max="12" step="1" value={p.gain ?? 0}
        oninput={(e) => updateInstrumentZone(def.id, zone, { gain: +(e.currentTarget as HTMLInputElement).value })} />
    </label>

    <label class="ctl">
      <span>decaimento <b>{Math.round((p.decay ?? 0.5) * 100)}%</b></span>
      <input type="range" min="0" max="1" step="0.05" value={p.decay ?? 0.5}
        oninput={(e) => updateInstrumentZone(def.id, zone, { decay: +(e.currentTarget as HTMLInputElement).value })} />
    </label>

    <label class="ctl">
      <span>overtones <b>{Math.round((p.overtones ?? 0) * 100)}%</b></span>
      <input type="range" min="0" max="1" step="0.05" value={p.overtones ?? 0}
        oninput={(e) => updateInstrumentZone(def.id, zone, { overtones: +(e.currentTarget as HTMLInputElement).value })} />
    </label>

    <label class="ctl">
      <span>timbre</span>
      <select value={p.tone?.type ?? 'none'} onchange={(e) => setTone(def, zone, (e.currentTarget as HTMLSelectElement).value)}>
        {#each FILTERS as f (f.v)}<option value={f.v}>{f.label}</option>{/each}
      </select>
    </label>

    {#if p.tone}
      <label class="ctl">
        <span>corte <b>{p.tone.cutoff} Hz</b></span>
        <input type="range" min="100" max="8000" step="50" value={p.tone.cutoff}
          oninput={(e) => setToneField(def, zone, 'cutoff', +(e.currentTarget as HTMLInputElement).value)} />
      </label>
      <label class="ctl">
        <span>ressonância <b>{p.tone.q.toFixed(1)}</b></span>
        <input type="range" min="0.1" max="12" step="0.1" value={p.tone.q}
          oninput={(e) => setToneField(def, zone, 'q', +(e.currentTarget as HTMLInputElement).value)} />
      </label>
    {/if}
  </div>
{/snippet}

<div class="kit-page">
  <div class="head">
    <div>
      <h2>Kit — instrumentos</h2>
      <p class="sub">Crie instrumentos a partir de um base e ajuste couro e borda separadamente. Os instrumentos aqui aparecem na seleção de linhas do editor. Excluir um não mata a linha — ela volta ao som base.</p>
    </div>
    <div class="new">
      <select bind:value={newBase} aria-label="Base do novo instrumento">
        {#each Object.entries(INSTRUMENTS) as [id, name] (id)}<option value={id}>{name}</option>{/each}
      </select>
      <button class="add" onclick={() => addInstrument(newBase)}>+ novo</button>
    </div>
  </div>

  {#if custom.length}
    <h3>Custom</h3>
    <div class="cards">
      {#each custom as def (def.id)}
        <section class="card">
          <header class="card-head">
            <input class="dname" value={def.name} oninput={(e) => renameInstrument(def.id, (e.currentTarget as HTMLInputElement).value)} spellcheck="false" />
            <span class="base">base: {INSTRUMENTS[def.base]}</span>
            {#if usedBy(def.id)}<span class="used">{usedBy(def.id)} linha(s)</span>{/if}
            <button class="mini" title="Duplicar" onclick={() => duplicateInstrument(def.id)}>⧉</button>
            <button class="mini danger" title="Excluir" onclick={() => onDelete(def)}>×</button>
          </header>
          <div class="zones">
            {@render zoneEditor(def, 'head')}
            {@render zoneEditor(def, 'rim')}
          </div>
        </section>
      {/each}
    </div>
  {/if}

  <h3>Prontos</h3>
  <div class="builtin-list">
    {#each builtins as def (def.id)}
      <div class="builtin">
        <span class="bn">{def.name}</span>
        <span class="base">{INSTRUMENTS[def.base]}</span>
        <button class="play" title="Ouvir couro" onclick={() => play(def, 'head')}>▶ couro</button>
        <button class="play" title="Ouvir borda" onclick={() => play(def, 'rim')}>▶ borda</button>
        <button class="mini" title="Duplicar para editar" onclick={() => duplicateInstrument(def.id)}>⧉ editar cópia</button>
      </div>
    {/each}
  </div>
</div>

<style>
  .kit-page {
    padding: 0.5rem 0 2rem;
    max-width: 62rem;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 800;
    color: var(--ink);
  }

  h3 {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--ink-soft);
    margin: 1rem 0 0.5rem;
  }

  .sub {
    margin: 0.25rem 0 0;
    font-size: 0.78rem;
    color: var(--ink-soft);
    max-width: 42rem;
  }

  .new {
    display: flex;
    gap: 0.4rem;
    flex: 0 0 auto;
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

  .add {
    font: inherit;
    font-size: 0.8rem;
    font-weight: 700;
    padding: 0.4rem 0.9rem;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .cards {
    display: flex;
    flex-wrap: wrap;
    gap: 0.9rem;
  }

  .card {
    flex: 1 1 26rem;
    min-width: 24rem;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--paper);
    padding: 0.6rem 0.7rem 0.7rem;
  }

  .card-head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .dname {
    font: inherit;
    font-weight: 800;
    font-size: 0.9rem;
    color: var(--ink);
    background: transparent;
    border: none;
    border-bottom: 1px dashed transparent;
    min-width: 6rem;
    flex: 1;
  }

  .dname:hover,
  .dname:focus-visible {
    border-bottom-color: var(--line-strong);
    outline: none;
  }

  .base {
    font-size: 0.68rem;
    color: var(--ink-soft);
  }

  .used {
    font-size: 0.62rem;
    font-weight: 700;
    color: var(--accent-2);
    border: 1px solid var(--accent-2);
    border-radius: 999px;
    padding: 0.05rem 0.4rem;
  }

  .mini {
    font: inherit;
    font-size: 0.72rem;
    padding: 0.2rem 0.5rem;
    border: 1px solid var(--line-strong);
    background: var(--paper-raise);
    color: var(--ink-soft);
    border-radius: 5px;
    cursor: pointer;
  }

  .mini.danger:hover {
    color: var(--accent);
    border-color: var(--accent);
  }

  .zones {
    display: flex;
    gap: 0.8rem;
  }

  .zone {
    flex: 1 1 0;
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 0.4rem 0.5rem 0.5rem;
    background: var(--paper-raise);
  }

  .zone-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.3rem;
  }

  .zt {
    font-size: 0.72rem;
    font-weight: 800;
    color: var(--ink);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .play {
    font: inherit;
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.2rem 0.55rem;
    border: 1px solid var(--accent);
    border-radius: 6px;
    background: var(--accent-wash);
    color: var(--accent);
    cursor: pointer;
  }

  .play:hover {
    background: var(--accent);
    color: #fff;
  }

  .ctl {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    margin-top: 0.35rem;
  }

  .ctl span {
    font-size: 0.68rem;
    color: var(--ink-soft);
  }

  .ctl b {
    color: var(--ink);
  }

  .ctl input[type='range'] {
    width: 100%;
    accent-color: var(--accent);
  }

  .builtin-list {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .builtin {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.4rem 0.6rem;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--paper);
  }

  .bn {
    font-weight: 700;
    font-size: 0.82rem;
    color: var(--ink);
    min-width: 9rem;
  }

  .builtin .play {
    font-size: 0.68rem;
  }

  .builtin .mini {
    margin-left: auto;
  }
</style>
