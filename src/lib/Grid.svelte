<script lang="ts">
  import {
    store,
    setCell,
    getCell,
    getCellCol,
    setCellCol,
    ensureCols,
    colToPos,
    addMeasure,
    removeMeasure,
    sectionRanges,
    renameSection,
    removeSection,
    cloneSection,
    moveSection,
    setSectionRepeat,
    setMeasureDynamic,
    toggleMeasureHairpin,
    trackHasDynamics,
    toggleMainTrack,
    isMainTrack,
    removeTrack,
    fillSequential,
    fillPhraseBeats,
    railsForTracks,
    cellOwnerOf,
    personColor,
    personAudible,
    isShared,
    setCellsOwner,
    instrumentLabel,
    renameTrack,
    setTrackInstrument,
    moveTrack,
    toggleTrackPerson,
    paletteInstruments,
    resolveInstrument,
    getKyai,
    setKyai,
    fillKyai,
    trackHasKyai,
    type CellPos,
  } from './store.svelte'
  import { splitKyai, isShout, KYAI_PRESETS, KYAI_DEFAULTS } from './kyai'
  import { cellsPerMeasure, DYNAMIC_LEVELS, type Dynamic } from './types'
  import { mainMul } from './timeline'
  const DYNAMICS = Object.keys(DYNAMIC_LEVELS) as Dynamic[]

  /** instrumentos com a sub-linha de dinâmica aberta (quem tem marcação abre sozinho) */
  let dynOpen: string[] = $state([])
  const dynVisible = (trackId: string) => dynOpen.includes(trackId) || trackHasDynamics(trackId)
  function toggleDynRow(trackId: string) {
    if (dynOpen.includes(trackId)) dynOpen = dynOpen.filter((x) => x !== trackId)
    else dynOpen = [...dynOpen, trackId]
  }
  /** linhas com a sub-linha de kiai aberta (quem tem grito escrito abre sozinho) */
  let kyaiOpen: string[] = $state([])
  const kyaiVisible = (trackId: string) => kyaiOpen.includes(trackId) || trackHasKyai(trackId)
  function toggleKyaiRow(trackId: string) {
    if (kyaiOpen.includes(trackId)) kyaiOpen = kyaiOpen.filter((x) => x !== trackId)
    else kyaiOpen = [...kyaiOpen, trackId]
  }
  /** última célula de kiai focada (alvo do seletor de kakegoe prontos) */
  let kyaiFocus: { trackId: string; mi: number; ci: number } | null = null

  const cellSec = (mi: number) => 60 / store.song.tempo / (store.song.measures[mi]?.subdivision ?? 2)

  function kyaiInput(trackId: string, mi: number, ci: number): HTMLInputElement | null {
    return document.querySelector(`input.kcell[data-track="${trackId}"][data-mi="${mi}"][data-ci="${ci}"]`)
  }

  /** escreve o texto (uma ou mais palavras) a partir da célula: 1 sílaba por célula */
  function writeKyai(trackId: string, mi: number, ci: number, text: string, input?: HTMLInputElement | null): CellPos | null | false {
    if (text.trim() === '') {
      setKyai(trackId, mi, ci, null)
      return null
    }
    const syls = splitKyai(text)
    if (!syls) {
      if (input) {
        input.classList.add('invalid')
        setTimeout(() => input.classList.remove('invalid'), 600)
        input.value = getKyai(trackId, mi, ci) ?? ''
      }
      return false
    }
    const res = fillKyai(trackId, mi, ci, syls)
    if (input) input.value = getKyai(trackId, mi, ci) ?? ''
    setTimeout(() => {
      for (const pos of res.written) {
        const box = kyaiInput(trackId, pos.mi, pos.ci)?.closest('.kbox')
        box?.classList.add('flash')
        setTimeout(() => box?.classList.remove('flash'), 900)
      }
    }, 0)
    const track = store.song.tracks.find((t) => t.id === trackId)
    if (track) engine.previewKyai($state.snapshot(track), syls, cellSec(mi), cellOwnerOf(track, mi, ci) ?? undefined)
    return res.next
  }

  function onKyaiKey(e: KeyboardEvent, trackId: string, mi: number, ci: number) {
    const input = e.currentTarget as HTMLInputElement
    if (e.key === 'Enter') {
      e.preventDefault()
      const next = writeKyai(trackId, mi, ci, input.value, input)
      input.dataset.done = '1'
      if (next) kyaiInput(trackId, next.mi, next.ci)?.focus()
      else input.blur()
    } else if (e.key === 'Escape') {
      input.value = getKyai(trackId, mi, ci) ?? ''
      input.dataset.done = '1'
      input.blur()
    }
  }

  function onKyaiBlur(e: FocusEvent, trackId: string, mi: number, ci: number) {
    const input = e.currentTarget as HTMLInputElement
    if (input.dataset.done) {
      delete input.dataset.done
      return
    }
    if (input.value.trim() !== (getKyai(trackId, mi, ci) ?? '')) writeKyai(trackId, mi, ci, input.value, input)
  }

  /** seletor de kakegoe prontos: escreve a partir da última célula focada da linha */
  function insertKyaiPreset(trackId: string, text: string, firstMi: number) {
    const at = kyaiFocus?.trackId === trackId ? kyaiFocus : { trackId, mi: firstMi, ci: 0 }
    const next = writeKyai(trackId, at.mi, at.ci, text)
    if (next) kyaiFocus = { trackId, ...next }
  }

  function setKyaiVoice(trackId: string, key: 'pitch' | 'vary' | 'gain', v: number) {
    const t = store.song.tracks.find((x) => x.id === trackId)
    if (t) t.kyai = { ...t.kyai, [key]: v }
  }

  function testKyai(trackId: string) {
    const t = store.song.tracks.find((x) => x.id === trackId)
    if (!t) return
    const text = KYAI_PRESETS[Math.floor(Math.random() * KYAI_PRESETS.length)]
    engine.previewKyai($state.snapshot(t), splitKyai(text) ?? [], 60 / store.song.tempo / 2)
  }
  import { VOCABULARY } from './vocabulary'
  import { parseWords, resolveCell, isSfz, stripSfz, toggleSfz } from './phrase'
  import { engine } from './audio'
  import { cellFloatAt } from './lane'

  const measureStarts = $derived.by(() => {
    const starts: number[] = []
    let acc = 0
    for (const m of store.song.measures) {
      starts.push(acc)
      acc += cellsPerMeasure(m)
    }
    return starts
  })

  const totalCells = $derived(
    store.song.measures.reduce((sum, m) => sum + cellsPerMeasure(m), 0),
  )

  const sections = $derived(sectionRanges())

  // trilhos de agrupamento por pessoa (uma coluna colorida por pessoa, acesa nas
  // linhas que ela toca). Índice de rows casa com o índice de track (ti global).
  const rails = $derived(railsForTracks())

  const palette = $derived(paletteInstruments())

  // editor de linha (popover): trocar instrumento, atribuir pessoas, mover, renomear
  let trackEditor = $state<{ id: string; x: number; y: number } | null>(null)
  function openTrackEditor(e: MouseEvent, id: string) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    trackEditor = trackEditor?.id === id ? null : { id, x: r.left, y: r.bottom + 4 }
  }
  $effect(() => {
    if (!trackEditor) return
    function onDown(e: PointerEvent) {
      const t = e.target as HTMLElement
      if (t.closest('.track-editor') || t.closest('.te-open')) return
      trackEditor = null
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') trackEditor = null
    }
    window.addEventListener('pointerdown', onDown)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('pointerdown', onDown)
      window.removeEventListener('keydown', onKey)
    }
  })

  interface Block {
    id: string
    name: string
    start: number
    end: number
    selectable: boolean
  }

  // cada seção vira um bloco (grade separada); trechos sem seção viram blocos próprios
  const blocks = $derived.by<Block[]>(() => {
    const M = store.song.measures.length
    if (sections.length === 0) return [{ id: 'all', name: 'Música', start: 0, end: M - 1, selectable: false }]
    const res: Block[] = []
    if (sections[0].start > 0)
      res.push({ id: '__pre', name: '(sem seção)', start: 0, end: sections[0].start - 1, selectable: false })
    for (const s of sections) res.push({ id: s.id, name: s.name, start: s.start, end: s.end, selectable: true })
    return res
  })

  const range = (a: number, b: number) => Array.from({ length: Math.max(0, b - a + 1) }, (_, i) => a + i)

  const isSelected = (id: string) => store.selectedSections.includes(id)

  // clicar na caixa da seção seleciona; Ctrl/⌘ = multi-seleção
  function onBlockClick(e: MouseEvent, blk: Block) {
    if (!blk.selectable) return
    const t = e.target as HTMLElement
    if (t.closest('.score') || t.closest('button') || t.closest('input')) return // célula/controle, não seleção
    const ctrl = e.ctrlKey || e.metaKey
    if (ctrl) {
      store.selectedSections = isSelected(blk.id)
        ? store.selectedSections.filter((x) => x !== blk.id)
        : [...store.selectedSections, blk.id]
    } else {
      store.selectedSections = [blk.id]
    }
  }

  // minimizar seção (vira aba)
  let collapsed = $state(new Set<string>())
  function toggleCollapse(id: string) {
    const s = new Set(collapsed)
    if (s.has(id)) s.delete(id)
    else s.add(id)
    collapsed = s
  }

  // ---- reordenar seções arrastando a alça ----
  let dragSecId = $state<string | null>(null)

  function onHandleDown(e: PointerEvent, blk: Block) {
    e.preventDefault()
    e.stopPropagation()
    dragSecId = blk.id
    window.addEventListener('pointermove', onSecDragMove)
    window.addEventListener('pointerup', onSecDragUp)
  }

  function onSecDragMove(e: PointerEvent) {
    if (!dragSecId) return
    const over = document
      .elementFromPoint(e.clientX, e.clientY)
      ?.closest<HTMLElement>('.section-block')
      ?.getAttribute('data-secid')
    if (!over || over === dragSecId) return
    const order = blocks.filter((b) => b.selectable).map((b) => b.id)
    if (!order.includes(over) || !order.includes(dragSecId)) return
    moveSection(dragSecId, order.indexOf(over)) // reordena ao vivo
  }

  function onSecDragUp() {
    window.removeEventListener('pointermove', onSecDragMove)
    window.removeEventListener('pointerup', onSecDragUp)
    dragSecId = null
  }

  // arrastar a timeline = pan do bloco; clicar = mover o playhead (startCell)
  let panEl: HTMLElement | null = null
  let panning = false
  let panMoved = false
  let panStartX = 0
  let panScroll = 0
  let panId = -1

  function onTimelineDown(e: PointerEvent) {
    if (e.button !== 0) return
    e.stopPropagation() // não deixar o pan do corpo também disparar
    panEl = (e.currentTarget as HTMLElement).closest<HTMLElement>('.score')
    if (!panEl) return
    panning = true
    panMoved = false
    panStartX = e.clientX
    panScroll = panEl.scrollLeft
    panId = e.pointerId
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  function onTimelineMove(e: PointerEvent) {
    if (!panning || !panEl) return
    const dx = e.clientX - panStartX
    if (Math.abs(dx) > 4) panMoved = true
    panEl.scrollLeft = panScroll - dx
  }

  function onTimelineUp(e: PointerEvent) {
    if (!panning) return
    panning = false
    ;(e.currentTarget as HTMLElement).releasePointerCapture?.(panId)
    if (panMoved) return // foi arrasto (pan), não seek
    const hit = document.elementFromPoint(e.clientX, e.clientY)
    const cell = hit?.closest<HTMLElement>('.count')
    if (cell?.dataset.gidx) {
      store.startCell = Number(cell.dataset.gidx)
      if (store.playing) engine.seek($state.snapshot(store.song), store.startCell) // seek ao vivo
    }
  }

  // ---- auto-scroll suave seguindo o playback (rola o bloco da célula atual) ----
  /**
   * Destaque do cursor de playback aplicado direto no DOM: só as células da
   * coluna que sai e da que entra mudam. Antes, `class:cursor` fazia TODAS as
   * células re-rodarem o template a cada passo do cursor (8×/s a 120 BPM).
   */
  let gridRoot: HTMLElement | null = $state(null)
  $effect(() => {
    const c = store.cursor
    void colTemplates // grade re-renderizada → reaplica
    if (!gridRoot) return
    const on = c >= 0 ? gridRoot.querySelectorAll<HTMLElement>(`.cellbox[data-gidx="${c}"]`) : []
    on.forEach((el) => el.classList.add('cursor'))
    return () => on.forEach((el) => el.classList.remove('cursor'))
  })

  /**
   * cache coluna global → (bloco rolável, x do centro no conteúdo). A posição
   * no conteúdo não depende do scroll, então só muda quando a grade muda —
   * evita querySelector + getBoundingClientRect (layout forçado) a cada quadro.
   */
  let colCache = new Map<number, { sc: HTMLElement; x: number } | null>()

  function colCenter(i: number): { sc: HTMLElement; x: number } | null {
    if (colCache.has(i)) return colCache.get(i)!
    const t0 = store.song.tracks[0]?.id
    const p = t0 ? colToPos(i) : null
    const el = p ? cellInput(t0!, p.mi, p.ci) : null
    const sc = el?.closest<HTMLElement>('.score') ?? null
    let res: { sc: HTMLElement; x: number } | null = null
    if (el && sc) {
      const b = sc.getBoundingClientRect()
      const r = el.getBoundingClientRect()
      res = { sc, x: r.left - b.left + sc.scrollLeft + r.width / 2 }
    }
    colCache.set(i, res)
    return res
  }

  function follow() {
    const song = store.song
    if (!song.tracks.length) return
    const gf = cellFloatAt(song, engine.transportSeconds())
    const i = Math.floor(gf)
    const c0 = colCenter(i)
    if (!c0) return
    const c1 = colCenter(i + 1)
    const x = c1 && c1.sc === c0.sc ? c0.x + (c1.x - c0.x) * (gf - i) : c0.x
    const sc = c0.sc
    const target = Math.max(0, Math.min(sc.scrollWidth - sc.clientWidth, x - sc.clientWidth * 0.4))
    // só escreve quando muda de fato (≥ 0.5px): menos trabalho de composição
    if (Math.abs(sc.scrollLeft - target) >= 0.5) sc.scrollLeft = target
  }

  $effect(() => {
    if (!store.playing) return
    // grade mudou (edição, zoom, seções, colapso) → posições em cache invalidam
    void colTemplates
    void store.zoom
    void collapsed.size
    void store.song.tracks.length
    void store.song.sections?.length
    colCache = new Map()
    const onResize = () => (colCache = new Map())
    window.addEventListener('resize', onResize)
    let raf = 0
    const step = () => {
      follow()
      raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    return () => {
      cancelAnimationFrame(raf)
      window.removeEventListener('resize', onResize)
    }
  })

  // ---- pan do corpo do bloco: botão do meio (qualquer modo) ou esquerdo (edit) ----
  let bodyEl: HTMLElement | null = null
  let bodyDown = false
  let bodyPan = $state(false) // pan de fato (só após mover / botão do meio)
  let bodyX = 0
  let bodyScroll = 0
  let bodyId = -1

  function onScoreDown(e: PointerEvent) {
    const mid = e.button === 1
    const leftEdit = e.button === 0 && store.mode === 'edit'
    if (!mid && !leftEdit) return
    bodyEl = e.currentTarget as HTMLElement
    bodyDown = true
    bodyX = e.clientX
    bodyScroll = bodyEl.scrollLeft
    bodyId = e.pointerId
    if (mid) {
      e.preventDefault() // corta o autoscroll do botão do meio
      bodyPan = true
      bodyEl.setPointerCapture(bodyId)
    }
  }

  function onScoreMove(e: PointerEvent) {
    if (!bodyDown || !bodyEl) return
    const dx = e.clientX - bodyX
    if (!bodyPan && Math.abs(dx) > 4) {
      bodyPan = true
      bodyEl.setPointerCapture(bodyId)
      ;(document.activeElement as HTMLElement | null)?.blur?.()
      document.getSelection()?.removeAllRanges()
    }
    if (bodyPan) bodyEl.scrollLeft = bodyScroll - dx
  }

  function onScoreUp() {
    if (!bodyDown) return
    bodyDown = false
    if (bodyPan) bodyEl?.releasePointerCapture?.(bodyId)
    bodyPan = false
  }

  // ---- modo select: rubber-band, mover, clonar, copiar/colar ----
  interface Sel {
    r0: number
    c0: number
    r1: number
    c1: number
  }
  let sel: Sel | null = $state(null)
  let clip: { rows: number; cols: number; data: (string | null)[][] } | null = null
  let dragMode: 'none' | 'band' | 'move' = $state('none')
  let anchor: { r: number; c: number } | null = null
  let moveStart: { r: number; c: number } | null = null
  let cloneDrag = false
  let previewDelta: { dr: number; dc: number } | null = $state(null)

  const norm = (s: Sel) => ({
    r0: Math.min(s.r0, s.r1),
    r1: Math.max(s.r0, s.r1),
    c0: Math.min(s.c0, s.c1),
    c1: Math.max(s.c0, s.c1),
  })

  function inSel(r: number, c: number): boolean {
    if (!sel) return false
    const n = norm(sel)
    return r >= n.r0 && r <= n.r1 && c >= n.c0 && c <= n.c1
  }

  /** classe visual da célula (ti, coluna global) no modo select */
  function selClass(r: number, c: number): 'sel' | 'selsrc' | 'movet' | '' {
    if (dragMode === 'move' && sel && previewDelta) {
      const n = norm(sel)
      const inTarget =
        r >= n.r0 + previewDelta.dr &&
        r <= n.r1 + previewDelta.dr &&
        c >= n.c0 + previewDelta.dc &&
        c <= n.c1 + previewDelta.dc
      if (inTarget) return 'movet'
      if (inSel(r, c)) return 'selsrc'
      return ''
    }
    return inSel(r, c) ? 'sel' : ''
  }

  const trackIndex = (id: string) => store.song.tracks.findIndex((t) => t.id === id)

  function cellFromPoint(x: number, y: number): { r: number; c: number } | null {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('input.cell')
    if (!el?.dataset.track || el.dataset.mi === undefined || el.dataset.ci === undefined) return null
    const r = trackIndex(el.dataset.track)
    const c = measureStarts[Number(el.dataset.mi)] + Number(el.dataset.ci)
    return r < 0 ? null : { r, c }
  }

  function onCellPointerDown(e: PointerEvent, trackId: string, mi: number, ci: number) {
    if (store.mode !== 'select') return
    e.preventDefault()
    const r = trackIndex(trackId)
    const c = measureStarts[mi] + ci
    cloneDrag = e.altKey
    if (e.shiftKey) {
      // Shift = selecionar range (funciona por várias linhas)
      dragMode = 'band'
      if (!anchor) anchor = { r, c }
      sel = { r0: anchor.r, c0: anchor.c, r1: r, c1: c }
    } else {
      // sem shift = mover; se a célula não está na seleção, seleciona só ela
      if (!sel || !inSel(r, c)) {
        sel = { r0: r, c0: c, r1: r, c1: c }
        anchor = { r, c }
      }
      dragMode = 'move'
      moveStart = { r, c }
      previewDelta = { dr: 0, dc: 0 }
    }
    window.addEventListener('pointermove', onSelDragMove)
    window.addEventListener('pointerup', onSelDragUp)
  }

  function onSelDragMove(e: PointerEvent) {
    const hit = cellFromPoint(e.clientX, e.clientY)
    if (!hit) return
    if (dragMode === 'band' && anchor) {
      sel = { r0: anchor.r, c0: anchor.c, r1: hit.r, c1: hit.c }
    } else if (dragMode === 'move' && moveStart) {
      previewDelta = { dr: hit.r - moveStart.r, dc: hit.c - moveStart.c }
    }
  }

  function onSelDragUp(e: PointerEvent) {
    window.removeEventListener('pointermove', onSelDragMove)
    window.removeEventListener('pointerup', onSelDragUp)
    if (dragMode === 'move' && previewDelta && (previewDelta.dr || previewDelta.dc)) {
      applyMove(previewDelta.dr, previewDelta.dc, e.altKey || cloneDrag)
    }
    dragMode = 'none'
    moveStart = null
    previewDelta = null
  }

  function applyMove(dr: number, dc: number, clone: boolean) {
    if (!sel) return
    const n = norm(sel)
    const tracks = store.song.tracks
    const block: { r: number; c: number; v: string | null }[] = []
    for (let r = n.r0; r <= n.r1; r++)
      for (let c = n.c0; c <= n.c1; c++) block.push({ r, c, v: getCellCol(tracks[r].id, c) })
    ensureCols(n.c1 + dc + 1) // cria compassos se o alvo passa do fim
    if (!clone) for (const b of block) setCellCol(tracks[b.r].id, b.c, null)
    for (const b of block) {
      const nr = b.r + dr
      const nc = b.c + dc
      if (nr < 0 || nr >= tracks.length || nc < 0) continue
      setCellCol(tracks[nr].id, nc, b.v)
    }
    sel = { r0: n.r0 + dr, c0: n.c0 + dc, r1: n.r1 + dr, c1: n.c1 + dc }
  }

  function copySel() {
    if (!sel) return
    const n = norm(sel)
    const tracks = store.song.tracks
    const data: (string | null)[][] = []
    for (let r = n.r0; r <= n.r1; r++) {
      const row: (string | null)[] = []
      for (let c = n.c0; c <= n.c1; c++) row.push(getCellCol(tracks[r].id, c))
      data.push(row)
    }
    clip = { rows: n.r1 - n.r0 + 1, cols: n.c1 - n.c0 + 1, data }
  }

  function clearSel() {
    if (!sel) return
    const n = norm(sel)
    const tracks = store.song.tracks
    for (let r = n.r0; r <= n.r1; r++) for (let c = n.c0; c <= n.c1; c++) setCellCol(tracks[r].id, c, null)
  }

  /**
   * liga/desliga sfz nas notas selecionadas: se alguma ainda não tem, liga em
   * todas; se todas já têm, desliga (células vazias são ignoradas)
   */
  function toggleSfzSel() {
    if (!sel) return
    const n = norm(sel)
    const tracks = store.song.tracks
    const cells: { id: string; c: number; v: string }[] = []
    for (let r = n.r0; r <= n.r1; r++)
      for (let c = n.c0; c <= n.c1; c++) {
        const v = getCellCol(tracks[r].id, c)
        if (v) cells.push({ id: tracks[r].id, c, v })
      }
    const turnOn = cells.some((x) => !isSfz(x.v))
    for (const x of cells) if (isSfz(x.v) !== turnOn) setCellCol(x.id, x.c, toggleSfz(x.v))
  }

  /** reatribui o dono das células selecionadas que estão em tracks compartilhadas */
  function assignSelToPerson(personId: string | null) {
    if (!sel) return
    const n = norm(sel)
    const tracks = store.song.tracks
    const cells: { trackId: string; mi: number; ci: number }[] = []
    for (let r = n.r0; r <= n.r1; r++) {
      const track = tracks[r]
      if (!track || !isShared(track)) continue
      for (let c = n.c0; c <= n.c1; c++) {
        const p = colToPos(c)
        if (p) cells.push({ trackId: track.id, mi: p.mi, ci: p.ci })
      }
    }
    setCellsOwner(cells, personId)
  }

  function pasteSel() {
    if (!clip || !sel) return
    const n = norm(sel)
    const tracks = store.song.tracks
    ensureCols(n.c0 + clip.cols) // cria compassos se a colagem passa do fim
    for (let i = 0; i < clip.rows; i++)
      for (let j = 0; j < clip.cols; j++) {
        const r = n.r0 + i
        if (r >= tracks.length) continue
        setCellCol(tracks[r].id, n.c0 + j, clip.data[i][j])
      }
    sel = { r0: n.r0, c0: n.c0, r1: Math.min(tracks.length - 1, n.r0 + clip.rows - 1), c1: n.c0 + clip.cols - 1 }
  }

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (store.mode !== 'select') return
      if ((e.target as HTMLElement | null)?.closest?.('input.kcell')) return // digitando kiai
      const meta = e.ctrlKey || e.metaKey
      if (meta && e.key === 'c') {
        e.preventDefault()
        copySel()
      } else if (meta && e.key === 'x') {
        e.preventDefault()
        copySel()
        clearSel()
      } else if (meta && e.key === 'v') {
        e.preventDefault()
        pasteSel()
      } else if (e.key === 'Delete' || e.key === 'Backspace') {
        if (!sel) return
        e.preventDefault()
        clearSel()
      } else if (e.key === '!' && sel) {
        e.preventDefault()
        toggleSfzSel()
      } else if (e.key === 'Escape') {
        sel = null
      } else if (sel && e.key.startsWith('Arrow')) {
        const d: Record<string, [number, number]> = {
          ArrowUp: [-1, 0],
          ArrowDown: [1, 0],
          ArrowLeft: [0, -1],
          ArrowRight: [0, 1],
        }
        const step = d[e.key]
        if (step) {
          e.preventDefault()
          applyMove(step[0], step[1], e.altKey)
        }
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  function countLabel(ci: number, subdivision: number): string {
    if (ci % subdivision === 0) return String(ci / subdivision + 1)
    if (subdivision > 1 && ci % subdivision === subdivision / 2) return 'e'
    return '·'
  }

  interface Seg {
    zone: 'head' | 'rim'
    accent: boolean
    both: boolean
  }

  /** golpes da célula agrupados por instante: 1 sub-célula por nota tocada */
  function cellSegments(syl: string | null): Seg[] {
    if (!syl) return []
    const r = resolveCell(syl)
    if (!r || !r.hits.length) return []
    const groups = new Map<number, typeof r.hits>()
    for (const h of r.hits) {
      const g = groups.get(h.offset) ?? []
      g.push(h)
      groups.set(h.offset, g)
    }
    return [...groups.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([, g]) => ({
        zone: g[0].zone,
        accent: g.some((h) => h.accent),
        both: g.length >= 2 && new Set(g.map((h) => h.hand)).size >= 2,
      }))
  }

  /**
   * largura de cada coluna por compasso. Estica pra caber as sub-células quando
   * há muitos golpes (dokodoko); usa o MAIOR nº de golpes entre os instrumentos
   * naquele tempo, pra manter o alinhamento vertical entre as linhas.
   */
  // nº de sub-células por sílaba (puro): evita re-resolver ~3600 células a cada edição
  const segCountCache = new Map<string, number>()
  function segCount(syl: string): number {
    let n = segCountCache.get(syl)
    if (n === undefined) segCountCache.set(syl, (n = cellSegments(syl).length))
    return n
  }
  // mesmo array quando nada mudou: quem depende da grade (cursor, auto-scroll) não re-roda
  let prevTemplates: string[] = []
  const colTemplates = $derived.by(() => {
    const trackIds = store.song.tracks.map((t) => t.id)
    const next = store.song.measures.map((meas) => {
      const n = cellsPerMeasure(meas)
      const base = (meas.subdivision >= 8 ? 1.9 : meas.subdivision >= 4 ? 2.5 : 3.4) * store.zoom
      const segMin = 0.95 * store.zoom
      const rows = trackIds.map((id) => meas.cells[id])
      const cols: string[] = []
      for (let ci = 0; ci < n; ci++) {
        let maxSeg = 1
        for (const row of rows) {
          const c = row?.[ci]
          const s = c ? segCount(c) : 0
          if (s > maxSeg) maxSeg = s
        }
        cols.push(`${Math.max(base, maxSeg * segMin).toFixed(2)}rem`)
      }
      return cols.join(' ')
    })
    if (next.length === prevTemplates.length && next.every((t, i) => t === prevTemplates[i])) return prevTemplates
    return (prevTemplates = next)
  })

  /**
   * Aplica o texto digitado na célula. Uma palavra (mesmo composta, tipo
   * "dokodoko") ocupa ESTA célula; frase com espaços flui uma palavra por
   * célula. Retorna false se alguma palavra for inválida.
   */
  function cellInput(trackId: string, mi: number, ci: number): HTMLInputElement | null {
    return document.querySelector(`input.cell[data-track="${trackId}"][data-mi="${mi}"][data-ci="${ci}"]`)
  }

  /** pisca em vermelho as células que a inserção tocou */
  function flashCells(trackId: string, cells: CellPos[]) {
    setTimeout(() => {
      for (const pos of cells) {
        const el = cellInput(trackId, pos.mi, pos.ci)
        if (!el) continue
        const box = el.closest('.cellbox') ?? el
        box.classList.add('flash')
        setTimeout(() => box.classList.remove('flash'), 900)
      }
    }, 0)
  }

  interface CommitResult {
    ok: boolean
    /** posição pra onde o foco deve ir depois da inserção (fim da seleção) */
    next: CellPos | null
  }

  function commitCell(
    input: HTMLInputElement,
    trackId: string,
    mi: number,
    ci: number,
    perBeat = false,
  ): CommitResult {
    const text = input.value.trim()
    if (text === '') {
      setCell(trackId, mi, ci, null)
      return { ok: true, next: null }
    }
    const words = parseWords(text)
    if (!words || words.length === 0) {
      input.classList.add('invalid')
      setTimeout(() => input.classList.remove('invalid'), 600)
      input.value = getCell(trackId, mi, ci) ?? ''
      return { ok: false, next: null }
    }
    let next: CellPos | null = null
    if (words.length === 1) {
      setCell(trackId, mi, ci, words[0])
      flashCells(trackId, [{ mi, ci }])
    } else {
      const result = perBeat
        ? fillPhraseBeats(trackId, mi, ci, words)
        : fillSequential(trackId, mi, ci, words)
      flashCells(trackId, result.written)
      next = result.next
    }
    // sincroniza o DOM com o store: se a 1ª palavra coincidir com o valor
    // anterior, o Svelte não re-renderiza e a frase inteira ficaria no input
    input.value = getCell(trackId, mi, ci) ?? ''
    const track = store.song.tracks.find((t) => t.id === trackId)
    if (track && resolveCell(words[0])?.hits.length) engine.preview($state.snapshot(track), words[0])
    return { ok: true, next }
  }

  function focusNeighbor(input: HTMLInputElement, dir: 1 | -1) {
    const row = input.closest('.row')
    if (!row) return
    const inputs = [...row.querySelectorAll<HTMLInputElement>('input.cell')]
    const i = inputs.indexOf(input)
    inputs[i + dir]?.focus()
  }

  function focusAfter(input: HTMLInputElement, trackId: string, next: CellPos | null) {
    if (next) {
      const target = cellInput(trackId, next.mi, next.ci)
      if (target) {
        target.focus()
        return
      }
    }
    focusNeighbor(input, 1)
  }

  function onCellKey(event: KeyboardEvent, trackId: string, mi: number, ci: number) {
    const input = event.currentTarget as HTMLInputElement
    if (event.key === ' ') {
      event.preventDefault()
      const result = commitCell(input, trackId, mi, ci)
      if (result.ok) focusAfter(input, trackId, result.next)
    } else if (event.key === 'Enter') {
      input.blur()
    } else if (event.key === 'Escape') {
      input.value = getCell(trackId, mi, ci) ?? ''
      input.blur()
    } else if (event.key === 'ArrowRight' && input.selectionStart === input.value.length) {
      focusNeighbor(input, 1)
    } else if (event.key === 'ArrowLeft' && input.selectionStart === 0) {
      focusNeighbor(input, -1)
    }
  }

  // foco numa célula = mostra a paleta flutuante; blur esconde (com delay pra
  // trocar de célula / clicar num chip não piscar)
  let editTimer: ReturnType<typeof setTimeout>
  function onCellFocus(e: FocusEvent) {
    clearTimeout(editTimer)
    store.editing = true
    ;(e.currentTarget as HTMLInputElement).select()
  }
  function onCellBlur(e: FocusEvent, trackId: string, mi: number, ci: number) {
    commitCell(e.currentTarget as HTMLInputElement, trackId, mi, ci)
    clearTimeout(editTimer)
    editTimer = setTimeout(() => (store.editing = false), 180)
  }

  function erase(event: MouseEvent, trackId: string, mi: number, ci: number) {
    event.preventDefault()
    setCell(trackId, mi, ci, null)
    ;(event.currentTarget as HTMLInputElement).value = ''
  }

  /** paleta/frases escrevem na célula focada (mousedown preventDefault preserva o foco) */
  export function writeToFocused(text: string, perBeat = false): boolean {
    const active = document.activeElement
    if (!(active instanceof HTMLInputElement) || !active.classList.contains('cell')) return false
    const { track, mi, ci } = active.dataset
    if (track === undefined || mi === undefined || ci === undefined) return false
    active.value = text
    const result = commitCell(active, track, Number(mi), Number(ci), perBeat)
    if (result.ok) focusAfter(active, track, result.next)
    return true
  }
</script>

{#snippet blockView(blk: Block)}
  {@const collapsedNow = collapsed.has(blk.id)}
  <section
    class="section-block"
    class:active={blk.selectable && isSelected(blk.id)}
    class:playing={store.playing && blk.selectable && isSelected(blk.id)}
    class:dragging={dragSecId === blk.id}
    class:collapsed={collapsedNow}
    data-secid={blk.id}
    role="presentation"
    onclick={(e) => onBlockClick(e, blk)}
  >
    <header class="block-head">
      {#if blk.selectable}
        <button
          class="sec-handle"
          title="Arraste para reordenar a seção"
          onpointerdown={(e) => onHandleDown(e, blk)}>⠿</button
        >
        <input
          class="sec-name"
          value={blk.name}
          oninput={(e) => renameSection(blk.id, (e.currentTarget as HTMLInputElement).value)}
          onclick={(e) => e.stopPropagation()}
          spellcheck="false"
        />
        {@const rep = store.song.sections?.find((x) => x.id === blk.id)?.repeat ?? 1}
        <span class="sec-repeat" class:on={rep > 1} title="Ritornello: quantas vezes a seção toca">
          <span class="rp-mark" aria-hidden="true">𝄆</span>
          <button
            aria-label="Menos repetições"
            disabled={rep <= 1}
            onclick={(e) => { e.stopPropagation(); setSectionRepeat(blk.id, rep - 1) }}>−</button
          >
          <span class="rp-n">{rep}×</span>
          <button
            aria-label="Mais repetições"
            onclick={(e) => { e.stopPropagation(); setSectionRepeat(blk.id, rep + 1) }}>+</button
          >
          <span class="rp-mark" aria-hidden="true">𝄇</span>
        </span>
      {:else}
        <span class="sec-name-static">{blk.name}</span>
      {/if}

      {#if !blk.selectable || isSelected(blk.id)}
        <span class="blk-compasso">
          <span class="cl">compasso</span>
          <button title="Adicionar compasso a esta seção" onclick={(e) => { e.stopPropagation(); addMeasure(blk.selectable ? blk.id : undefined) }}>+</button>
          <button title="Remover compasso desta seção" onclick={(e) => { e.stopPropagation(); removeMeasure(blk.selectable ? blk.id : undefined) }}>−</button>
        </span>
      {/if}

      {#if blk.selectable}
        <button
          class="sec-min"
          title={collapsedNow ? 'Expandir' : 'Minimizar (vira aba)'}
          onclick={() => toggleCollapse(blk.id)}>{collapsedNow ? '▸' : '▾'}</button
        >
        <button class="sec-clone" title="Clonar seção (cópia logo depois desta)" onclick={() => cloneSection(blk.id)}>⧉ clonar</button>
        <button class="sec-del" title="Remover seção" onclick={() => removeSection(blk.id)}>×</button>
      {/if}
    </header>

    {#if !collapsedNow}
    <div
      class="score"
      class:selectmode={store.mode === 'select'}
      class:panning={bodyPan}
      role="grid"
      tabindex="-1"
      aria-label={`Grade — ${blk.name}`}
      onpointerdown={onScoreDown}
      onpointermove={onScoreMove}
      onpointerup={onScoreUp}
      onpointercancel={onScoreUp}
    >
      <!-- dinâmica: marcação (pp..ff) e crescendo/diminuendo por compasso -->
      <div class="row dyn-row" role="presentation" onpointerdown={(e) => e.stopPropagation()}>
        {#if rails.length}
          <div class="rail-gutter" aria-hidden="true">
            {#each rails as rail (rail.person.id)}
              <div class="rail spacer"></div>
            {/each}
          </div>
        {/if}
        <div class="track-head spacer"><span class="tl-hint">dinâmica</span></div>
        <div class="measures">
          {#each range(blk.start, blk.end) as mi (mi)}
            <div class="measure dyn" style:grid-template-columns={colTemplates[mi]}>
              {@render dynCell(mi, null)}
            </div>
          {/each}
        </div>
      </div>

      <!-- timeline: arrastar faz pan, clicar move o playhead -->
      <div
        class="row count-row timeline"
        role="slider"
        tabindex="0"
        aria-label="Timeline — arraste para navegar, clique para tocar daqui"
        aria-valuemin="0"
        aria-valuemax={Math.max(0, totalCells - 1)}
        aria-valuenow={store.startCell}
        onpointerdown={onTimelineDown}
        onpointermove={onTimelineMove}
        onpointerup={onTimelineUp}
        onpointercancel={onTimelineUp}
      >
        {#if rails.length}
          <div class="rail-gutter" aria-hidden="true">
            {#each rails as rail (rail.person.id)}
              <div class="rail spacer"></div>
            {/each}
          </div>
        {/if}
        <div class="track-head spacer"><span class="tl-hint">↔ arraste · clique = tocar daqui</span></div>
        <div class="measures">
          {#each range(blk.start, blk.end) as mi (mi)}
            {@const meas = store.song.measures[mi]}
            <div class="measure counts" style:grid-template-columns={colTemplates[mi]}>
              {#each Array(cellsPerMeasure(meas)) as _, ci}
                {@const gidx = measureStarts[mi] + ci}
                <div
                  class="count"
                  class:beat={ci % meas.subdivision === 0}
                  class:start={store.startCell === gidx}
                  data-gidx={gidx}
                >
                  {countLabel(ci, meas.subdivision)}
                </div>
              {/each}
            </div>
          {/each}
        </div>
      </div>

      {#each store.song.tracks as track, ti (track.id)}
        <div class="row">
          {#if rails.length}
            <div class="rail-gutter">
              {#each rails as rail (rail.person.id)}
                {@const owns = rail.rows[ti]}
                {@const capTop = owns && !(ti > 0 && rail.rows[ti - 1])}
                <div class="rail" style:--pc={rail.person.color ?? 'var(--accent)'} title={rail.person.name}>
                  {#if owns}<span class="bar" class:extend={rail.rows[ti + 1]} class:cap-top={capTop}></span>{/if}
                  {#if capTop}<span class="rail-name">{rail.person.name}</span>{/if}
                </div>
              {/each}
            </div>
          {/if}
          <div class="track-head">
            <div class="track-name" class:recessed={mainMul(store.song, track.id) < 1}>
              <span class="name">{track.name}</span>
              <span class="inst">{instrumentLabel(track)}</span>
            </div>
            <div class="track-ctl">
              <button
                class="mini te-open"
                class:active={trackEditor?.id === track.id}
                title="Editar linha (instrumento, pessoas, posição)"
                onclick={(e) => openTrackEditor(e, track.id)}>⚙</button
              >
              <button
                class="mini dyn-toggle"
                class:active={dynVisible(track.id)}
                class:marked={trackHasDynamics(track.id)}
                title={trackHasDynamics(track.id) ? 'Dinâmica própria deste instrumento (tem marcações)' : 'Dinâmica própria deste instrumento'}
                aria-pressed={dynVisible(track.id)}
                disabled={trackHasDynamics(track.id)}
                onclick={() => toggleDynRow(track.id)}><i>f</i></button
              >
              <button
                class="mini kyai-toggle"
                class:active={kyaiVisible(track.id)}
                class:marked={trackHasKyai(track.id)}
                title={trackHasKyai(track.id) ? 'Kiai (gritos) desta linha — tem gritos escritos' : 'Kiai (gritos/kakegoe) desta linha'}
                aria-pressed={kyaiVisible(track.id)}
                disabled={trackHasKyai(track.id)}
                onclick={() => toggleKyaiRow(track.id)}>声</button
              >
              <button
                class="mini"
                class:active={track.muted}
                title="Mudo"
                aria-pressed={track.muted}
                onclick={() => (track.muted = !track.muted)}>M</button
              >
              <button
                class="mini"
                class:active={track.solo}
                title="Solo"
                aria-pressed={track.solo}
                onclick={() => (track.solo = !track.solo)}>S</button
              >
              <button
                class="mini main-toggle"
                class:active={isMainTrack(track.id)}
                title={isMainTrack(track.id)
                  ? 'Main: em destaque (como f); os não marcados recuam (como p). Clique pra tirar do destaque'
                  : 'Main: pôr em destaque (como f); dá pra marcar vários — os não marcados recuam (como p)'}
                aria-pressed={isMainTrack(track.id)}
                onclick={() => toggleMainTrack(track.id)}>★</button
              >
              <button class="mini danger" title="Remover linha" onclick={() => removeTrack(track.id)}>×</button>
            </div>
          </div>
          <div class="measures">
            {#each range(blk.start, blk.end) as mi (mi)}
              {@const meas = store.song.measures[mi]}
              <div class="measure" class:narrow={meas.subdivision >= 8} style:grid-template-columns={colTemplates[mi]}>
                {#each Array(cellsPerMeasure(meas)) as _, ci}
                  {@const syl = getCell(track.id, mi, ci)}
                  {@const globalIdx = measureStarts[mi] + ci}
                  {@const scls = store.mode === 'select' ? selClass(ti, globalIdx) : ''}
                  {@const segs = cellSegments(syl)}
                  {@const owner = cellOwnerOf(track, mi, ci)}
                  {@const ownerCol = personColor(owner)}
                  {@const ownerMuted = owner ? !personAudible(owner) : false}
                  <div
                    class="cellbox"
                    class:filled={syl !== null}
                    class:beat={ci % meas.subdivision === 0}
                    data-gidx={globalIdx}
                    class:start={store.startCell === globalIdx}
                    class:sel={scls === 'sel'}
                    class:selsrc={scls === 'selsrc'}
                    class:movet={scls === 'movet'}
                    class:owner-hint={isShared(track) && !!ownerCol}
                    class:owner-muted={ownerMuted}
                    style:--owner={ownerCol ?? 'transparent'}
                  >
                    {#if isSfz(syl)}<span class="sfz-badge" aria-hidden="true">sfz</span>{/if}
                    {#if segs.length}
                      <div class="hits" aria-hidden="true">
                        {#each segs as s, si (si)}
                          <span class="hit" class:couro={s.zone === 'head'} class:fuchi={s.zone === 'rim'} class:accent={s.accent} class:both={s.both}></span>
                        {/each}
                      </div>
                    {/if}
                    <input
                      class="cell"
                      class:long={(syl?.length ?? 0) > 4}
                      readonly={store.mode === 'select'}
                      onpointerdown={(e) => onCellPointerDown(e, track.id, mi, ci)}
                      value={syl ?? ''}
                      title={syl
                        ? `${syl} — ${VOCABULARY[stripSfz(syl)]?.hint ?? 'palavra composta'}${isSfz(syl) ? ' · sfz (sforzando)' : ''}`
                        : 'digite a sílaba ou palavra composta (dokodoko); Tab/espaço avança; botão direito apaga'}
                      data-track={track.id}
                      data-mi={mi}
                      data-ci={ci}
                      onkeydown={(e) => onCellKey(e, track.id, mi, ci)}
                      onblur={(e) => onCellBlur(e, track.id, mi, ci)}
                      onfocus={onCellFocus}
                      oncontextmenu={(e) => erase(e, track.id, mi, ci)}
                      spellcheck="false"
                      autocomplete="off"
                    />
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        </div>
        {#if kyaiVisible(track.id)}
          <div class="row kyai-row" role="presentation" onpointerdown={(e) => e.stopPropagation()}>
            {#if rails.length}
              <div class="rail-gutter" aria-hidden="true">
                {#each rails as rail (rail.person.id)}
                  <div class="rail spacer"></div>
                {/each}
              </div>
            {/if}
            <div class="track-head spacer kyai-head">
              <span class="tl-hint">声 kiai · {track.name}</span>
              <select
                class="kyai-presets"
                title="Inserir um kakegoe pronto na última célula de kiai clicada (ou no início do bloco)"
                value=""
                onchange={(e) => {
                  const sel = e.currentTarget as HTMLSelectElement
                  if (sel.value) insertKyaiPreset(track.id, sel.value, blk.start)
                  sel.value = ''
                }}
              >
                <option value="">＋ kakegoe…</option>
                {#each KYAI_PRESETS as k (k)}<option value={k}>{k}</option>{/each}
              </select>
            </div>
            <div class="measures">
              {#each range(blk.start, blk.end) as mi (mi)}
                {@const meas = store.song.measures[mi]}
                <div class="measure kyai" class:narrow={meas.subdivision >= 8} style:grid-template-columns={colTemplates[mi]}>
                  {#each Array(cellsPerMeasure(meas)) as _, ci}
                    {@const k = getKyai(track.id, mi, ci)}
                    <div class="kbox" class:filled={!!k} class:shout={!!k && isShout(k)} class:beat={ci % meas.subdivision === 0}>
                      <input
                        class="kcell"
                        list="kyai-list"
                        value={k ?? ''}
                        data-track={track.id}
                        data-mi={mi}
                        data-ci={ci}
                        title={k ? `kiai: ${k}` : 'kiai: digite (sa!, seiya!, soriya!…) — 1 sílaba por célula; Enter grava; botão direito apaga'}
                        onfocus={(e) => {
                          kyaiFocus = { trackId: track.id, mi, ci }
                          ;(e.currentTarget as HTMLInputElement).select()
                        }}
                        onkeydown={(e) => onKyaiKey(e, track.id, mi, ci)}
                        onblur={(e) => onKyaiBlur(e, track.id, mi, ci)}
                        oncontextmenu={(e) => {
                          e.preventDefault()
                          setKyai(track.id, mi, ci, null)
                          ;(e.currentTarget as HTMLInputElement).value = ''
                        }}
                        spellcheck="false"
                        autocomplete="off"
                      />
                    </div>
                  {/each}
                </div>
              {/each}
            </div>
          </div>
        {/if}
        {#if dynVisible(track.id)}
          <div class="row dyn-row track-dyn" role="presentation" onpointerdown={(e) => e.stopPropagation()}>
            {#if rails.length}
              <div class="rail-gutter" aria-hidden="true">
                {#each rails as rail (rail.person.id)}
                  <div class="rail spacer"></div>
                {/each}
              </div>
            {/if}
            <div class="track-head spacer"><span class="tl-hint">dinâmica · {track.name}</span></div>
            <div class="measures">
              {#each range(blk.start, blk.end) as mi (mi)}
                <div class="measure dyn" style:grid-template-columns={colTemplates[mi]}>
                  {@render dynCell(mi, track.id)}
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
    {/if}
  </section>
{/snippet}

<!-- marcação de dinâmica de um compasso: geral (trackId null) ou de um instrumento -->
{#snippet dynCell(mi: number, trackId: string | null)}
  {@const meas = store.song.measures[mi]}
  {@const mark = trackId ? meas.trackDynamics?.[trackId] : meas}
  <div class="dyn-cell">
    <select
      class="dyn-sel"
      class:set={!!mark?.dynamic}
      title={trackId ? 'Dinâmica deste instrumento a partir deste compasso (vence a geral)' : 'Dinâmica geral a partir deste compasso'}
      value={mark?.dynamic ?? ''}
      onchange={(e) => setMeasureDynamic(mi, ((e.currentTarget as HTMLSelectElement).value || null) as Dynamic | null, trackId)}
    >
      <option value="">·</option>
      {#each DYNAMICS as d (d)}<option value={d}>{d}</option>{/each}
    </select>
    <span class="hairpin" aria-hidden="true">
      {#if mark?.hairpin}
        <svg viewBox="0 0 100 12" preserveAspectRatio="none">
          {#if mark.hairpin === 'cresc'}<polyline points="100,1 0,6 100,11" />{:else}<polyline points="0,1 100,6 0,11" />{/if}
        </svg>
      {/if}
    </span>
    <button
      class="hp-btn"
      class:active={mark?.hairpin === 'cresc'}
      title="Crescendo neste compasso"
      aria-pressed={mark?.hairpin === 'cresc'}
      onclick={() => toggleMeasureHairpin(mi, 'cresc', trackId)}>&lt;</button
    >
    <button
      class="hp-btn"
      class:active={mark?.hairpin === 'dim'}
      title="Diminuendo neste compasso"
      aria-pressed={mark?.hairpin === 'dim'}
      onclick={() => toggleMeasureHairpin(mi, 'dim', trackId)}>&gt;</button
    >
  </div>
{/snippet}

{#if store.mode === 'select'}
  <div class="reassign-bar" role="toolbar" aria-label="Marcações das notas selecionadas">
    <span class="rb-label">notas selecionadas:</span>
    <button
      class="rb-chip sfz-chip"
      title="Sforzando: acento súbito no 1º golpe da nota, acima da dinâmica do trecho (atalho: !)"
      disabled={!sel}
      onclick={toggleSfzSel}><i>sfz</i> liga/desliga</button
    >
  </div>
{/if}

{#if store.mode === 'select' && rails.length}
  <div class="reassign-bar" role="toolbar" aria-label="Atribuir dono das notas selecionadas">
    <span class="rb-label">dono das notas selecionadas (só instrumentos compartilhados):</span>
    {#each rails as rail (rail.person.id)}
      <button class="rb-chip" style:--pc={rail.person.color ?? 'var(--accent)'} onclick={() => assignSelToPerson(rail.person.id)}>{rail.person.name}</button>
    {/each}
    <button class="rb-chip clear" onclick={() => assignSelToPerson(null)}>limpar</button>
  </div>
{/if}

<div class="grade-blocks" bind:this={gridRoot}>
  {#each blocks as blk (blk.id)}
    {@render blockView(blk)}
  {/each}
</div>

<datalist id="kyai-list">
  {#each KYAI_PRESETS as k (k)}<option value={k}></option>{/each}
</datalist>

{#if trackEditor}
  {@const te = trackEditor}
  {@const t = store.song.tracks.find((x) => x.id === te.id)}
  {#if t}
    <div class="track-editor" style:left={`${te.x}px`} style:top={`${te.y}px`} role="dialog" aria-label="Editar linha">
      <header class="te-head">
        <input class="te-name" value={t.name} oninput={(e) => renameTrack(t.id, (e.currentTarget as HTMLInputElement).value)} spellcheck="false" />
        <button class="te-x" title="Fechar" onclick={() => (trackEditor = null)}>×</button>
      </header>

      <label class="te-row">
        <span>instrumento</span>
        <select value={resolveInstrument(t).id} onchange={(e) => setTrackInstrument(t.id, (e.currentTarget as HTMLSelectElement).value)}>
          {#each palette as def (def.id)}<option value={def.id}>{def.name}</option>{/each}
        </select>
      </label>

      {#if (store.song.people?.length ?? 0)}
        <div class="te-block">
          <span class="te-lbl">pessoas</span>
          <div class="te-people">
            {#each store.song.people ?? [] as person (person.id)}
              <label class="te-chk" style:--pc={person.color ?? 'var(--accent)'}>
                <input type="checkbox" checked={(t.personIds ?? []).includes(person.id)} onchange={() => toggleTrackPerson(t.id, person.id)} />
                {person.name}
              </label>
            {/each}
          </div>
        </div>
      {/if}

      <div class="te-block">
        <span class="te-lbl">voz dos kiai 声 <small>(cada pessoa já tem voz própria; isto soma)</small></span>
        <label class="te-row te-range">
          <span>tom</span>
          <input type="range" min="-12" max="12" step="1" value={t.kyai?.pitch ?? KYAI_DEFAULTS.pitch}
            oninput={(e) => setKyaiVoice(t.id, 'pitch', +(e.currentTarget as HTMLInputElement).value)} />
          <output>{(t.kyai?.pitch ?? 0) > 0 ? '+' : ''}{t.kyai?.pitch ?? 0} st</output>
        </label>
        <label class="te-row te-range">
          <span>variação</span>
          <input type="range" min="0" max="1" step="0.05" value={t.kyai?.vary ?? KYAI_DEFAULTS.vary}
            oninput={(e) => setKyaiVoice(t.id, 'vary', +(e.currentTarget as HTMLInputElement).value)} />
          <output>{Math.round((t.kyai?.vary ?? KYAI_DEFAULTS.vary) * 100)}%</output>
        </label>
        <label class="te-row te-range">
          <span>volume</span>
          <input type="range" min="-24" max="18" step="1" value={t.kyai?.gain ?? KYAI_DEFAULTS.gain}
            oninput={(e) => setKyaiVoice(t.id, 'gain', +(e.currentTarget as HTMLInputElement).value)} />
          <output>{t.kyai?.gain ?? 0} dB</output>
        </label>
        <button class="te-test" onclick={() => testKyai(t.id)}>▶ testar grito</button>
      </div>

      <div class="te-row">
        <span>posição</span>
        <span class="te-move">
          <button title="Subir" onclick={() => moveTrack(t.id, -1)}>↑</button>
          <button title="Descer" onclick={() => moveTrack(t.id, 1)}>↓</button>
        </span>
      </div>

      <button class="te-del" onclick={() => { removeTrack(t.id); trackEditor = null }}>excluir linha</button>
    </div>
  {/if}
{/if}

<style>
  .score {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    overflow-x: auto;
    padding-bottom: 0.75rem;
    /* rolagem composta na GPU: sem isso o auto-scroll do playback repintava e
       re-layerizava a grade inteira a cada quadro (~50% do main thread) */
    will-change: scroll-position;
  }

  .row {
    display: flex;
    align-items: stretch;
    gap: 0.75rem;
    /* linhas fora da tela não são pintadas/layerizadas (com 90+ compassos cada edição
       repintava ~20k nós: ~1s por tecla). max-content: a linha tem a largura do
       conteúdo, senão a contenção cortava os compassos e sumia a rolagem horizontal;
       clip-margin deixa a barra da pessoa (.bar.extend) passar pro gap */
    content-visibility: auto;
    contain-intrinsic-size: auto 40px;
    overflow-clip-margin: 0.5rem;
    width: max-content;
    min-width: 100%;
  }

  .track-head {
    flex: 0 0 13rem;
    /* min-width:0 impede o item flex de crescer além de 11.5rem por causa do
       conteúdo (nome/instrumento longo empurrava as células); overflow corta */
    min-width: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.25rem 0.5rem;
    border-right: 3px solid var(--ink);
  }

  .track-head.spacer {
    border-right-color: transparent;
  }

  /* gutter de agrupamento: um colchete "[" por pessoa cobrindo as linhas dela.
     Cada linha desenha seu pedaço (borda esquerda + tampa no topo/base do trecho);
     `bridge` cobre o gap entre linhas do mesmo trecho. */
  .rail-gutter {
    flex: 0 0 auto;
    display: flex;
    gap: 0.3rem;
    padding-right: 0.4rem;
    align-items: stretch;
  }

  .rail {
    position: relative;
    width: 0.8rem;
    box-sizing: border-box;
  }

  /* barra simples: linha vertical contínua na cor da pessoa */
  .bar {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--pc);
    border-radius: 0;
  }

  .bar.cap-top {
    border-top-left-radius: 2px;
    border-top-right-radius: 2px;
  }

  .bar:not(.extend) {
    border-bottom-left-radius: 2px;
    border-bottom-right-radius: 2px;
  }

  /* linha seguinte é da mesma pessoa: estende sobre o gap de 0.4rem → linha única */
  .bar.extend {
    bottom: -0.42rem;
  }

  /* nome vertical (rotacionado à esquerda) no início do trecho */
  .rail-name {
    position: absolute;
    top: 0.15rem;
    left: 4px;
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    font-size: 0.62rem;
    font-weight: 800;
    color: var(--pc);
    white-space: nowrap;
    pointer-events: none;
    z-index: 2;
  }

  /* barra de reatribuição de dono (modo select) */
  .reassign-bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.35rem;
    margin-bottom: 0.8rem;
    padding: 0.4rem 0.55rem;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: 8px;
  }

  .rb-label {
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--ink-soft);
    margin-right: 0.2rem;
  }

  .rb-chip {
    font: inherit;
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.25rem 0.7rem;
    border-radius: 999px;
    border: 1px solid var(--pc, var(--line-strong));
    background: color-mix(in srgb, var(--pc) 14%, transparent);
    color: var(--ink);
    cursor: pointer;
  }

  .rb-chip:hover {
    background: color-mix(in srgb, var(--pc) 26%, transparent);
  }

  .rb-chip.clear {
    border-color: var(--line-strong);
    background: var(--paper-raise);
    color: var(--ink-soft);
  }

  /* popover de edição da linha */
  .track-editor {
    position: fixed;
    z-index: 60;
    width: 15rem;
    background: var(--paper);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: 0 10px 34px rgba(0, 0, 0, 0.28);
    padding: 0.6rem 0.7rem 0.7rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .te-head {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .te-name {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-weight: 800;
    font-size: 0.85rem;
    color: var(--ink);
    background: transparent;
    border: none;
    border-bottom: 1px dashed var(--line-strong);
  }

  .te-name:focus-visible {
    outline: none;
    border-bottom-color: var(--accent);
  }

  .te-x {
    font: inherit;
    font-size: 1rem;
    border: none;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    padding: 0 0.15rem;
  }

  .te-x:hover {
    color: var(--accent);
  }

  .te-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: var(--ink-soft);
    font-weight: 700;
  }

  .te-row select {
    flex: 1;
    font: inherit;
    font-size: 0.76rem;
    padding: 0.25rem 0.35rem;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--paper-raise);
    color: var(--ink);
    max-width: 9rem;
  }

  .te-move {
    display: inline-flex;
    gap: 0.25rem;
  }

  .te-move button {
    font: inherit;
    font-weight: 700;
    width: 1.7rem;
    height: 1.6rem;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    background: var(--paper-raise);
    color: var(--ink-soft);
    cursor: pointer;
  }

  .te-move button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .te-lbl {
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--ink-soft);
  }

  .te-people {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    margin-top: 0.25rem;
  }

  .te-chk {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.78rem;
    color: var(--ink);
    cursor: pointer;
  }

  .te-chk input {
    accent-color: var(--pc);
  }

  .te-del {
    font: inherit;
    font-size: 0.74rem;
    font-weight: 700;
    padding: 0.3rem 0.6rem;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--paper-raise);
    color: var(--ink-soft);
    cursor: pointer;
    margin-top: 0.2rem;
  }

  .te-del:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .track-name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .track-name .name {
    font-weight: 700;
    font-size: 0.85rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* com um "main" ativo, os outros instrumentos aparecem recuados */
  .track-name.recessed {
    opacity: 0.45;
  }

  .track-name .inst {
    font-size: 0.68rem;
    color: var(--ink-soft);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-ctl {
    display: flex;
    gap: 0.15rem;
    flex: 0 0 auto;
  }

  .mini {
    font: inherit;
    font-size: 0.65rem;
    font-weight: 700;
    width: 1.3rem;
    height: 1.4rem;
    padding: 0;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--ink-soft);
    border-radius: 3px;
    cursor: pointer;
  }

  .mini.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--paper);
  }

  .mini.danger:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .measures {
    display: flex;
    gap: 0.5rem;
  }

  .measure {
    display: grid;
    grid-template-columns: repeat(var(--cells), var(--cellw, 3.4rem));
    border-left: 2px solid var(--ink);
    border-right: 2px solid var(--ink);
    position: relative;
  }

  .measure.counts {
    border-color: transparent;
  }

  .measure.dyn {
    border-color: transparent;
  }

  .track-dyn {
    margin-top: -0.1rem;
    margin-bottom: 0.15rem;
  }

  .track-dyn .tl-hint {
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-dyn .measure.dyn {
    background: color-mix(in srgb, var(--ink) 4%, transparent);
    border-radius: 3px;
  }

  .kyai-row {
    margin-top: -0.1rem;
    margin-bottom: 0.15rem;
  }

  .kyai-head {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.15rem;
  }

  .kyai-presets {
    font: inherit;
    font-size: 0.68rem;
    max-width: 100%;
    padding: 0 0.2rem;
    border: 1px solid var(--line);
    border-radius: 3px;
    background: var(--paper-raise);
    color: var(--ink);
  }

  .measure.kyai {
    border-color: transparent;
    background: color-mix(in srgb, var(--accent) 6%, transparent);
    border-radius: 3px;
  }

  .kbox {
    position: relative;
    height: 1.7rem;
    border-left: 1px dashed var(--line);
  }

  .kbox.beat {
    border-left: 1px solid var(--line-strong);
  }

  .kbox:global(.flash) {
    background: color-mix(in srgb, var(--accent) 35%, transparent);
  }

  input.kcell {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    border: none;
    background: transparent;
    font-family: 'Zen Kaku Gothic New', inherit;
    font-size: 0.78rem;
    font-style: italic;
    text-align: center;
    color: var(--ink-soft);
    padding: 0;
  }

  input.kcell::-webkit-calendar-picker-indicator {
    display: none !important;
  }

  .measure.narrow input.kcell {
    font-size: 0.6rem;
  }

  .kbox.filled input.kcell {
    color: var(--ink);
  }

  .kbox.shout input.kcell {
    color: var(--accent);
    font-weight: 700;
  }

  input.kcell:focus {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    background: var(--paper-raise);
    font-style: normal;
  }

  input.kcell:global(.invalid) {
    background: color-mix(in srgb, #d33 30%, transparent);
  }

  .mini.kyai-toggle {
    font-family: 'Zen Kaku Gothic New', serif;
    font-size: 0.7rem;
  }

  .mini.kyai-toggle:disabled {
    cursor: default;
    opacity: 1;
  }

  .te-range input[type='range'] {
    flex: 1;
    min-width: 0;
  }

  .te-range output {
    font-size: 0.7rem;
    min-width: 3.2rem;
    text-align: right;
    color: var(--ink-soft);
  }

  .te-lbl small {
    font-weight: 400;
    color: var(--ink-soft);
  }

  .te-test {
    font: inherit;
    font-size: 0.75rem;
    margin-top: 0.2rem;
  }

  .mini.dyn-toggle i {
    font-family: Georgia, 'Times New Roman', serif;
    font-weight: 700;
  }

  .mini.dyn-toggle:disabled {
    cursor: default;
    opacity: 1;
  }

  .dyn-cell {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 0.2rem;
    min-width: 0;
    padding: 0.1rem 0.15rem;
  }

  .dyn-sel {
    font-family: Georgia, 'Times New Roman', serif;
    font-style: italic;
    font-weight: 700;
    font-size: 0.8rem;
    color: var(--ink-faint);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 3px;
    padding: 0 0.1rem;
    cursor: pointer;
  }

  .dyn-sel:hover,
  .dyn-sel:focus-visible {
    border-color: var(--line-strong);
  }

  .dyn-sel.set {
    color: var(--ink);
  }

  .hairpin {
    flex: 1;
    min-width: 0;
    height: 0.75rem;
    display: flex;
  }

  .hairpin svg {
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .hairpin polyline {
    fill: none;
    stroke: var(--ink);
    stroke-width: 1.4;
    vector-effect: non-scaling-stroke;
  }

  .hp-btn {
    font: inherit;
    font-size: 0.65rem;
    font-weight: 700;
    width: 1.2rem;
    height: 1.2rem;
    padding: 0;
    border: 1px solid var(--line);
    border-radius: 3px;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
  }

  .hp-btn:hover {
    color: var(--ink);
    border-color: var(--line-strong);
  }

  .hp-btn.active {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--paper);
  }

  .sec-repeat {
    display: inline-flex;
    align-items: center;
    gap: 0.15rem;
    color: var(--ink-faint);
    font-size: 0.75rem;
  }

  .sec-repeat.on {
    color: var(--accent);
  }

  .sec-repeat .rp-mark {
    font-size: 1.05rem;
    line-height: 1;
  }

  .sec-repeat .rp-n {
    font-weight: 800;
    min-width: 1.6rem;
    text-align: center;
    color: var(--ink);
  }

  .sec-repeat.on .rp-n {
    color: var(--accent);
  }

  .sec-repeat button {
    font: inherit;
    font-size: 0.75rem;
    font-weight: 700;
    width: 1.3rem;
    height: 1.3rem;
    padding: 0;
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    background: var(--paper-raise);
    color: var(--ink-soft);
    cursor: pointer;
  }

  .sec-repeat button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .grade-blocks {
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
  }

  .section-block {
    /* seção fora da tela: pula render (tamanho estimado até a 1ª vez que aparece) */
    content-visibility: auto;
    contain-intrinsic-size: auto 500px;
    border: 2px solid transparent;
    border-radius: 8px;
    padding: 0.3rem 0.5rem 0.5rem;
    transition:
      border-color 120ms ease,
      background 120ms ease;
  }

  .section-block.active {
    border-color: var(--accent);
    background: var(--accent-wash);
  }

  .section-block.playing {
    box-shadow: 0 0 0 2px var(--accent);
  }

  .section-block.dragging {
    opacity: 0.55;
    border-color: var(--line-strong);
    border-style: dashed;
  }

  .sec-handle {
    font: inherit;
    font-size: 0.95rem;
    line-height: 1;
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    background: var(--paper-raise);
    color: var(--ink-soft);
    cursor: grab;
    padding: 0.1rem 0.35rem;
    touch-action: none;
  }

  .sec-handle:active {
    cursor: grabbing;
  }

  .block-head {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.3rem;
    padding-left: 0.25rem;
  }

  .sec-name-static {
    font-weight: 700;
    font-size: 0.85rem;
    color: var(--ink-soft);
  }

  .section-block.active .block-head {
    /* destaque do header quando selecionada */
    border-radius: 4px;
  }

  .sec-min {
    font: inherit;
    font-size: 0.8rem;
    line-height: 1;
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    background: var(--paper-raise);
    color: var(--ink-soft);
    cursor: pointer;
    padding: 0.1rem 0.4rem;
  }

  .section-block.collapsed {
    padding-bottom: 0.3rem;
    background: var(--paper-raise);
    border-color: var(--line-strong);
  }

  .section-block.collapsed.active {
    border-color: var(--accent);
  }

  .section-block.collapsed .block-head {
    margin-bottom: 0;
  }

  .sec-name {
    font: inherit;
    font-size: 0.9rem;
    font-weight: 800;
    color: var(--ink);
    background: transparent;
    border: none;
    border-bottom: 1px dashed transparent;
    min-width: 6rem;
  }

  .sec-name:hover,
  .sec-name:focus-visible {
    border-bottom-color: var(--line-strong);
    outline: none;
  }

  .sec-del {
    font: inherit;
    font-size: 0.9rem;
    font-weight: 700;
    border: none;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    padding: 0 0.2rem;
    margin-left: auto;
  }

  .sec-clone {
    font: inherit;
    font-size: 0.72rem;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: transparent;
    color: var(--ink-soft);
    cursor: pointer;
    padding: 0.05rem 0.4rem;
    margin-left: auto;
  }

  .sec-clone:hover {
    color: var(--accent);
    border-color: var(--accent);
  }

  .sec-clone + .sec-del {
    margin-left: 0.2rem;
  }

  .sec-del:hover {
    color: var(--accent);
  }

  .blk-compasso {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    margin-left: 0.4rem;
  }

  .blk-compasso .cl {
    font-size: 0.65rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--ink-soft);
    font-weight: 700;
    margin-right: 0.1rem;
  }

  .blk-compasso button {
    font: inherit;
    font-size: 0.85rem;
    font-weight: 700;
    width: 1.7rem;
    height: 1.6rem;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    background: var(--paper-raise);
    color: var(--ink-soft);
    cursor: pointer;
  }

  .blk-compasso button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .timeline {
    cursor: grab;
    touch-action: none;
    user-select: none;
  }

  .timeline:active {
    cursor: grabbing;
  }

  .tl-hint {
    font-size: 0.6rem;
    color: var(--ink-faint);
    white-space: nowrap;
  }

  .count {
    position: relative;
    text-align: center;
    font-size: 0.7rem;
    color: var(--ink-soft);
    padding-bottom: 0.15rem;
  }

  .count.beat {
    font-weight: 700;
    color: var(--ink);
  }

  /* playhead: triângulo no topo da coluna de início */
  .count.start::before {
    content: '';
    position: absolute;
    left: 0;
    top: -2px;
    transform: translateX(-50%);
    border-left: 5px solid transparent;
    border-right: 5px solid transparent;
    border-top: 7px solid var(--accent);
    z-index: 3;
  }

  /* célula = caixa (estado visual) + camada de golpes + input transparente */
  .cellbox {
    position: relative;
    height: 2.6rem;
    border: 1px solid var(--line);
    border-left-width: 0;
    background: var(--paper-raise);
    overflow: hidden;
  }

  .cellbox:first-child {
    border-left-width: 1px;
  }

  .cellbox.beat {
    border-left: 2px solid var(--line-strong);
  }

  .cellbox.filled {
    background: var(--paper);
  }

  /* instrumento compartilhado: barra no topo na cor do dono da nota */
  .cellbox.owner-hint.filled::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: var(--owner);
    z-index: 2;
  }

  /* dono mudo (submix silenciado): nota transparente */
  .cellbox.owner-muted .hits,
  .cellbox.owner-muted input.cell {
    opacity: 0.28;
  }

  .cellbox:hover {
    background: var(--paper-hover);
  }

  .cellbox:global(.cursor) {
    background: var(--accent-wash);
    box-shadow: inset 0 -3px 0 var(--accent);
  }

  .cellbox.start {
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .cellbox:global(.cursor).start {
    box-shadow: inset 3px 0 0 var(--accent), inset 0 -3px 0 var(--accent);
  }

  /* sub-células de golpe (faixa inferior): 1 por nota tocada */
  .hits {
    position: absolute;
    left: 2px;
    right: 2px;
    bottom: 2px;
    height: 0.85rem;
    display: flex;
    gap: 1px;
    z-index: 0;
    pointer-events: none;
  }

  .hit {
    flex: 1 1 0;
    min-width: 0;
    border-radius: 2px 2px 3px 3px;
    position: relative;
  }

  .hit.couro {
    background: var(--accent);
  }

  .hit.fuchi {
    background: var(--accent-2);
  }

  /* sforzando: selo no canto da célula */
  .sfz-badge {
    position: absolute;
    top: 1px;
    right: 3px;
    z-index: 2;
    font-family: Georgia, 'Times New Roman', serif;
    font-style: italic;
    font-weight: 700;
    font-size: 0.6rem;
    line-height: 1;
    color: var(--accent);
    pointer-events: none;
  }

  .sfz-chip i {
    font-family: Georgia, 'Times New Roman', serif;
    font-weight: 700;
  }

  .rb-chip:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* nota forte: símbolo de acento dentro da sub-célula */
  .hit.accent::after {
    content: '›';
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 0.62rem;
    font-weight: 800;
    line-height: 1;
    color: #fff;
  }

  /* duas mãos: borda na sub-célula */
  .hit.both {
    box-shadow: inset 0 0 0 2px var(--paper);
    outline: 1px solid var(--ink);
    outline-offset: -1px;
  }

  input.cell {
    position: absolute;
    inset: 0;
    z-index: 1;
    font-family: inherit;
    width: 100%;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--ink);
    font-size: 0.82rem;
    font-weight: 500;
    text-align: center;
    padding: 0 0 0.85rem;
    box-sizing: border-box;
    border-radius: 0;
  }

  .measure.narrow input.cell {
    font-size: 0.62rem;
  }

  input.cell.long {
    font-size: 0.58rem;
    letter-spacing: -0.02em;
  }

  /* enquanto edita, o input fica opaco pra digitar limpo sobre os golpes */
  input.cell:focus,
  input.cell:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    background: var(--paper);
    padding-bottom: 0;
  }

  .cellbox:global(.cursor) .cell {
    color: var(--accent);
    font-weight: 700;
  }

  .score.panning {
    cursor: grabbing;
  }

  .score.panning .cellbox {
    pointer-events: none;
  }

  .score.selectmode input.cell {
    cursor: cell;
    user-select: none;
  }

  .score.selectmode .cellbox.sel {
    background: var(--accent-wash);
    box-shadow: inset 0 0 0 1px var(--accent);
  }

  .score.selectmode .cellbox.selsrc {
    opacity: 0.4;
  }

  .score.selectmode .cellbox.movet {
    background: var(--accent-wash);
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  input.cell:global(.invalid) {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .cellbox:global(.flash) {
    animation: cell-flash 0.9s ease-out;
  }

  @keyframes cell-flash {
    0% {
      background: var(--accent);
    }
    45% {
      background: var(--accent-wash);
    }
    100% {
      background: var(--paper);
    }
  }
</style>
