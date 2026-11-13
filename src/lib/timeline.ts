import type { Song, Dynamic, Measure } from './types'
import { DYNAMIC_LEVELS } from './types'

/**
 * Ordem de execução: índice do compasso-fonte para cada compasso tocado.
 * Compassos antes da 1ª seção tocam 1×; cada seção toca `repeat` vezes
 * (ritornello). Com região, só os compassos dentro dela — mas as seções
 * selecionadas continuam repetindo.
 */
export function playOrder(song: Song, region: { startBar: number; endBar: number } | null = null): number[] {
  const n = song.measures.length
  const lo = region?.startBar ?? 0
  const hi = Math.min(region?.endBar ?? n, n)
  const inRange = (mi: number) => mi >= lo && mi < hi
  const secs = [...(song.sections ?? [])].sort((a, b) => a.start - b.start)
  const out: number[] = []
  for (let mi = 0; mi < (secs[0]?.start ?? n); mi++) if (inRange(mi)) out.push(mi)
  secs.forEach((s, i) => {
    const end = Math.min(secs[i + 1]?.start ?? n, n)
    const times = Math.max(1, s.repeat ?? 1)
    for (let r = 0; r < times; r++) for (let mi = s.start; mi < end; mi++) if (inRange(mi)) out.push(mi)
  })
  return out
}

/** instrumentos em destaque que ainda existem (aceita o formato antigo mainTrackId) */
export function mainTracks(song: Song): string[] {
  const ids = song.mainTrackIds ?? (song.mainTrackId ? [song.mainTrackId] : [])
  return ids.filter((id) => song.tracks.some((t) => t.id === id))
}

/**
 * Destaque "main": os instrumentos principais sobem como de mf→f (×f/mf) e os
 * demais descem como de mf→p (×p/mf). Relativo: crescendos, sfz e marcações
 * continuam. Sem destaque, ou com TODOS em destaque, tudo ×1.
 */
export function mainMul(song: Song, trackId: string): number {
  const main = mainTracks(song)
  if (!main.length || main.length >= song.tracks.length) return 1
  return main.includes(trackId) ? DYNAMIC_LEVELS.f / DYNAMIC_LEVELS.mf : DYNAMIC_LEVELS.p / DYNAMIC_LEVELS.mf
}

/**
 * Multiplicador de dinâmica por compasso tocado, na ordem de execução.
 * Uma marcação (pp..ff) vale até a próxima; crescendo/diminuendo num compasso
 * rampa do nível atual até a próxima marcação (ou um degrau acima/abaixo, se
 * não houver). Retorna f(u, frac) com frac ∈ [0,1) dentro do compasso u.
 * Com trackId: marcações do instrumento têm prioridade sobre a geral no
 * compasso em que aparecem; onde ele não marca nada, segue a geral.
 */
export function dynamicsCurve(song: Song, order: number[], trackId?: string): (u: number, frac: number) => number {
  const keys = Object.keys(DYNAMIC_LEVELS) as Dynamic[]
  const own = (m: Measure | undefined) => (trackId ? m?.trackDynamics?.[trackId] : undefined)
  const dynOf = (mi: number) => {
    const m = song.measures[mi]
    return own(m)?.dynamic ?? m?.dynamic
  }
  const hpOf = (mi: number) => {
    const m = song.measures[mi]
    return own(m)?.hairpin ?? m?.hairpin
  }
  // nível no início de cada compasso tocado (marcação herdada)
  const startLvl: number[] = []
  let cur = DYNAMIC_LEVELS.mf
  let curKey: Dynamic = 'mf'
  // tocando um trecho: herda a última marcação antes dele
  for (let mi = 0; mi < (order[0] ?? 0); mi++) {
    const d = dynOf(mi)
    if (d) {
      cur = DYNAMIC_LEVELS[d]
      curKey = d
    }
  }
  const curKeys: Dynamic[] = []
  for (const mi of order) {
    const d = dynOf(mi)
    if (d) {
      cur = DYNAMIC_LEVELS[d]
      curKey = d
    }
    startLvl.push(cur)
    curKeys.push(curKey)
  }
  // segmentos de hairpin: sequência de compassos com o mesmo hairpin até uma marcação
  const ramp: ({ from: number; to: number; i0: number; len: number } | null)[] = order.map(() => null)
  let u = 0
  while (u < order.length) {
    const hp = hpOf(order[u])
    if (!hp) {
      u++
      continue
    }
    let v = u
    while (v + 1 < order.length && hpOf(order[v + 1]) === hp && !dynOf(order[v + 1])) v++
    const nextMarked = v + 1 < order.length ? dynOf(order[v + 1]) : undefined
    const from = startLvl[u]
    const idx = keys.indexOf(curKeys[u])
    const step = keys[Math.max(0, Math.min(keys.length - 1, idx + (hp === 'cresc' ? 1 : -1)))]
    // alvo = próxima marcação, se ela vai no sentido do hairpin (ex.: a volta
    // do ritornello pra um p não "desfaz" o crescendo); senão, um degrau
    const marked = nextMarked ? DYNAMIC_LEVELS[nextMarked] : null
    const goesRight = marked !== null && (hp === 'cresc' ? marked > from : marked < from)
    const to = goesRight ? marked : DYNAMIC_LEVELS[step]
    for (let k = u; k <= v; k++) ramp[k] = { from, to, i0: u, len: v - u + 1 }
    // sem marcação depois: o nível-alvo persiste após a rampa
    if (!nextMarked) for (let k = v + 1; k < order.length && !dynOf(order[k]); k++) startLvl[k] = to
    u = v + 1
  }
  return (uu, frac) => {
    const r = ramp[uu]
    if (!r) return startLvl[uu] ?? DYNAMIC_LEVELS.mf
    const t = (uu - r.i0 + frac) / r.len
    return r.from + (r.to - r.from) * t
  }
}
