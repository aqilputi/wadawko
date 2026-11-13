import { mainTracks } from './timeline'
import type { Song, Track, InstrumentId, Measure, Section, Person, InstrumentDef, ZoneParams, Dynamic, DynamicMark } from './types'
import { cellsPerMeasure, INSTRUMENTS, PERSON_COLORS, BUILTIN_INSTRUMENTS } from './types'
import { demoSong } from './demo'
import { loadDb, dbGet, dbSet, dbRemove } from './db'

await loadDb()

const WS_KEY = 'wadawko:workspace'
const projKey = (id: string) => `wadawko:project:${id}`
const LEGACY_KEY = 'taiko-fu:song'

export interface ProjectMeta {
  id: string
  title: string
  updatedAt: number
}
interface Workspace {
  version: 2
  projects: ProjectMeta[]
  currentId: string | null
}

let idCounter = 0
function genId() {
  idCounter++
  return `p-${Date.now().toString(36)}-${idCounter}`
}

function readSong(id: string): Song | null {
  try {
    const raw = dbGet(projKey(id))
    if (!raw) return null
    const s = JSON.parse(raw)
    return s?.version === 1 ? (s as Song) : null
  } catch {
    return null
  }
}

function readWorkspace(): Workspace | null {
  try {
    const raw = dbGet(WS_KEY)
    if (!raw) return null
    const w = JSON.parse(raw)
    return w?.version === 2 && Array.isArray(w.projects) ? (w as Workspace) : null
  } catch {
    return null
  }
}

/** carrega o workspace; no 1º boot migra o save antigo (taiko-fu:song) ou cria demo */
function bootstrap(): { workspace: Workspace; song: Song } {
  const ws = readWorkspace()
  if (ws?.currentId) {
    const song = readSong(ws.currentId)
    if (song) return { workspace: ws, song }
  }
  let legacy: Song | null = null
  try {
    const raw = localStorage.getItem(LEGACY_KEY)
    if (raw) {
      const s = JSON.parse(raw)
      if (s?.version === 1) legacy = s as Song
    }
  } catch {
    /* ignora */
  }
  const song = legacy ?? demoSong()
  const id = genId()
  const workspace: Workspace = {
    version: 2,
    projects: [{ id, title: song.title || 'Sem título', updatedAt: Date.now() }],
    currentId: id,
  }
  dbSet(projKey(id), JSON.stringify(song))
  dbSet(WS_KEY, JSON.stringify(workspace))
  return { workspace, song }
}

const boot = bootstrap()

export const store = $state({
  workspace: boot.workspace as Workspace,
  song: boot.song,
  /** incrementa a cada troca/novo/exclusão de projeto (App reseta o histórico) */
  projectEpoch: 0,
  selectedSyl: 'don' as string,
  playing: false,
  loop: true,
  cursor: -1,
  /** célula global onde o playback começa (playhead da timeline) */
  startCell: 0,
  /** edit = digitar nas células · select = mover/copiar blocos */
  mode: 'edit' as 'edit' | 'select',
  /** uma célula está focada (mostra a paleta flutuante) */
  editing: false,
  /** seções selecionadas (vazio = música inteira); playback usa o intervalo delas */
  selectedSections: [] as string[],
  /** metrônomo tocando junto */
  metronome: false,
  /** count-in de 1 compasso antes de tocar */
  countIn: false,
  /** treino: rampa de tempo em loop (acelera a cada repetição) */
  trainerOn: false,
  /** fração do tempo base onde o treino começa (0.5 = 50%) */
  trainerStartPct: 0.7,
  /** incremento por repetição, fração do tempo base (0.05 = +5%) */
  trainerStep: 0.05,
  /** BPM efetivo durante o playback (rampa do treino mostra aqui) */
  playbackBpm: boot.song.tempo,
  /** página atual (nav rail à esquerda) */
  page: 'editor' as 'editor' | 'mixer' | 'people' | 'kit',
  /** zoom horizontal da grade (1 = normal) */
  zoom: 1,
  /** caneta: pessoa dona das notas digitadas em track compartilhada (null = nenhuma) */
  activePersonId: null as string | null,
})

function persistWorkspace() {
  dbSet(WS_KEY, JSON.stringify(store.workspace))
}

function resetUiForSong() {
  store.selectedSections = []
  store.startCell = 0
  store.cursor = -1
  store.playing = false
  store.projectEpoch++
}

export function switchProject(id: string) {
  if (id === store.workspace.currentId) return
  save() // salva o atual antes de trocar
  const song = readSong(id)
  if (!song) return
  store.workspace.currentId = id
  store.song = song
  resetUiForSong()
  persistWorkspace()
}

