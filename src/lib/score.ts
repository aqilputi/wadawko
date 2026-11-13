/**
 * Renderização da música como partitura de taiko (visão read-only).
 * Convenção usada por grupos: pauta de 1 linha por tambor, cabeça normal = pele,
 * cabeça × = borda (fuchi), acento >, sílaba kuchi shōga embaixo da nota.
 */
import {
  Renderer,
  Stave,
  StaveConnector,
  StaveNote,
  Beam,
  Annotation,
  AnnotationVerticalJustify,
  Articulation,
  Formatter,
  Voice,
  Tuplet,
} from 'vexflow'
import type { Song, Track, Measure, Person } from './types'
import { cellsPerMeasure } from './types'
import { resolveCell } from './phrase'

/** dono efetivo de uma célula (espelha store.cellOwnerOf, mas sobre a Song passada) */
function ownerOf(track: Track, meas: Measure, ci: number): string | null {
  const ids = track.personIds ?? []
  if (!ids.length) return null
  if (ids.length === 1) return ids[0]
  const raw = meas.cellOwners?.[track.id]?.[ci] ?? null
  return raw && ids.includes(raw) ? raw : ids[0]
}

function personOf(song: Song, id: string | null): Person | null {
  return song.people?.find((p) => p.id === id) ?? null
}

/** pessoa audível segundo mudo/solo (espelha store.personAudible) */
function audibleOf(song: Song, id: string | null): boolean {
  if (!id) return true
  const people = song.people ?? []
  const p = people.find((x) => x.id === id)
  if (!p) return true
  if (p.muted) return false
  const anySolo = people.some((x) => x.solo)
  return !anySolo || !!p.solo
}

const isSharedTrack = (t: Track) => (t.personIds?.length ?? 0) >= 2

/** sílabas de 2 golpes quebradas por golpe na letra: doko → do + ko */
const SPLIT: Record<string, string[]> = {
  doko: ['do', 'ko'],
  kara: ['ka', 'ra'],
  tsuku: ['tsu', 'ku'],
  teke: ['te', 'ke'],
}

/** durações por subdivisão: [célula, 1/2 célula, 1/4 célula] */
const DUR: Record<number, [string, string, string]> = {
  1: ['4', '8', '16'],
  2: ['8', '16', '32'],
  4: ['16', '32', '64'],
  8: ['32', '64', '128'],
}

const STAVE_HEIGHT = 86

/** cor dos kiai na partitura (destaca do kuchi shōga, logo acima deles) */
const KYAI_COLOR = '#d6452b'

interface NoteSpec {
  rest: boolean
  rim: boolean
  accent: boolean
  /** sforzando: "sfz" escrito acima da nota */
  sfz?: boolean
  /** mão: R desenha acima da linha, L abaixo (convenção Wagaku Hitosuji) */
  hand: 'R' | 'L'
  syllable: string | null
  /** duração VexFlow: '8', '16', '8r'... */
  duration: string
  /** índice da célula de origem dentro do compasso (p/ highlight do playback) */
  cell: number
  /** índice do golpe dentro da célula (p/ highlight por nota) */
  hitIndex?: number
  /** nota única representando N>4 golpes (rulo): quantos golpes ela cobre */
  hitSpan?: number
  /** células com 3 golpes viram quiáltera; id agrupa as notas dela */
  tripletGroup?: number
  /** cor do dono (só em instrumento compartilhado) — tinge a nota */
  ownerColor?: string | null
  /** dono da nota está mudo (submix silenciado) → nota semitransparente */
  ownerMuted?: boolean
  /** kiai gritado nesta célula (escrito acima da pauta) */
  kyai?: string | null
}

/** chave "trackId:célulaGlobal:golpe" → elementos SVG da nota que soa */
export type HighlightMap = Map<string, SVGElement[]>

export function hitKey(trackId: string, globalCell: number, hitIndex: number): string {
  return `${trackId}:${globalCell}:${hitIndex}`
}

function svgElementOf(note: StaveNote): SVGElement | null {
  const anyNote = note as unknown as { getSVGElement?: () => SVGElement | undefined; attrs?: { el?: SVGElement } }
  return anyNote.getSVGElement?.() ?? anyNote.attrs?.el ?? null
}

/** rótulos por golpe de uma célula: parte de 2 golpes usa o SPLIT (doko → do, ko) */
function hitLabels(value: string): (string | null)[] {
  const resolved = resolveCell(value)
  if (!resolved) return [value]
  const labels: (string | null)[] = []
  for (const part of resolved.parts) {
    if (part.hits.length === 2) {
      const split = SPLIT[part.syl] ?? [part.syl, null]
      labels.push(split[0] ?? null, split[1] ?? null)
    } else {
      labels.push(part.syl)
      for (let i = 1; i < part.hits.length; i++) labels.push(null)
    }
  }
  return labels
}

