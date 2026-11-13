<script lang="ts">
  import Grid from './lib/Grid.svelte'
  import Score from './lib/Score.svelte'
  import Lane from './lib/Lane.svelte'
  import NavRail from './lib/NavRail.svelte'
  import RightPanel from './lib/RightPanel.svelte'
  import StatusBar from './lib/StatusBar.svelte'
  import ContextToolbar from './lib/ContextToolbar.svelte'
  import MixerPage from './lib/MixerPage.svelte'
  import PeoplePanel from './lib/PeoplePanel.svelte'
  import KitPage from './lib/KitPage.svelte'
  import CommandPalette, { type Command } from './lib/CommandPalette.svelte'
  import { store, save, importSong, selectedRegion, addMeasure, appendSection, addTrack, newProject, resolveInstrument } from './lib/store.svelte'
  import { engine, playFue } from './lib/audio'
  import { PALETTE_GROUPS, PHRASES, VOCABULARY, displaySyllable, type KnownPhrase } from './lib/vocabulary'
  import { parseWords } from './lib/phrase'
  import { cellsPerMeasure } from './lib/types'

  let fileInput: HTMLInputElement
  let gridRef: Grid | undefined = $state()
  let importError: string | null = $state(null)
  let view: 'grade' | 'partitura' | 'esteira' = $state('grade')
  let navOpen = $state(true)
  let cmdOpen = $state(false)

  // tema (light/dark) — persiste em wadawko:theme; default segue o SO
  let theme: 'light' | 'dark' = $state(
    (localStorage.getItem('wadawko:theme') as 'light' | 'dark') ||
      (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'),
  )
  $effect(() => {
    document.documentElement.setAttribute('data-theme', theme)
    localStorage.setItem('wadawko:theme', theme)
  })

  // engine resolve a InstrumentDef de cada linha pelo kit da música (lê o store ao vivo)
  engine.setInstrumentResolver((t) => resolveInstrument(t))

  const commands = $derived<Command[]>([
    { label: store.playing ? '■ Parar' : '▶ Tocar', run: togglePlay },
    { label: `Loop: ${store.loop ? 'desligar' : 'ligar'}`, run: () => (store.loop = !store.loop) },
    { label: `Metrônomo: ${store.metronome ? 'desligar' : 'ligar'}`, run: () => (store.metronome = !store.metronome) },
    { label: `Count-in: ${store.countIn ? 'desligar' : 'ligar'}`, run: () => (store.countIn = !store.countIn) },
    { label: `Treino (rampa de tempo): ${store.trainerOn ? 'desligar' : 'ligar'}`, run: () => (store.trainerOn = !store.trainerOn) },
    { label: `Tema: ${theme === 'dark' ? 'claro' : 'escuro'}`, run: () => (theme = theme === 'dark' ? 'light' : 'dark') },
    { label: 'Ver: grade', run: () => { store.page = 'editor'; view = 'grade' } },
    { label: 'Ver: partitura', run: () => { store.page = 'editor'; view = 'partitura' } },
    { label: 'Ver: esteira', run: () => { store.page = 'editor'; view = 'esteira' } },
    { label: 'Ir para Mixer', run: () => (store.page = 'mixer') },
    { label: 'Ir para Pessoas', run: () => (store.page = 'people') },
    { label: 'Ir para Kit', run: () => (store.page = 'kit') },
    { label: 'Modo: editar', run: () => (store.mode = 'edit') },
    { label: 'Modo: selecionar', run: () => (store.mode = 'select') },
    { label: '+ Compasso (seção selecionada)', run: () => addMeasure() },
    { label: '+ Seção', run: () => appendSection() },
    { label: '+ Linha (Chū-daiko)', run: () => addTrack('chu') },
    { label: 'Nova música', run: () => newProject() },
  ])

  let phraseFeedback: string | null = $state(null)

  function onChipClick(syl: string) {
    store.selectedSyl = syl
    gridRef?.writeToFocused(syl)
  }

  function onPhraseClick(known: KnownPhrase) {
    clearTimeout(hoverTimer)
    engine.stopPhraseLoop()
    const written = gridRef?.writeToFocused(known.phrase, !known.perCell) ?? false
    phraseFeedback = written ? null : 'clique numa célula da grade primeiro, depois na frase'
  }

  let hoverTimer: ReturnType<typeof setTimeout> | undefined

  /** linha da célula focada (preview soa como o instrumento em edição) */
  function focusedTrack() {
    const active = document.activeElement
    const trackId = active instanceof HTMLInputElement ? active.dataset.track : undefined
    return store.song.tracks.find((t) => t.id === trackId) ?? store.song.tracks[0]
  }

  function onPhraseHover(known: KnownPhrase) {
    clearTimeout(hoverTimer)
    if (store.playing) return
    hoverTimer = setTimeout(() => {
      if (store.playing) return
      const words = parseWords(known.phrase)
      const track = focusedTrack()
      if (!words || !track) return
      const beatSec = 60 / store.song.tempo
      const subdivision = store.song.measures[0]?.subdivision ?? 2
      const unitSec = known.perCell ? beatSec / subdivision : beatSec
      engine.startPhraseLoop($state.snapshot(resolveInstrument(track)), words, unitSec, store.song.swing ?? 0)
    }, 2000)
  }

  function onPhraseLeave() {
    clearTimeout(hoverTimer)
    engine.stopPhraseLoop()
  }

  function onCursor(idx: number) {
    store.cursor = idx
  }

  function measureStartCell(mi: number): number {
    let acc = 0
    for (let i = 0; i < mi && i < store.song.measures.length; i++) acc += cellsPerMeasure(store.song.measures[i])
    return acc
  }

  function stopPlayback() {
    engine.stop()
    store.playing = false
    store.cursor = -1
    store.playbackBpm = store.song.tempo
    engine.setBpm(store.song.tempo)
  }

  async function togglePlay() {
    if (store.playing) {
      stopPlayback()
    } else {
      store.playing = true
      const region = selectedRegion()
      const start = region ? measureStartCell(region.startBar) : store.startCell
      await engine.play($state.snapshot(store.song), store.loop, onCursor, start, region, stopPlayback, {
        countIn: store.countIn,
        trainer: store.trainerOn ? { startPct: store.trainerStartPct, step: store.trainerStep } : null,
        onBpm: (b) => (store.playbackBpm = b),
      })
    }
  }

  // UMA serialização por mudança, compartilhada por autosave e histórico
  // (antes cada efeito fazia o seu JSON.stringify da música inteira)
  const songJson = $derived(JSON.stringify(store.song))

  // autosave (debounce)
  let saveTimer: ReturnType<typeof setTimeout>
  $effect(() => {
    void songJson
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => save(songJson), 400)
  })

  // histórico (undo/redo) — snapshot da música a cada mudança
  let past: string[] = []
  let future: string[] = []
  let lastSnap = JSON.stringify(store.song)
  let lastEpoch = store.projectEpoch
  let suppressHistory = false

  $effect(() => {
    const epoch = store.projectEpoch
    const cur = songJson
    if (epoch !== lastEpoch) {
      // trocou de projeto: zera o histórico
      lastEpoch = epoch
      past = []
      future = []
      lastSnap = cur
      return
    }
    if (cur === lastSnap) return
    if (suppressHistory) {
      suppressHistory = false
      lastSnap = cur
      return
    }
    past.push(lastSnap)
    if (past.length > 100) past.shift()
    future = []
    lastSnap = cur
  })

  function undo() {
    if (!past.length) return
    future.push(JSON.stringify(store.song))
    suppressHistory = true
    store.song = JSON.parse(past.pop()!)
  }

  function redo() {
    if (!future.length) return
    past.push(JSON.stringify(store.song))
    suppressHistory = true
    store.song = JSON.parse(future.pop()!)
  }

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey)) return
      if (e.key === 'k') {
        e.preventDefault()
        cmdOpen = !cmdOpen
      } else if (e.key === 'z' && !e.shiftKey) {
        e.preventDefault()
        undo()
      } else if (e.key === 'y' || (e.key === 'z' && e.shiftKey)) {
        e.preventDefault()
        redo()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  // BPM segue o input mesmo tocando (eventos em tempo musical acompanham)
  $effect(() => {
    engine.setBpm(store.song.tempo)
    if (!store.playing || !store.trainerOn) store.playbackBpm = store.song.tempo
  })

  // paleta flutuante só faz sentido editando a grade
  $effect(() => {
    if (!(view === 'grade' && store.mode === 'edit')) store.editing = false
  })

  // posiciona a paleta ancorada na célula focada: acima; se não couber, abaixo; nunca sobre a célula
  let floatEl: HTMLElement | undefined = $state()
  function positionPalette() {
    const a = document.activeElement
    if (!floatEl || !(a instanceof HTMLElement) || !a.classList.contains('cell')) return
    const cell = a.getBoundingClientRect()
    const pw = floatEl.offsetWidth
    const ph = floatEl.offsetHeight
    const gap = 8
    const vw = window.innerWidth
    const vh = window.innerHeight
    let left = cell.left + cell.width / 2 - pw / 2
    left = Math.max(8, Math.min(vw - pw - 8, left))
    const spaceAbove = cell.top - gap
    const spaceBelow = vh - cell.bottom - gap
    let top: number
    if (ph <= spaceAbove) top = cell.top - ph - gap
    else if (ph <= spaceBelow) top = cell.bottom + gap
    else top = spaceAbove >= spaceBelow ? 8 : Math.max(8, vh - ph - 8)
    floatEl.style.left = `${left}px`
    floatEl.style.top = `${top}px`
  }

  $effect(() => {
    if (!store.editing) return
    let raf = 0
    const loop = () => {
      positionPalette()
      raf = requestAnimationFrame(loop)
    }
    raf = requestAnimationFrame(loop)
    return () => cancelAnimationFrame(raf)
  })

  // mute/solo reativos (track e pessoa)
  $effect(() => {
    // só o que o mix usa: copiar a música inteira (com as notas) a cada edição custava caro
    engine.applyMix({ tracks: $state.snapshot(store.song.tracks), people: $state.snapshot(store.song.people) })
  })

  // edição durante o playback reagenda (sensação de step sequencer)
  $effect(() => {
    engine.setMetronome(store.metronome)
  })
  $effect(() => {
    // parado não precisa reagendar: não serializa nada (play reagenda ao começar)
    if (!store.playing) return
    JSON.stringify(store.song.measures)
    JSON.stringify(store.song.sections)
    JSON.stringify(store.song.instruments)
    // estrutura das linhas (instrumento/pessoas/ordem) — sem mute/solo/vol p/ não glitchar
    JSON.stringify(store.song.tracks.map((t) => [t.id, t.instrument, t.instrumentId, t.personIds]))
    void store.song.swing
    JSON.stringify(store.song.mainTrackIds)
    void store.song.mainTrackId
    const snap = $state.snapshot(store.song)
    engine.schedule(snap, onCursor)
    engine.setLoop(store.loop, snap)
  })

  $effect(() => {
    if (store.playing) engine.setLoop(store.loop, $state.snapshot(store.song))
  })

  // easter egg 🌸: 5 cliques no logo → gira e toca Sakura Sakura em fue
  let logoClicks = 0
  let logoClickTimer: ReturnType<typeof setTimeout> | undefined
  let logoSpinning = $state(false)

  async function onLogoClick() {
    clearTimeout(logoClickTimer)
    logoClicks++
    if (logoClicks >= 5) {
      logoClicks = 0
      if (logoSpinning) return
      logoSpinning = true
      const durMs = await playFue()
      setTimeout(() => (logoSpinning = false), Math.max(durMs, 1600))
    } else {
      logoClickTimer = setTimeout(() => (logoClicks = 0), 1500)
    }
  }

  async function onImport(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0]
    if (!file) return
    importError = await importSong(file)
    fileInput.value = ''
  }