export function newProject() {
  save()
  const song = demoSong()
  const id = genId()
  store.workspace.projects.push({ id, title: song.title || 'Sem título', updatedAt: Date.now() })
  store.workspace.currentId = id
  store.song = song
  dbSet(projKey(id), JSON.stringify(song))
  resetUiForSong()
  persistWorkspace()
}

export function duplicateProject(id: string) {
  const src = id === store.workspace.currentId ? store.song : readSong(id)
  if (!src) return
  save()
  const copy: Song = JSON.parse(JSON.stringify(src))
  copy.title = `${src.title || 'Sem título'} cópia`
  const nid = genId()
  store.workspace.projects.push({ id: nid, title: copy.title, updatedAt: Date.now() })
  store.workspace.currentId = nid
  store.song = copy
  dbSet(projKey(nid), JSON.stringify(copy))
  resetUiForSong()
  persistWorkspace()
}

export function deleteProject(id: string) {
  dbRemove(projKey(id))
  store.workspace.projects = store.workspace.projects.filter((p) => p.id !== id)
  if (store.workspace.currentId === id) {
    const next = store.workspace.projects[0]
    if (next) {
      store.workspace.currentId = next.id
      store.song = readSong(next.id) ?? demoSong()
    } else {
      const song = demoSong()
      const nid = genId()
      store.workspace.projects.push({ id: nid, title: song.title || 'Sem título', updatedAt: Date.now() })
      store.workspace.currentId = nid
      store.song = song
      dbSet(projKey(nid), JSON.stringify(song))
    }
    resetUiForSong()
  }
  persistWorkspace()
}

export interface SectionRange extends Section {
  /** último compasso da seção (inclusivo) */
  end: number
}

/** seções ordenadas por compasso inicial, com o compasso final calculado */
export function sectionRanges(): SectionRange[] {
  const secs = [...(store.song.sections ?? [])].sort((a, b) => a.start - b.start)
  return secs.map((s, i) => ({
    ...s,
    end: (secs[i + 1]?.start ?? store.song.measures.length) - 1,
  }))
}

export function sectionOfMeasure(mi: number): SectionRange | null {
  return sectionRanges().find((s) => mi >= s.start && mi <= s.end) ?? null
}

/** seção alvo pra +/− compasso: a última das selecionadas (null = sem seção) */
function selectedTargetSection(): SectionRange | null {
  if (!store.selectedSections.length) return null
  const ranges = sectionRanges().filter((r) => store.selectedSections.includes(r.id))
  if (!ranges.length) return null
  return ranges.reduce((a, b) => (b.end > a.end ? b : a))
}

function makeMeasure(beats: number, subdivision: number): Measure {
  const cells: Measure['cells'] = {}
  for (const t of store.song.tracks) cells[t.id] = Array(beats * subdivision).fill(null)
  return { beats, subdivision, cells }
}

/** intervalo de compassos [startBar, endBar) que cobre as seções selecionadas (null = tudo) */
export function selectedRegion(): { startBar: number; endBar: number } | null {
  if (!store.selectedSections.length) return null
  const ranges = sectionRanges().filter((r) => store.selectedSections.includes(r.id))
  if (!ranges.length) return null
  return {
    startBar: Math.min(...ranges.map((r) => r.start)),
    endBar: Math.max(...ranges.map((r) => r.end)) + 1,
  }
}

let sectionCounter = 0

function newSectionId() {
  sectionCounter++
  return `sec-${Date.now().toString(36)}-${sectionCounter}`
}

/**
 * Adiciona uma seção SEMPRE no fim. A primeira envolve a música já existente
 * (start 0); as seguintes ganham um compasso novo próprio no fim.
 */
export function appendSection(name?: string) {
  if (!store.song.sections) store.song.sections = []
  const count = store.song.sections.length
  if (count === 0) {
    store.song.sections.push({ id: newSectionId(), name: name ?? 'Seção 1', start: 0 })
    return
  }
  const start = store.song.measures.length
  const ref = store.song.measures.at(-1)
  store.song.measures.push(makeMeasure(ref?.beats ?? 4, ref?.subdivision ?? 2)) // bloco próprio no fim
  store.song.sections.push({ id: newSectionId(), name: name ?? `Seção ${count + 1}`, start })
}

/**
 * Clona a seção `id`: copia os compassos (notas, donos, dinâmicas, kiai) e
 * insere a cópia logo depois da original, como seção nova "<nome> (cópia)".
 */