/** Converte as células de um compasso/linha em especificações de nota. */
function measureToNotes(song: Song, track: Track, meas: Measure): NoteSpec[] {
  const nCells = cellsPerMeasure(meas)
  const cells = meas.cells[track.id] ?? []
  const [cellDur, halfDur, quarterDur] = DUR[meas.subdivision] ?? DUR[2]
  const shared = isSharedTrack(track)

  const specs: NoteSpec[] = []
  for (let ci = 0; ci < nCells; ci++) {
    const syl = cells[ci]
    const owner = shared ? ownerOf(track, meas, ci) : null
    const ownerColor = owner ? (personOf(song, owner)?.color ?? null) : null
    const ownerMuted = owner ? !audibleOf(song, owner) : false
    const resolved = syl ? resolveCell(syl) : null
    const kyai = meas.kyai?.[track.id]?.[ci] ?? null
    if (!syl || !resolved || resolved.hits.length === 0) {
      specs.push({
        kyai,
        rest: true,
        rim: false,
        accent: false,
        hand: 'R',
        syllable: syl === 'su' ? 'su' : null,
        duration: `${cellDur}r`,
        cell: ci,
      })
      continue
    }

    const n = resolved.hits.length
    const labels = hitLabels(syl)
    // n golpes dentro de UMA célula: 1 = célula inteira; 2 = metades; 3 =
    // quiáltera de meias-células; 4 = quartos; mais que 4 = nota única rotulada
    // com a palavra inteira (aproximação de rulo)
    const duration = n === 1 ? cellDur : n === 2 || n === 3 ? halfDur : n === 4 ? quarterDur : cellDur
    const shown = n > 4 ? 1 : n
    for (let i = 0; i < shown; i++) {
      const hit = resolved.hits[i]
      specs.push({
        rest: false,
        rim: hit.zone === 'rim',
        accent: hit.accent,
        sfz: hit.sfz,
        hand: hit.hand,
        syllable: n > 4 ? syl : (labels[i] ?? null),
        duration,
        cell: ci,
        hitIndex: i,
        kyai: i === 0 ? kyai : null,
        hitSpan: n > 4 ? n : undefined,
        tripletGroup: n === 3 ? ci : undefined,
        ownerColor,
        ownerMuted,
      })
    }
  }
  return specs
}

function toStaveNote(spec: NoteSpec): StaveNote {
  // pauta de 1 linha do VexFlow preserva a linha do TOPO do pentagrama (f/5):
  // mão direita = espaço acima (g/5), esquerda = espaço abaixo (e/5), pausa na linha
  const pos = spec.rest ? 'f/5' : spec.hand === 'L' ? 'e/5' : 'g/5'
  const key = spec.rim ? `${pos}/x2` : pos
  const note = new StaveNote({ keys: [key], duration: spec.duration, stemDirection: 1 })
  // instrumento compartilhado: tinge a nota com a cor do dono
  if (spec.ownerColor) {
    note.setStyle({ fillStyle: spec.ownerColor, strokeStyle: spec.ownerColor })
  }
  if (spec.accent) {
    note.addModifier(new Articulation('a>').setPosition(3))
  }
  if (spec.sfz) {
    note.addModifier(
      new Annotation('sfz').setFont('Georgia', 11, 'bold', 'italic').setVerticalJustification(AnnotationVerticalJustify.TOP),
    )
  }
  if (spec.syllable) {
    const ann = new Annotation(spec.syllable)
      .setFont('Zen Kaku Gothic New', 11)
      .setVerticalJustification(AnnotationVerticalJustify.BOTTOM)
    note.addModifier(ann)
  }
  // kiai embaixo do kuchi shōga da própria pauta (em cima colaria na pauta anterior)
  if (spec.kyai) {
    const shout = spec.kyai.endsWith('!')
    const ann = new Annotation(spec.kyai)
      .setFont('Zen Kaku Gothic New', shout ? 12 : 11, shout ? 'bold' : 'normal', 'italic')
      .setVerticalJustification(AnnotationVerticalJustify.BOTTOM)
    ann.setStyle({ fillStyle: KYAI_COLOR, strokeStyle: KYAI_COLOR })
    note.addModifier(ann)
  }
  return note
}

const TOP = 10
const ROW_H = 110

/**
 * Limpa o container e desenha a partitura como UM sistema: todas as pautas no
 * mesmo SVG, um Formatter compartilhado por compasso — notas alinham
 * verticalmente por tempo entre os instrumentos, e a barra de compasso conecta
 * as pautas (StaveConnector).
 */
