/**
 * Eventos de nota para a esteira (visão estilo Taiko no Tatsujin): cada golpe
 * vira um disco com o instante ABSOLUTO em segundos. A matemática de tempo
 * (offset do golpe, swing, posição em quartos) espelha Engine.schedule — manter
 * as duas em sincronia. A chave `trackId:célulaGlobal:golpe` casa com o
 * hitListener do engine, então o flash do tambor cai no frame exato do áudio.
 */
import type { Song, Track, Measure } from './types'
import { cellsPerMeasure } from './types'
import { resolveCell } from './phrase'
import { swingWarp } from './audio'

/** dono efetivo de uma célula (espelha store.cellOwnerOf, sobre a Song passada) */
function ownerOf(track: Track, meas: Measure, ci: number): string | null {
  const ids = track.personIds ?? []
  if (!ids.length) return null
  if (ids.length === 1) return ids[0]
  const raw = meas.cellOwners?.[track.id]?.[ci] ?? null
  return raw && ids.includes(raw) ? raw : ids[0]
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

export interface NoteEvent {
  trackId: string
  /** hitKey — idêntica à do engine (trackId:célulaGlobal:golpe) */
  key: string
  /** instante no transport, em segundos (no BPM atual da música) */
  sec: number
  /** couro (head) = vermelho · fuchi (rim) = azul */
  zone: 'head' | 'rim'
  accent: boolean
  /** golpe fantasma (tsu/ku): disco menor e apagado */
  ghost: boolean
  /** duas mãos no mesmo instante (doron): disco grande com anel duplo */
  bothHands: boolean
  hand: 'R' | 'L'
  /** sílaba desenhada no disco (só no 1º golpe da célula) */
  label: string
  /** dono da nota (instrumento compartilhado); null = sem dono/atribuição */
  ownerId: string | null
  /** cor do dono (só em instrumento compartilhado) → anel no disco */
  ownerColor: string | null
  /** dono mudo (submix silenciado) → disco transparente */
  ownerMuted: boolean
}

export interface LaneData {
  events: NoteEvent[]
  /** duração total da música em segundos (p/ loop e fim) */
  lengthSec: number
}

export function noteEvents(song: Song): LaneData {
  const swing = song.swing ?? 0
  const beatSec = 60 / song.tempo // segundos por quarto (bar = 4 quartos, como no Transport)
  const events: NoteEvent[] = []
  let globalCell = 0

  song.measures.forEach((meas, mi) => {
    const nCells = cellsPerMeasure(meas)
    for (let ci = 0; ci < nCells; ci++) {
      const beat = Math.floor(ci / meas.subdivision)
      const cellFrac = (ci % meas.subdivision) / meas.subdivision
      const globalIdx = globalCell
      for (const track of song.tracks) {
        const syl = meas.cells[track.id]?.[ci]
        if (!syl) continue
        const resolved = resolveCell(syl)
        if (!resolved) continue
        const shared = (track.personIds?.length ?? 0) >= 2
        const ownerId = shared ? ownerOf(track, meas, ci) : null
        const ownerColor = ownerId ? (song.people?.find((p) => p.id === ownerId)?.color ?? null) : null
        const ownerMuted = ownerId ? !audibleOf(song, ownerId) : false
        // agrupa golpes simultâneos (mesmo offset): R+L juntos = uma nota de duas mãos
        const groups = new Map<number, { hit: (typeof resolved.hits)[number]; hi: number }[]>()
        resolved.hits.forEach((hit, hi) => {
          const g = groups.get(hit.offset) ?? []
          g.push({ hit, hi })
          groups.set(hit.offset, g)
        })
        for (const group of groups.values()) {
          const first = group[0].hit
          const bothHands = group.length >= 2 && new Set(group.map((g) => g.hit.hand)).size >= 2
          const warped = swingWarp(cellFrac + first.offset / meas.subdivision, swing)
          const quarters = mi * 4 + beat + warped
          events.push({
            trackId: track.id,
            key: `${track.id}:${globalIdx}:${group[0].hi}`,
            sec: quarters * beatSec,
            zone: first.zone,
            accent: first.accent || bothHands,
            ghost: first.vel <= 0.25 && !bothHands,
            bothHands,
            hand: first.hand,
            label: group.some((g) => g.hi === 0) ? syl : '',
            ownerId,
            ownerColor,
            ownerMuted,
          })
        }
      }
      globalCell++
    }
  })

  return { events, lengthSec: song.measures.length * 4 * beatSec }
}

/** posição do playback como célula global fracionária (p/ auto-scroll suave) */
export function cellFloatAt(song: Song, sec: number): number {
  const spb = 60 / song.tempo
  const quarters = sec / spb // bar = 4 quartos, como no Transport
  let acc = 0
  for (let mi = 0; mi < song.measures.length; mi++) {
    const m = song.measures[mi]
    const barStart = mi * 4
    if (quarters < barStart + 4 || mi === song.measures.length - 1) {
      const within = Math.max(0, quarters - barStart)
      return acc + Math.min(cellsPerMeasure(m), within * m.subdivision)
    }
    acc += cellsPerMeasure(m)
  }
  return acc
}