export function cloneSection(id: string): string | null {
  const r = sectionRanges().find((x) => x.id === id)
  if (!r) return null
  const copy: Measure[] = JSON.parse(JSON.stringify(store.song.measures.slice(r.start, r.end + 1)))
  const at = r.end + 1
  store.song.measures.splice(at, 0, ...copy)
  for (const s of store.song.sections ?? []) if (s.start >= at) s.start += copy.length
  const newId = newSectionId()
  const sections = store.song.sections ?? []
  const idx = sections.findIndex((s) => s.id === id)
  sections.splice(idx + 1, 0, { id: newId, name: `${r.name} (cópia)`, start: at, ...(r.repeat ? { repeat: r.repeat } : {}) })
  store.song.sections = sections
  return newId
}

/** move a seção `id` para a posição `targetIndex` (na ordem), levando os compassos junto */
export function moveSection(id: string, targetIndex: number) {
  const ranges = sectionRanges()
  const from = ranges.findIndex((r) => r.id === id)
  if (from < 0) return
  const preLen = ranges[0]?.start ?? 0 // compassos antes da 1ª seção ficam fixos no topo
  const pre = store.song.measures.slice(0, preLen)
  const blocks = ranges.map((r) => store.song.measures.slice(r.start, r.end + 1))
  const metas = ranges.map((r) => ({ id: r.id, name: r.name, repeat: r.repeat }))
  const [mb] = blocks.splice(from, 1)
  const [ms] = metas.splice(from, 1)
  const t = Math.max(0, Math.min(metas.length, targetIndex))
  blocks.splice(t, 0, mb)
  metas.splice(t, 0, ms)
  const newMeasures: Measure[] = [...pre]
  const newSections: Section[] = []
  let acc = preLen
  for (let k = 0; k < metas.length; k++) {
    newSections.push({ id: metas[k].id, name: metas[k].name, start: acc, ...(metas[k].repeat ? { repeat: metas[k].repeat } : {}) })
    newMeasures.push(...blocks[k])
    acc += blocks[k].length
  }
  store.song.measures = newMeasures
  store.song.sections = newSections
}

export function renameSection(id: string, name: string) {
  const s = store.song.sections?.find((x) => x.id === id)
  if (s) s.name = name
}

/** ritornello: quantas vezes a seção toca (1 = sem repetição) */
export function setSectionRepeat(id: string, times: number) {
  const s = store.song.sections?.find((x) => x.id === id)
  if (!s) return
  const n = Math.max(1, Math.min(16, Math.round(times)))
  if (n === 1) delete s.repeat
  else s.repeat = n
}

/** alvo da marcação: o compasso (geral) ou a entrada do instrumento nele */
function dynTarget(mi: number, trackId: string | null, create: boolean): DynamicMark | null {
  const m = store.song.measures[mi]
  if (!m) return null
  if (!trackId) return m
  if (!m.trackDynamics?.[trackId]) {
    if (!create) return null
    m.trackDynamics = { ...(m.trackDynamics ?? {}), [trackId]: {} }
  }
  return m.trackDynamics![trackId]
}

/** remove a entrada do instrumento se ficou vazia (não suja o JSON) */
function pruneDyn(mi: number, trackId: string | null) {
  const m = store.song.measures[mi]
  if (!trackId || !m?.trackDynamics) return
  const e = m.trackDynamics[trackId]
  if (e && !e.dynamic && !e.hairpin) delete m.trackDynamics[trackId]
  if (!Object.keys(m.trackDynamics).length) delete m.trackDynamics
}

/** marcação de dinâmica (pp..ff) no início do compasso; null remove. trackId = só aquele instrumento */
export function setMeasureDynamic(mi: number, dyn: Dynamic | null, trackId: string | null = null) {
  const t = dynTarget(mi, trackId, !!dyn)
  if (!t) return
  if (dyn) t.dynamic = dyn
  else delete t.dynamic
  pruneDyn(mi, trackId)
}

/** alterna crescendo/diminuendo no compasso (mesmo valor = remove) */
export function toggleMeasureHairpin(mi: number, hp: 'cresc' | 'dim', trackId: string | null = null) {
  const t = dynTarget(mi, trackId, true)
  if (!t) return
  if (t.hairpin === hp) delete t.hairpin
  else t.hairpin = hp
  pruneDyn(mi, trackId)
}

/** instrumento tem alguma marcação própria de dinâmica? */
export function trackHasDynamics(trackId: string): boolean {
  return store.song.measures.some((m) => !!m.trackDynamics?.[trackId])
}