export function renderScore(root: HTMLElement, song: Song): HighlightMap {
  const highlights: HighlightMap = new Map()
  root.replaceChildren()
  if (song.tracks.length === 0) return highlights

  const row = document.createElement('div')
  row.className = 'score-row'

  // gutter de trilhos de pessoa (uma coluna por pessoa, acesa nas linhas dela)
  const people = (song.people ?? []).filter((p) =>
    song.tracks.some((t) => (t.personIds ?? []).includes(p.id)),
  )
  let railsEl: HTMLDivElement | null = null
  if (people.length) {
    railsEl = document.createElement('div')
    railsEl.className = 'score-rails'
    for (const person of people) {
      const col = document.createElement('div')
      col.className = 'score-rail'
      col.style.setProperty('--pc', person.color ?? '#888')
      col.title = person.name
      const name = document.createElement('span')
      name.className = 'rail-name'
      name.textContent = person.name
      col.appendChild(name)
      for (const track of song.tracks) {
        const seg = document.createElement('div')
        seg.className = 'rail-seg'
        if ((track.personIds ?? []).includes(person.id)) seg.classList.add('on')
        col.appendChild(seg)
      }
      railsEl.appendChild(col)
    }
  }

  const labels = document.createElement('div')
  labels.className = 'score-labels'
  for (const track of song.tracks) {
    const label = document.createElement('div')
    label.className = 'score-label'
    label.textContent = track.name
    labels.appendChild(label)
  }
  const staves = document.createElement('div')
  staves.className = 'score-staves'
  if (railsEl) row.append(railsEl, labels, staves)
  else row.append(labels, staves)
  root.appendChild(row)

  const widths = song.measures.map((m) => Math.max(m.beats * 110, cellsPerMeasure(m) * 36))
  const totalWidth = widths.reduce((a, b) => a + b, 0) + 20

  const renderer = new Renderer(staves as HTMLDivElement, Renderer.Backends.SVG)
  renderer.resize(totalWidth, TOP + song.tracks.length * ROW_H + 10)
  const ctx = renderer.getContext()

  let x = 10
  let cellBase = 0
  song.measures.forEach((meas, mi) => {
    const width = widths[mi]
    const isLast = mi === song.measures.length - 1
    const voices: Voice[] = []
    const drawables: {
      voice: Voice
      stave: Stave
      beams: Beam[]
      tuplets: Tuplet[]
      specs: NoteSpec[]
      notes: StaveNote[]
      trackId: string
    }[] = []
    const measureStaves: Stave[] = []

    song.tracks.forEach((track, ti) => {
      const stave = new Stave(x, TOP + ti * ROW_H, width, { numLines: 1 })
      if (isLast) stave.setEndBarType(3) // barra final
      stave.setContext(ctx).draw()
      measureStaves.push(stave)

      const specs = measureToNotes(song, track, meas)
      const notes = specs.map(toStaveNote)

      // quiálteras (3 golpes numa célula)
      const tuplets: Tuplet[] = []
      const tripletGroups = new Map<number, StaveNote[]>()
      specs.forEach((spec, i) => {
        if (spec.tripletGroup === undefined) return
        const group = tripletGroups.get(spec.tripletGroup) ?? []
        group.push(notes[i])
        tripletGroups.set(spec.tripletGroup, group)
      })
      for (const group of tripletGroups.values()) {
        if (group.length === 3) tuplets.push(new Tuplet(group, { numNotes: 3, notesOccupied: 2 }))
      }

      const voice = new Voice({ numBeats: meas.beats, beatValue: 4 }).setStrict(false)
      voice.addTickables(notes)
      const beams = Beam.generateBeams(notes, { maintainStemDirections: false })
      voices.push(voice)
      drawables.push({ voice, stave, beams, tuplets, specs, notes, trackId: track.id })
    })

    // formata TODAS as vozes do compasso juntas: alinhamento vertical por tempo
    const formatter = new Formatter()
    for (const voice of voices) formatter.joinVoices([voice])
    formatter.format(voices, width - 30)

    for (const d of drawables) {
      d.voice.draw(ctx, d.stave)
      d.beams.forEach((b) => b.setContext(ctx).draw())
      d.tuplets.forEach((t) => t.setContext(ctx).draw())
      d.notes.forEach((note, i) => {
        const spec = d.specs[i]
        if (spec.rest || spec.hitIndex === undefined) return
        const el = svgElementOf(note)
        if (!el) return
        // dono mudo (submix silenciado): nota semitransparente
        if (spec.ownerMuted) el.style.opacity = '0.3'
        const globalIdx = cellBase + spec.cell
        // rulo (N>4) tem 1 nota cobrindo todos os golpes: registra em todas as chaves
        const span = spec.hitSpan ?? 1
        for (let h = 0; h < span; h++) {
          const key = hitKey(d.trackId, globalIdx, spec.hitIndex + h)
          const bucket = highlights.get(key)
          if (bucket) bucket.push(el)
          else highlights.set(key, [el])
        }
      })
    }

    // barra de compasso conectando os instrumentos (visual de ensemble)
    if (song.tracks.length > 1) {
      const first = measureStaves[0]
      const last = measureStaves[measureStaves.length - 1]
      new StaveConnector(first, last).setType(StaveConnector.type.SINGLE_LEFT).setContext(ctx).draw()
      if (isLast) {
        new StaveConnector(first, last).setType(StaveConnector.type.BOLD_DOUBLE_RIGHT).setContext(ctx).draw()
      }
    }

    cellBase += cellsPerMeasure(meas)
    x += width
  })
  return highlights
}