</script>

<div class="shell">
  <div class="rail-slot"><NavRail open={navOpen} /></div>

  <header class="topbar">
    <div class="brand">
      <button class="mark" class:spinning={logoSpinning} onclick={onLogoClick} aria-label="WaDAWko">太</button>
      <button class="hamb" title="Alternar barra lateral" aria-label="Alternar barra lateral" onclick={() => (navOpen = !navOpen)}>☰</button>
      <input class="title" bind:value={store.song.title} aria-label="Título da música" placeholder="Sem título" />
    </div>

    <div class="transport">
      <label class="bpm">
        <span>BPM</span>
        <input type="number" min="30" max="240" bind:value={store.song.tempo} />
      </label>
      <div class="swing-switch" role="group" aria-label="Ritmo">
        <button class:on={!(store.song.swing ?? 0)} title="colcheias retas" onclick={() => (store.song.swing = 0)}>reto</button>
        <button class:on={(store.song.swing ?? 0) > 0} title="colcheia atrasada pro terço (hane)" onclick={() => (store.song.swing = 1)}>swing</button>
      </div>
      <button class="toggle" class:active={store.loop} aria-pressed={store.loop} onclick={() => (store.loop = !store.loop)}>↺ loop</button>
      <button class="tbtn" class:active={store.metronome} title="Metrônomo" aria-pressed={store.metronome} onclick={() => (store.metronome = !store.metronome)}>🎵</button>
      <button class="tbtn" class:active={store.countIn} title="Count-in: 1 compasso antes de tocar" aria-pressed={store.countIn} onclick={() => (store.countIn = !store.countIn)}>⏱</button>
      <button class="tbtn" class:active={store.trainerOn} title="Treino: em loop de seção, começa devagar e acelera a cada volta" aria-pressed={store.trainerOn} onclick={() => (store.trainerOn = !store.trainerOn)}>🏃</button>
      {#if store.trainerOn}
        <span class="trainer">
          <label>de <input type="number" min="20" max="100" value={Math.round(store.trainerStartPct * 100)} oninput={(e) => (store.trainerStartPct = Math.min(1, Math.max(0.2, (+(e.currentTarget as HTMLInputElement).value || 70) / 100)))} />%</label>
          <label>+<input type="number" min="1" max="50" value={Math.round(store.trainerStep * 100)} oninput={(e) => (store.trainerStep = Math.min(0.5, Math.max(0.01, (+(e.currentTarget as HTMLInputElement).value || 5) / 100)))} />%/volta</label>
        </span>
      {/if}
      <button class="play" onclick={togglePlay}>{store.playing ? '■ parar' : '▶ tocar'}</button>
      <button class="theme" title="Tema claro/escuro" aria-label="Tema" onclick={() => (theme = theme === 'dark' ? 'light' : 'dark')}>
        {theme === 'dark' ? '☀' : '☾'}
      </button>
    </div>
  </header>

  <div class="body">
    <main class="editor">
      {#if store.page === 'mixer'}
        <MixerPage />
      {:else if store.page === 'people'}
        <PeoplePanel />
      {:else if store.page === 'kit'}
        <KitPage />
      {:else}
      <div class="view-switch" role="tablist" aria-label="Visualização">
        <button role="tab" aria-selected={view === 'grade'} class:on={view === 'grade'} onclick={() => (view = 'grade')}>grade</button>
        <button role="tab" aria-selected={view === 'partitura'} class:on={view === 'partitura'} onclick={() => (view = 'partitura')}>partitura</button>
        <button role="tab" aria-selected={view === 'esteira'} class:on={view === 'esteira'} onclick={() => (view = 'esteira')}>esteira</button>
      </div>

      {#if view === 'grade'}
        <ContextToolbar />
      {/if}

      {#if view === 'grade'}
        <Grid bind:this={gridRef} />
        {#if store.mode === 'edit'}
          <div
            class="palette-float"
            class:show={store.editing}
            role="toolbar"
            tabindex="-1"
            aria-label="Paleta de sílabas"
            bind:this={floatEl}
            onmousedown={(e) => e.preventDefault()}
          >
            <section class="palette" aria-label="Sílabas">
              {#each PALETTE_GROUPS as group (group.label)}
                <div class="palette-row">
                  <span class="palette-label">{group.label}</span>
                  {#each group.syls as syl (syl)}
                    <button class="chip" class:selected={store.selectedSyl === syl} title={VOCABULARY[syl].hint} onmousedown={(e) => e.preventDefault()} onclick={() => onChipClick(syl)}>
                      {displaySyllable(syl)}
                    </button>
                  {/each}
                </div>
              {/each}
              <div class="palette-row">
                <span class="palette-label">frases</span>
                {#each PHRASES as known (known.name)}
                  <button class="chip phrase" title="{known.phrase} — {known.hint}" onmousedown={(e) => e.preventDefault()} onclick={() => onPhraseClick(known)} onmouseenter={() => onPhraseHover(known)} onmouseleave={onPhraseLeave}>
                    {known.name}
                  </button>
                {/each}
              </div>
              <span class="hint">
                {phraseFeedback ?? `${VOCABULARY[store.selectedSyl]?.hint ?? ''} — digite direto na célula; Tab/espaço avança; botão direito apaga`}
              </span>
            </section>
          </div>
        {/if}
      {:else if view === 'partitura'}
        <Score />
      {:else}
        <Lane />
      {/if}

      {#if importError}
        <p class="error" role="alert">{importError}</p>
      {/if}
      {/if}
    </main>

    <RightPanel onImport={() => fileInput.click()} />
  </div>

  <div class="status-slot"><StatusBar /></div>
</div>

<input type="file" accept=".json,application/json" bind:this={fileInput} onchange={onImport} hidden />

<CommandPalette open={cmdOpen} commands={commands} onClose={() => (cmdOpen = false)} />

<style>
  .shell {
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: auto 1fr auto;
    height: 100dvh;
    overflow: hidden;
  }

  .rail-slot {
    grid-row: 1 / 4;
    grid-column: 1;
    display: flex;
  }

  .topbar {
    grid-column: 2;
    grid-row: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.6rem 1rem;
    background: var(--paper);
    border-bottom: 1px solid var(--line);
  }

  .body {
    grid-column: 2;
    grid-row: 2;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  .editor {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 1rem 1.25rem 1.5rem;
  }

  .status-slot {
    grid-column: 2;
    grid-row: 3;
  }

  /* marca / brand */
  .brand {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-width: 0;
  }

  .mark {
    font-family: 'Shippori Mincho', serif;
    font-weight: 800;
    font-size: 1.4rem;
    line-height: 1;
    color: #fff;
    background: var(--accent);
    border: none;
    border-radius: 8px;
    width: 2.3rem;
    height: 2.3rem;
    display: grid;
    place-items: center;
    padding: 0;
    cursor: default;
    user-select: none;
  }

  .mark.spinning {
    animation: logo-spin 1.6s cubic-bezier(0.3, 0, 0.2, 1);
  }

  @keyframes logo-spin {
    from { transform: rotate(0deg) scale(1); }
    50% { transform: rotate(540deg) scale(1.25); }
    to { transform: rotate(1080deg) scale(1); }
  }

  .hamb {
    font: inherit;
    font-size: 1rem;
    border: 1px solid var(--line-strong);
    background: var(--paper-raise);
    color: var(--ink-soft);
    border-radius: 6px;
    width: 2rem;
    height: 2rem;
    cursor: pointer;
  }

  .title {
    font-family: 'Shippori Mincho', serif;
    font-size: 1.2rem;
    font-weight: 800;
    color: var(--ink);
    background: transparent;
    border: none;
    border-bottom: 2px dashed transparent;
    padding: 0.1rem 0.2rem;
    min-width: 8rem;
    max-width: 24rem;
  }

  .title:hover,
  .title:focus-visible {
    border-bottom-color: var(--line-strong);
    outline: none;
  }

  .transport {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .tbtn {
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    border: 1px solid var(--line-strong);
    background: var(--paper-raise);
    color: var(--ink-soft);
    font-weight: 700;
    font-size: 0.85rem;
  }

  .tbtn.active {
    background: var(--accent-2-wash);
    border-color: var(--accent-2);
    color: var(--accent-2);
  }

  .trainer {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--ink-soft);
  }

  .trainer input {
    width: 3rem;
    font: inherit;
    font-weight: 700;
    padding: 0.25rem 0.35rem;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    background: var(--paper-raise);
    color: var(--ink);
  }

  .bpm {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--ink-soft);
  }

  .bpm input {
    width: 4rem;
    font: inherit;
    font-weight: 700;
    padding: 0.3rem 0.45rem;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--paper-raise);
    color: var(--ink);
  }

  button {
    font: inherit;
    cursor: pointer;
  }

  .swing-switch {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    overflow: hidden;
  }

  .swing-switch button {
    padding: 0.4rem 0.65rem;
    border: none;
    background: var(--paper-raise);
    color: var(--ink-soft);
    font-weight: 700;
    font-size: 0.75rem;
  }

  .swing-switch button.on {
    background: var(--accent-2);
    color: #fff;
  }

  .toggle,
  .theme {
    padding: 0.4rem 0.8rem;
    border-radius: 6px;
    border: 1px solid var(--line-strong);
    background: var(--paper-raise);
    color: var(--ink);
    font-weight: 700;
    font-size: 0.8rem;
  }

  .theme {
    width: 2.3rem;
    padding: 0.4rem 0;
  }

  .toggle.active {
    border-color: var(--accent-2);
    color: var(--accent-2);
    background: var(--accent-2-wash);
  }

  .play {
    padding: 0.4rem 1rem;
    border-radius: 6px;
    background: var(--accent);
    border: 1px solid var(--accent);
    color: #fff;
    font-weight: 700;
    font-size: 0.85rem;
    min-width: 6rem;
  }

  .play:hover {
    filter: brightness(1.05);
  }

  /* view tabs */
  .view-switch {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    overflow: hidden;
    margin-bottom: 1rem;
  }

  .view-switch button {
    padding: 0.4rem 1rem;
    border: none;
    background: var(--paper-raise);
    color: var(--ink-soft);
    font-weight: 700;
    font-size: 0.8rem;
  }

  .view-switch button.on {
    background: var(--accent);
    color: #fff;
  }

  .view-switch button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .palette {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .palette-float {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 50;
    transform-origin: center bottom;
    transform: translateY(8px) scale(0.96);
    opacity: 0;
    pointer-events: none;
    transition:
      transform 180ms cubic-bezier(0.2, 0.9, 0.3, 1.3),
      opacity 150ms ease;
    background: var(--paper);
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.28);
    padding: 0.6rem 0.8rem;
    max-width: min(96vw, 60rem);
    max-height: 46vh;
    overflow-y: auto;
  }

  .palette-float.show {
    transform: translateY(0) scale(1);
    opacity: 1;
    pointer-events: auto;
  }

  .palette-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .palette-label {
    flex: 0 0 3.6rem;
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--ink-soft);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .chip {
    padding: 0.4rem 0.85rem;
    border-radius: 999px;
    border: 1px solid var(--line-strong);
    background: var(--paper-raise);
    color: var(--ink);
    font-weight: 500;
    font-size: 0.85rem;
  }

  .chip.phrase {
    border-style: dashed;
    font-size: 0.78rem;
    font-weight: 700;
  }

  .chip.phrase:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .chip.selected {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    font-weight: 700;
  }

  .chip:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .hint {
    font-size: 0.72rem;
    color: var(--ink-soft);
    margin-left: 0.5rem;
  }

  .error {
    color: var(--accent);
    font-size: 0.8rem;
    margin: 0;
    align-self: center;
  }
</style>