/** remove a seção E os compassos dela (antes só sumia o marcador e os compassos viravam silêncio na seção anterior) */
export function removeSection(id: string) {
  if (!store.song.sections) return
  const range = sectionRanges().find((r) => r.id === id)
  store.song.sections = store.song.sections.filter((s) => s.id !== id)
  store.selectedSections = store.selectedSections.filter((s) => s !== id)
  if (!range) return
  const count = range.end - range.start + 1
  const ref = store.song.measures[range.start]
  store.song.measures.splice(range.start, count)
  for (const s of store.song.sections) if (s.start > range.start) s.start -= count
  // música não fica sem compasso
  if (!store.song.measures.length) store.song.measures.push(makeMeasure(ref?.beats ?? 4, ref?.subdivision ?? 2))
  store.startCell = 0
}

/** coluna global (0..total-1) → posição (compasso, célula) */
export function colToPos(globalCol: number): { mi: number; ci: number } | null {
  let acc = 0
  for (let mi = 0; mi < store.song.measures.length; mi++) {
    const n = cellsPerMeasure(store.song.measures[mi])
    if (globalCol < acc + n) return { mi, ci: globalCol - acc }
    acc += n
  }
  return null
}

export function getCellCol(trackId: string, globalCol: number): string | null {
  const p = colToPos(globalCol)
  return p ? getCell(trackId, p.mi, p.ci) : null
}

export function setCellCol(trackId: string, globalCol: number, syl: string | null) {
  const p = colToPos(globalCol)
  if (p) setCell(trackId, p.mi, p.ci, syl)
}

export function totalCols(): number {
  return store.song.measures.reduce((s, m) => s + cellsPerMeasure(m), 0)
}

/** garante que existam pelo menos `minCols` colunas, criando compassos ao fim */
export function ensureCols(minCols: number) {
  let guard = 0
  while (totalCols() < minCols && guard++ < 64) addMeasure()
}

/** `json`: a música já serializada (evita um 2º JSON.stringify quando quem chama já tem) */
export function save(json?: string) {
  const id = store.workspace.currentId
  if (!id) return
  dbSet(projKey(id), json ?? JSON.stringify(store.song))
  const meta = store.workspace.projects.find((p) => p.id === id)
  if (meta) {
    meta.title = store.song.title || 'Sem título'
    meta.updatedAt = Date.now()
  }
  persistWorkspace()
}

/**
 * Carimba o dono da célula ao escrever numa track compartilhada: nota nova ganha
 * a pessoa ativa (caneta); célula limpa perde o dono. Não faz nada se não for
 * compartilhada.
 */
function stampOwner(trackId: string, mi: number, ci: number, syl: string | null) {
  const track = store.song.tracks.find((t) => t.id === trackId)
  if (!track || !isShared(track)) return
  if (syl === null) assignCellOwner(trackId, mi, ci, null)
  else if (store.activePersonId && (track.personIds ?? []).includes(store.activePersonId))
    assignCellOwner(trackId, mi, ci, store.activePersonId)
}

export function setCell(trackId: string, measureIdx: number, cellIdx: number, syl: string | null) {
  const meas = store.song.measures[measureIdx]
  if (!meas) return
  if (!meas.cells[trackId]) meas.cells[trackId] = Array(cellsPerMeasure(meas)).fill(null)
  meas.cells[trackId][cellIdx] = syl
  stampOwner(trackId, measureIdx, cellIdx, syl)
}

export function getCell(trackId: string, measureIdx: number, cellIdx: number): string | null {
  return store.song.measures[measureIdx]?.cells[trackId]?.[cellIdx] ?? null
}

/* ── kiai (sub-linha de gritos por instrumento) ── */

export function getKyai(trackId: string, mi: number, ci: number): string | null {
  return store.song.measures[mi]?.kyai?.[trackId]?.[ci] ?? null
}

export function setKyai(trackId: string, mi: number, ci: number, syl: string | null) {
  const meas = store.song.measures[mi]
  if (!meas) return
  if (!meas.kyai) meas.kyai = {}
  if (!meas.kyai[trackId]) meas.kyai[trackId] = Array(cellsPerMeasure(meas)).fill(null)
  meas.kyai[trackId][ci] = syl
  // linha toda vazia: some do JSON
  if (meas.kyai[trackId].every((x) => !x)) delete meas.kyai[trackId]
  if (!Object.keys(meas.kyai).length) delete meas.kyai
}

export function trackHasKyai(trackId: string): boolean {
  return store.song.measures.some((m) => m.kyai?.[trackId]?.some(Boolean))
}

export interface CellPos {
  mi: number
  ci: number
}

export interface FillResult {
  /** todas as células tocadas (escritas ou limpas) — p/ flash visual */
  written: CellPos[]
  /** primeira célula DEPOIS da inserção — p/ mover o foco */
  next: CellPos | null
}

function normalizePos(m: number, c: number): CellPos | null {
  while (m < store.song.measures.length && c >= cellsPerMeasure(store.song.measures[m])) {
    m++
    c = 0
  }
  return m < store.song.measures.length ? { mi: m, ci: c } : null
}

/** Preenche células consecutivas (atravessando compassos) a partir de (mi, ci). */
export function fillSequential(trackId: string, mi: number, ci: number, syls: string[]): FillResult {
  const written: CellPos[] = []
  let m = mi
  let c = ci
  for (const syl of syls) {
    const pos = normalizePos(m, c)
    if (!pos) break
    ;({ mi: m, ci: c } = pos)
    const meas = store.song.measures[m]
    if (!meas.cells[trackId]) meas.cells[trackId] = Array(cellsPerMeasure(meas)).fill(null)
    meas.cells[trackId][c] = syl
    stampOwner(trackId, m, c, syl)
    written.push({ mi: m, ci: c })
    c++
  }
  return { written, next: normalizePos(m, c) }
}

/**
 * Escreve uma palavra POR TEMPO a partir de (mi, ci), sobrescrevendo o span
 * inteiro de cada tempo (célula da palavra + subdivisões restantes limpas).
 * É o modo das frases do catálogo: "don doko don don" = 4 tempos.
 */
export function fillPhraseBeats(trackId: string, mi: number, ci: number, words: string[]): FillResult {
  const written: CellPos[] = []
  let m = mi
  // frase é coisa de tempo: ancora no início do tempo da célula focada
  const sub = store.song.measures[mi]?.subdivision ?? 2
  let c = Math.floor(ci / sub) * sub
  for (const word of words) {
    const pos = normalizePos(m, c)
    if (!pos) break
    ;({ mi: m, ci: c } = pos)
    const meas = store.song.measures[m]
    const n = cellsPerMeasure(meas)
    if (!meas.cells[trackId]) meas.cells[trackId] = Array(n).fill(null)
    meas.cells[trackId][c] = word
    stampOwner(trackId, m, c, word)
    written.push({ mi: m, ci: c })
    for (let k = 1; k < meas.subdivision && c + k < n; k++) {
      meas.cells[trackId][c + k] = null
      stampOwner(trackId, m, c + k, null)
      written.push({ mi: m, ci: c + k })
    }
    c += meas.subdivision
  }
  return { written, next: normalizePos(m, c) }
}

/** sílabas de kiai em células consecutivas (atravessando compassos) a partir de (mi, ci) */
export function fillKyai(trackId: string, mi: number, ci: number, syls: string[]): FillResult {
  const written: CellPos[] = []
  let m = mi
  let c = ci
  for (const syl of syls) {
    const pos = normalizePos(m, c)
    if (!pos) break
    ;({ mi: m, ci: c } = pos)
    setKyai(trackId, m, c, syl)
    written.push({ mi: m, ci: c })
    c++
  }
  return { written, next: normalizePos(m, c) }
}

/** Muda a subdivisão de todos os compassos, reamostrando o conteúdo. */
export function setSubdivision(sub: number) {
  for (const meas of store.song.measures) {
    const oldN = cellsPerMeasure(meas)
    const newN = meas.beats * sub
    if (newN === oldN) {
      meas.subdivision = sub
      continue
    }
    const resample = (rows: Measure['cells']) => {
      const res: Measure['cells'] = {}
      for (const [trackId, cells] of Object.entries(rows)) {
        const arr: (string | null)[] = Array(newN).fill(null)
        cells?.forEach((syl, i) => {
          if (!syl) return
          let idx = Math.min(newN - 1, Math.round((i * newN) / oldN))
          while (idx < newN && arr[idx] !== null) idx++
          if (idx < newN) arr[idx] = syl
        })
        res[trackId] = arr
      }
      return res
    }
    if (meas.kyai) meas.kyai = resample(meas.kyai)
    const resampled: Measure['cells'] = {}
    for (const [trackId, cells] of Object.entries(meas.cells)) {
      const arr: (string | null)[] = Array(newN).fill(null)
      cells?.forEach((syl, i) => {
        if (!syl) return
        let idx = Math.min(newN - 1, Math.round((i * newN) / oldN))
        while (idx < newN && arr[idx] !== null) idx++
        if (idx < newN) arr[idx] = syl
      })
      resampled[trackId] = arr
    }
    meas.subdivision = sub
    meas.cells = resampled
    // índices mudaram: descarta donos (fallback = personIds[0] até reatribuir)
    delete meas.cellOwners
  }
}

/** adiciona compasso: dentro da seção dada/selecionada (no fim dela) ou no fim da música */
export function addMeasure(sectionId?: string) {
  const target = sectionId ? (sectionRanges().find((r) => r.id === sectionId) ?? null) : selectedTargetSection()
  if (target) {
    const ref = store.song.measures[target.end] ?? store.song.measures.at(-1)
    const at = target.end + 1
    store.song.measures.splice(at, 0, makeMeasure(ref?.beats ?? 4, ref?.subdivision ?? 2))
    for (const s of store.song.sections ?? []) if (s.start >= at) s.start++
  } else {
    const ref = store.song.measures.at(-1)
    store.song.measures.push(makeMeasure(ref?.beats ?? 4, ref?.subdivision ?? 2))
  }
}

/** remove o último compasso da seção dada/selecionada (mantém ao menos 1) ou o último da música */
export function removeMeasure(sectionId?: string) {
  const target = sectionId ? (sectionRanges().find((r) => r.id === sectionId) ?? null) : selectedTargetSection()
  if (target) {
    if (target.end <= target.start) return // seção não pode ficar sem compasso
    const at = target.end
    store.song.measures.splice(at, 1)
    for (const s of store.song.sections ?? []) if (s.start > at) s.start--
  } else if (store.song.measures.length > 1) {
    store.song.measures.pop()
  }
}

let trackCounter = 0

/** adiciona linha a partir de uma InstrumentDef (builtin id ou custom id) */
export function addTrack(defId: string) {
  const def = instrumentById(defId) ?? BUILTIN_INSTRUMENTS[0]
  trackCounter++
  const id = `${def.base}-${Date.now().toString(36)}-${trackCounter}`
  const count = store.song.tracks.filter((t) => (t.instrumentId ?? t.instrument) === def.id).length
  const track: Track = {
    id,
    name: count > 0 ? `${def.name} ${count + 1}` : def.name,
    instrument: def.base,
    instrumentId: def.id,
    muted: false,
    solo: false,
  }
  store.song.tracks.push(track)
  for (const meas of store.song.measures) {
    meas.cells[id] = Array(cellsPerMeasure(meas)).fill(null)
  }
}

export function removeTrack(trackId: string) {
  store.song.tracks = store.song.tracks.filter((t) => t.id !== trackId)
  setMainTracks(mainTracks(store.song))
  for (const meas of store.song.measures) {
    delete meas.cells[trackId]
    if (meas.cellOwners) delete meas.cellOwners[trackId]
    if (meas.kyai) delete meas.kyai[trackId]
    if (meas.trackDynamics) {
      delete meas.trackDynamics[trackId]
      if (!Object.keys(meas.trackDynamics).length) delete meas.trackDynamics
    }
  }
}

/** grava a lista de destaque (vazia = remove o campo); migra o formato antigo */
function setMainTracks(ids: string[]) {
  delete store.song.mainTrackId
  if (ids.length) store.song.mainTrackIds = ids
  else delete store.song.mainTrackIds
}

/** liga/desliga o "main" de um instrumento (vários podem estar em destaque) */
export function toggleMainTrack(trackId: string) {
  const cur = mainTracks(store.song)
  setMainTracks(cur.includes(trackId) ? cur.filter((id) => id !== trackId) : [...cur, trackId])
}

export function isMainTrack(trackId: string): boolean {
  return mainTracks(store.song).includes(trackId)
}

export function renameTrack(trackId: string, name: string) {
  const t = store.song.tracks.find((x) => x.id === trackId)
  if (t) t.name = name
}

/** troca o instrumento de uma linha (mantém as notas); atualiza base + instrumentId */
export function setTrackInstrument(trackId: string, defId: string) {
  const t = store.song.tracks.find((x) => x.id === trackId)
  const def = instrumentById(defId)
  if (!t || !def) return
  t.instrument = def.base
  t.instrumentId = def.id
}

/** move a linha na ordem (reflete em grade/partitura/esteira). dir = -1 (cima) / +1 (baixo) */
export function moveTrack(trackId: string, dir: -1 | 1) {
  const arr = store.song.tracks
  const i = arr.findIndex((t) => t.id === trackId)
  const j = i + dir
  if (i < 0 || j < 0 || j >= arr.length) return
  const [x] = arr.splice(i, 1)
  arr.splice(j, 0, x)
}

// ---------- Pessoas (agrupamento de instrumentos) ----------

let personCounter = 0
function newPersonId() {
  personCounter++
  return `person-${Date.now().toString(36)}-${personCounter}`
}

export function personById(id: string | null | undefined): Person | null {
  if (!id) return null
  return store.song.people?.find((p) => p.id === id) ?? null
}

export function personColor(id: string | null | undefined): string | null {
  return personById(id)?.color ?? null
}

/** true se o instrumento é compartilhado (2+ pessoas) */
export function isShared(track: Track): boolean {
  return (track.personIds?.length ?? 0) >= 2
}

/**
 * Dono efetivo de uma célula (versão que recebe a track, sem lookup):
 * - track sem pessoa → null
 * - track com 1 pessoa → sempre ela (ignora cellOwners)
 * - compartilhada → cellOwners[trackId][ci]; fallback personIds[0] se ausente/stale
 */
export function cellOwnerOf(track: Track | undefined, mi: number, ci: number): string | null {
  const ids = track?.personIds ?? []
  if (!track || ids.length === 0) return null
  if (ids.length === 1) return ids[0]
  const raw = store.song.measures[mi]?.cellOwners?.[track.id]?.[ci] ?? null
  return raw && ids.includes(raw) ? raw : ids[0]
}

export function cellOwner(trackId: string, mi: number, ci: number): string | null {
  return cellOwnerOf(
    store.song.tracks.find((t) => t.id === trackId),
    mi,
    ci,
  )
}

/**
 * Pessoa audível segundo mudo/solo do submix: false = silenciada (notas dela
 * ficam transparentes nas views e não soam). null = sem dono → sempre audível.
 */
export function personAudible(id: string | null | undefined): boolean {
  if (!id) return true
  const people = store.song.people ?? []
  const p = people.find((x) => x.id === id)
  if (!p) return true
  if (p.muted) return false
  const anySolo = people.some((x) => x.solo)
  return !anySolo || !!p.solo
}

/** grava o dono de uma célula (só faz sentido em track compartilhada) */
export function assignCellOwner(trackId: string, mi: number, ci: number, personId: string | null) {
  const meas = store.song.measures[mi]
  if (!meas) return
  if (!meas.cellOwners) meas.cellOwners = {}
  if (!meas.cellOwners[trackId]) meas.cellOwners[trackId] = Array(cellsPerMeasure(meas)).fill(null)
  meas.cellOwners[trackId][ci] = personId
}

/** reatribui o dono de várias células (modo select) */
export function setCellsOwner(cells: { trackId: string; mi: number; ci: number }[], personId: string | null) {
  for (const c of cells) assignCellOwner(c.trackId, c.mi, c.ci, personId)
}

export function addPerson(name?: string): Person {
  if (!store.song.people) store.song.people = []
  const n = store.song.people.length
  const person: Person = {
    id: newPersonId(),
    name: name ?? `Pessoa ${n + 1}`,
    color: PERSON_COLORS[n % PERSON_COLORS.length],
  }
  store.song.people.push(person)
  if (!store.activePersonId) store.activePersonId = person.id
  return person
}

export function renamePerson(id: string, name: string) {
  const p = personById(id)
  if (p) p.name = name
}

export function setPersonColor(id: string, color: string) {
  const p = personById(id)
  if (p) p.color = color
}

/** remove a pessoa: limpa personIds e cellOwners; NÃO apaga tracks nem notas */
export function deletePerson(id: string) {
  if (!store.song.people) return
  store.song.people = store.song.people.filter((p) => p.id !== id)
  for (const t of store.song.tracks) {
    if (t.personIds) t.personIds = t.personIds.filter((pid) => pid !== id)
  }
  for (const meas of store.song.measures) {
    if (!meas.cellOwners) continue
    for (const arr of Object.values(meas.cellOwners)) {
      for (let i = 0; i < arr.length; i++) if (arr[i] === id) arr[i] = null
    }
  }
  if (store.activePersonId === id) store.activePersonId = store.song.people[0]?.id ?? null
}

/** liga/desliga a pertença de um instrumento a uma pessoa */
export function toggleTrackPerson(trackId: string, personId: string) {
  const track = store.song.tracks.find((t) => t.id === trackId)
  if (!track) return
  const ids = track.personIds ? [...track.personIds] : []
  const at = ids.indexOf(personId)
  if (at >= 0) ids.splice(at, 1)
  else ids.push(personId)
  track.personIds = ids
  // deixou de ser compartilhada → cellOwners perde o sentido; limpa
  if (ids.length <= 1) {
    for (const meas of store.song.measures) {
      if (meas.cellOwners) delete meas.cellOwners[trackId]
    }
  }
}

/**
 * Trilhos de agrupamento: para cada pessoa, quais linhas (na ordem de song.tracks)
 * ela toca. Suporta sobreposição e não-contiguidade. Consumido pelas views.
 */
export function railsForTracks(): { person: Person; rows: boolean[] }[] {
  const people = store.song.people ?? []
  const tracks = store.song.tracks
  return people
    .map((person) => ({
      person,
      rows: tracks.map((t) => (t.personIds ?? []).includes(person.id)),
    }))
    .filter((r) => r.rows.some(Boolean))
}

// ---------- Instrumentos (Kit) ----------

let instrumentCounter = 0
function newInstrumentId() {
  instrumentCounter++
  return `inst-${Date.now().toString(36)}-${instrumentCounter}`
}

/** paleta = builtins (constantes) + custom da música */
export function paletteInstruments(): InstrumentDef[] {
  return [...BUILTIN_INSTRUMENTS, ...(store.song.instruments ?? [])]
}

export function instrumentById(id: string | null | undefined): InstrumentDef | null {
  if (!id) return null
  return paletteInstruments().find((d) => d.id === id) ?? null
}

/** def efetiva de uma linha: por instrumentId; fallback = builtin do base (nunca null) */
export function resolveInstrument(track: Track): InstrumentDef {
  return (
    instrumentById(track.instrumentId) ??
    BUILTIN_INSTRUMENTS.find((d) => d.base === track.instrument) ??
    BUILTIN_INSTRUMENTS[0]
  )
}

/** nome a mostrar nos rótulos da linha */
export function instrumentLabel(track: Track): string {
  return resolveInstrument(track).name
}

export function addInstrument(base: InstrumentId, name?: string): InstrumentDef {
  if (!store.song.instruments) store.song.instruments = []
  const zone = () => ({ pitch: 0, gain: 0, overtones: 0, decay: 0.5 })
  const def: InstrumentDef = {
    id: newInstrumentId(),
    name: name ?? `${INSTRUMENTS[base]} custom`,
    base,
    head: zone(),
    rim: zone(),
  }
  store.song.instruments.push(def)
  return def
}

export function updateInstrument(id: string, patch: Partial<InstrumentDef>) {
  const def = store.song.instruments?.find((d) => d.id === id)
  if (def) Object.assign(def, patch)
}

/** merge de params de uma zona (couro/borda) de um instrument custom */
export function updateInstrumentZone(id: string, zone: 'head' | 'rim', patch: Partial<ZoneParams>) {
  const def = store.song.instruments?.find((d) => d.id === id)
  if (!def) return
  def[zone] = { ...(def[zone] ?? {}), ...patch }
}

export function renameInstrument(id: string, name: string) {
  updateInstrument(id, { name })
}

export function duplicateInstrument(id: string): InstrumentDef | null {
  const src = instrumentById(id)
  if (!src) return null
  if (!store.song.instruments) store.song.instruments = []
  const copy: InstrumentDef = { ...JSON.parse(JSON.stringify(src)), id: newInstrumentId(), name: `${src.name} cópia`, builtin: false }
  store.song.instruments.push(copy)
  return copy
}

/** remove a def custom; linhas que a usavam voltam ao som base (não morrem) */
export function deleteInstrument(id: string) {
  if (!store.song.instruments) return
  store.song.instruments = store.song.instruments.filter((d) => d.id !== id)
  for (const t of store.song.tracks) if (t.instrumentId === id) t.instrumentId = undefined
}

export function exportSong() {
  const blob = new Blob([JSON.stringify(store.song, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${store.song.title.toLowerCase().replace(/\s+/g, '-') || 'musica'}.taiko.json`
  a.click()
  URL.revokeObjectURL(url)
}

export async function importSong(file: File): Promise<string | null> {
  try {
    const parsed = JSON.parse(await file.text())
    if (parsed?.version !== 1 || !Array.isArray(parsed.tracks) || !Array.isArray(parsed.measures)) {
      return 'Arquivo não parece ser uma música do taiko-fu (version 1).'
    }
    store.song = parsed as Song
    return null
  } catch {
    return 'JSON inválido.'
  }
}

export function resetToDemo() {
  store.song = demoSong()
}
