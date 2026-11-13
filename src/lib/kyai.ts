/**
 * Kiai / kakegoe (掛け声): gritos escritos numa sub-linha de qualquer
 * instrumento, UMA sílaba por célula ("so" | "re!"). A voz é animalese
 * (letras do animalese.js recortadas e transpostas) + ruído filtrado nas
 * consoantes sibilantes, que no animalese são mudas (S, T, F…).
 *
 * Cada pessoa (ou linha, se não tiver pessoa) tem uma voz própria derivada do
 * id: tom, velocidade e "timbre" (ponto do recorte da letra). Cada grito ainda
 * varia aleatoriamente (tom, contorno, tempo, duração) — `vary` regula quanto.
 * Entonação japonesa: sílaba inicial baixa subindo, meio alto, e o "!" final é
 * um ataque alto que despenca (o grito); sem "!" a palavra termina neutra.
 */
import * as Tone from 'tone'
import type { Song, Track, Measure, KyaiVoice } from './types'
import { cellsPerMeasure } from './types'

/** kakegoe prontos (palavras separadas por espaço = gritos separados) */
export const KYAI_PRESETS = [
  'sa!',
  'sei!',
  'seiya!',
  'ha!',
  'hai!',
  'sore!',
  'soriya!',
  'hyia!',
  'soryia!',
  'soresoresoresore!',
  'soresoresoresore! soryia!',
]

const VOWELS = 'aeiou'
const isV = (c: string | undefined) => !!c && c.length === 1 && VOWELS.includes(c)
/** onset: consoante (dígrafos primeiro) + glide opcional ("hy", "ryi" antes de vogal) */
const ONSET = /^(?:ch|sh|ts|[bcdfghjklmnpqrstvwxz])?(?:yi(?=[aeou])|y)?/

/**
 * Texto → sílabas, uma por célula. "seiya!" → ["sei", "ya!"]; o "!" vai na
 * última sílaba de cada palavra. Ditongos (ai, ei, ou), vogal longa (aa, "a-")
 * e n final ficam na mesma sílaba; consoante dobrada (sokuon) fecha a anterior.
 * null = texto inválido (letra sem vogal, símbolo estranho).
 */
export function splitKyai(text: string): string[] | null {
  const out: string[] = []
  const words = text.trim().toLowerCase().split(/[\s,;|]+/).filter(Boolean)
  for (const raw of words) {
    const shout = /!+$/.test(raw)
    const w = raw.replace(/!+$/, '').replace(/ー/g, '-')
    if (!/^[a-z-]+$/.test(w) || !/[aeiou]/.test(w)) return null
    const syls: string[] = []
    let i = 0
    while (i < w.length) {
      const onset = w.slice(i).match(ONSET)?.[0] ?? ''
      const v = w[i + onset.length]
      if (!isV(v)) {
        // sem vogal: n moraico ou consoante dobrada → coda da sílaba anterior
        const c = w[i]
        if (syls.length && (c === 'n' || c === '-' || c === w[i + 1])) {
          syls[syls.length - 1] += c
          i++
          continue
        }
        return null
      }
      let s = onset + v
      i += s.length
      // coda: vogal longa / "-", ditongo, n que não abre sílaba
      while (i < w.length) {
        const c = w[i]
        const prev = s[s.length - 1]
        const nextIsVowel = isV(w[i + 1]) || w[i + 1] === 'y'
        if (c === '-' || (c === prev && isV(c))) s += c
        else if (c === 'i' && 'aeou'.includes(prev) && !isV(w[i + 1])) s += c
        else if (c === 'u' && prev === 'o' && !isV(w[i + 1])) s += c
        else if (c === 'n' && !nextIsVowel) s += c
        else break
        i++
      }
      syls.push(s)
    }
    if (shout) syls[syls.length - 1] += '!'
    out.push(...syls)
  }
  return out.length ? out : null
}

export const isShout = (syl: string) => syl.endsWith('!')

/* ───────────────────────── voz por pessoa ───────────────────────── */

export interface KyaiProfile {
  /** transposição base (semitons) */
  pitch: number
  /** multiplicador de duração das consoantes/sílabas */
  speed: number
  /** 0..1 — quanto cada grito varia */
  vary: number
  /** deslocamento (s) do recorte dentro da letra: muda o "timbre" */
  grit: number
  /** volume do ruído das sibilantes */
  breath: number
  /** extensão do contorno (gritos mais "cantados" ou mais secos) */
  range: number
}

function hash(s: string): number {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
  return h >>> 0
}

/** número 0..1 determinístico a partir de (seed, k) */
function rnd(seed: number, k: number): number {
  const x = Math.sin(seed * 0.0001 + k * 12.9898) * 43758.5453
  return x - Math.floor(x)
}

export const KYAI_DEFAULTS: Required<KyaiVoice> = { pitch: 0, vary: 0.5, gain: 0 }

/** voz de um jogador (seed = id da pessoa; sem pessoa, id da linha) */
export function kyaiProfile(seedId: string, track: Track): KyaiProfile {
  const h = hash(seedId)
  return {
    pitch: -4 + rnd(h, 1) * 9 + (track.kyai?.pitch ?? KYAI_DEFAULTS.pitch),
    speed: 0.85 + rnd(h, 2) * 0.35,
    vary: track.kyai?.vary ?? KYAI_DEFAULTS.vary,
    grit: rnd(h, 3) * 0.012,
    breath: 0.5 + rnd(h, 4) * 0.6,
    range: 0.75 + rnd(h, 5) * 0.5,
  }
}

/* ───────────────────────── entonação ───────────────────────── */

/**
 * Contorno [início, fim] em semitons de uma sílaba dentro da palavra.
 * 1 sílaba com "!" = ataque alto que cai (sa!, hai!); palavra longa sobe
 * da 1ª pra 2ª (heiban), alterna alto/médio no meio (so-re so-re…) e o "!"
 * final é o pico, caindo no fim (so-ri-YA!).
 */
export function contour(idx: number, n: number, shout: boolean): [number, number] {
  const lift = Math.min(2, idx * 0.15) // cantos longos vão esquentando
  if (n === 1) return shout ? [5, -4] : [3, 0.5]
  if (idx === n - 1) return shout ? [6 + lift, -5 + lift] : [2 + lift, 0.5 + lift]
  if (idx === 0) return [0, 1.5]
  return idx % 2 ? [3 + lift, 3.5 + lift] : [1.5 + lift, 2 + lift]
}

/* ───────────────────────── eventos da música ───────────────────────── */

export interface KyaiEvent {
  track: Track
  /** compasso tocado (unidade da ordem de execução), compasso-fonte e célula */
  u: number
  mi: number
  ci: number
  beat: number
  /** fração do tempo (já com swing) */
  frac: number
  /** posição absoluta em tempos desde o início da execução */
  beats: number
  syl: string
  idx: number
  n: number
  shout: boolean
  /** tempos até a próxima sílaba da mesma linha (Infinity = nada depois) */
  gapBeats: number
}

/** sílabas de kiai na ordem de execução, agrupadas em palavras por linha */
export function kyaiEvents(song: Song, order: number[], swingWarp: (f: number) => number): KyaiEvent[] {
  const byTrack = new Map<string, KyaiEvent[]>()
  order.forEach((mi, u) => {
    const meas = song.measures[mi]
    if (!meas?.kyai) return
    for (const track of song.tracks) {
      const row = meas.kyai[track.id]
      if (!row) continue
      for (let ci = 0; ci < cellsPerMeasure(meas); ci++) {
        const syl = row[ci]
        if (!syl) continue
        const beat = Math.floor(ci / meas.subdivision)
        const frac = swingWarp((ci % meas.subdivision) / meas.subdivision)
        const list = byTrack.get(track.id) ?? []
        list.push({ track, u, mi, ci, beat, frac, beats: u * 4 + beat + frac, syl, idx: 0, n: 1, shout: isShout(syl), gapBeats: Infinity })
        byTrack.set(track.id, list)
      }
    }
  })
  const all: KyaiEvent[] = []
  for (const list of byTrack.values()) {
    // palavra termina num "!" ou numa pausa de mais de 1 tempo
    let word: KyaiEvent[] = []
    const flush = () => {
      word.forEach((e, i) => {
        e.idx = i
        e.n = word.length
      })
      word = []
    }
    list.forEach((e, i) => {
      const next = list[i + 1]
      e.gapBeats = next ? next.beats - e.beats : Infinity
      word.push(e)
      if (e.shout || !next || e.gapBeats > 1.01) flush()
    })
    all.push(...list)
  }
  return all
}

/** dono da célula de kiai (espelha o dono dos golpes) → semente da voz */
export function kyaiSeed(track: Track, meas: Measure, ci: number): string {
  const ids = track.personIds ?? []
  if (!ids.length) return track.id
  if (ids.length === 1) return ids[0]
  const raw = meas.cellOwners?.[track.id]?.[ci] ?? null
  return raw && ids.includes(raw) ? raw : ids[0]
}

/* ───────────────────────── síntese ───────────────────────── */

const LETTER_SEC = 0.15
/** início audível (s) e força relativa de cada letra do animalese.wav */
const LETTER: Record<string, { start: number; gain: number }> = {
  a: { start: 0, gain: 1 }, b: { start: 0, gain: 0.8 }, d: { start: 0.02, gain: 1.6 },
  e: { start: 0, gain: 1.5 }, g: { start: 0.01, gain: 2 }, i: { start: 0.008, gain: 1.8 },
  j: { start: 0.02, gain: 2.5 }, k: { start: 0.03, gain: 1.6 }, m: { start: 0, gain: 3 },
  n: { start: 0, gain: 2.5 }, o: { start: 0, gain: 0.8 }, p: { start: 0.02, gain: 1.3 },
  u: { start: 0, gain: 3.5 }, w: { start: 0.02, gain: 1.4 },
}

/** consoantes: letra do animalese, ou ruído filtrado nas que lá são mudas */
type Cons =
  | { kind: 'letter'; l: string; dur: number; gain?: number }
  | { kind: 'noise'; type: BiquadFilterType; freq: number; q: number; dur: number; gain: number }
const CONS: Record<string, Cons[]> = {
  k: [{ kind: 'letter', l: 'k', dur: 0.035 }],
  g: [{ kind: 'letter', l: 'g', dur: 0.035 }],
  p: [{ kind: 'letter', l: 'p', dur: 0.03 }],
  b: [{ kind: 'letter', l: 'b', dur: 0.03 }],
  d: [{ kind: 'letter', l: 'd', dur: 0.03 }],
  r: [{ kind: 'letter', l: 'd', dur: 0.018, gain: 0.6 }], // r japonês = batida de língua
  m: [{ kind: 'letter', l: 'm', dur: 0.045 }],
  n: [{ kind: 'letter', l: 'n', dur: 0.04 }],
  w: [{ kind: 'letter', l: 'w', dur: 0.03 }],
  j: [{ kind: 'letter', l: 'j', dur: 0.035 }],
  z: [{ kind: 'letter', l: 'j', dur: 0.035 }],
  y: [{ kind: 'letter', l: 'i', dur: 0.03, gain: 0.7 }],
  s: [{ kind: 'noise', type: 'highpass', freq: 5200, q: 0.7, dur: 0.075, gain: 0.5 }],
  sh: [{ kind: 'noise', type: 'bandpass', freq: 3000, q: 1.2, dur: 0.07, gain: 0.7 }],
  ts: [{ kind: 'noise', type: 'highpass', freq: 4500, q: 0.7, dur: 0.05, gain: 0.55 }],
  ch: [{ kind: 'noise', type: 'bandpass', freq: 3600, q: 1, dur: 0.05, gain: 0.6 }],
  t: [{ kind: 'noise', type: 'highpass', freq: 3500, q: 0.7, dur: 0.014, gain: 0.7 }],
  h: [{ kind: 'noise', type: 'bandpass', freq: 1700, q: 0.8, dur: 0.05, gain: 0.45 }],
  f: [{ kind: 'noise', type: 'bandpass', freq: 1500, q: 0.8, dur: 0.05, gain: 0.4 }],
}

let noiseBuf: AudioBuffer | null = null
function noise(ctx: BaseAudioContext): AudioBuffer {
  if (!noiseBuf || noiseBuf.sampleRate !== ctx.sampleRate) {
    noiseBuf = ctx.createBuffer(1, Math.ceil(ctx.sampleRate * 0.4), ctx.sampleRate)
    const d = noiseBuf.getChannelData(0)
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1
  }
  return noiseBuf
}

/** quebra a sílaba em consoantes + vogais ("hyia" → h, y, a; "sein" → s, e, i, n) */
function phonemes(syl: string): { cons: Cons[]; vowels: string[]; coda: Cons[] } {
  const s = syl.replace(/!+$/, '')
  const onset = s.match(ONSET)?.[0] ?? ''
  const cons: Cons[] = []
  let o = onset
  for (const dig of ['ch', 'sh', 'ts']) {
    if (o.startsWith(dig)) {
      cons.push(...CONS[dig])
      o = o.slice(2)
    }
  }
  for (const c of o.replace(/yi$/, 'y')) cons.push(...(CONS[c] ?? []))
  const vowels: string[] = []
  const coda: Cons[] = []
  let last = ''
  for (const c of s.slice(onset.length)) {
    if (c === '-') vowels.push(last)
    else if (isV(c)) vowels.push((last = c))
    else if (c === 'n') coda.push(...CONS.n)
    // consoante dobrada (sokuon) = só fecha a sílaba
  }
  return { cons, vowels, coda }
}

export interface KyaiNote {
  syl: string
  /** semitons no início/fim da sílaba (contorno + voz + variação) */
  st0: number
  st1: number
  /** duração da sílaba (s) */
  dur: number
  shout: boolean
  /** ganho linear */
  gain: number
}

/**
 * Agenda uma sílaba no contexto atual do Tone (online ou offline). Usa nós
 * nativos: grão por letra, transposto por playbackRate, com envelope curto;
 * a vogal é sustentada repetindo o grão (o "gaguejo" do animalese).
 */
export function playKyai(letters: AudioBuffer, out: Tone.ToneAudioNode, time: number, note: KyaiNote, prof: KyaiProfile) {
  const ctx = Tone.getContext().rawContext as BaseAudioContext
  const { cons, vowels, coda } = phonemes(note.syl)
  const rateAt = (frac: number) => {
    const st = note.st0 + (note.st1 - note.st0) * Math.pow(Math.min(1, Math.max(0, frac)), 1.3) + prof.pitch
    return Math.pow(2, st / 12)
  }
  const end = time + note.dur
  // grãos → volume da sílaba → canal da linha
  const pre = ctx.createGain()
  pre.gain.value = note.gain
  Tone.connect(pre, out)
  const cleanup = () => pre.disconnect()

  const grain = (l: string, t: number, dur: number, rate: number, gain: number) => {
    const info = LETTER[l]
    if (!info) return
    const src = ctx.createBufferSource()
    src.buffer = letters
    src.playbackRate.value = rate
    const g = ctx.createGain()
    const peak = gain * info.gain
    const att = Math.min(0.008, dur / 3)
    const rel = Math.min(0.014, dur / 3)
    g.gain.setValueAtTime(0, t)
    g.gain.linearRampToValueAtTime(peak, t + att)
    g.gain.setValueAtTime(peak, t + dur - rel)
    g.gain.linearRampToValueAtTime(0, t + dur)
    src.connect(g).connect(pre)
    const idx = l.charCodeAt(0) - 97
    const offset = idx * LETTER_SEC + info.start + prof.grit + Math.random() * 0.004
    // duração em tempo de buffer (o playbackRate encolhe/estica)
    src.start(t, offset, Math.min(LETTER_SEC - info.start - prof.grit, dur * rate))
    src.stop(t + dur + 0.01)
  }

  const hiss = (c: Extract<Cons, { kind: 'noise' }>, t: number, dur: number) => {
    const src = ctx.createBufferSource()
    src.buffer = noise(ctx)
    const f = ctx.createBiquadFilter()
    f.type = c.type
    f.frequency.value = c.freq * (0.9 + Math.random() * 0.2)
    f.Q.value = c.q
    const g = ctx.createGain()
    const peak = c.gain * prof.breath
    g.gain.setValueAtTime(0, t)
    g.gain.linearRampToValueAtTime(peak, t + Math.min(0.012, dur / 2))
    g.gain.linearRampToValueAtTime(0, t + dur)
    src.connect(f).connect(g).connect(pre)
    src.start(t, Math.random() * 0.2)
    src.stop(t + dur + 0.01)
  }

  // consoantes do ataque
  let t = time
  const consEnd = time + note.dur * 0.45
  for (const c of cons) {
    const d = Math.min(c.dur * prof.speed, Math.max(0.01, consEnd - t))
    if (c.kind === 'noise') hiss(c, t, d)
    else grain(c.l, t, d, rateAt(0), c.gain ?? 1)
    t += d * 0.85
  }
  // vogais: grãos de 60 ms a cada 40 ms — a sobreposição (20 ms) cobre as
  // rampas de entrada/saída, então a vogal sai contínua (sem o "picotado")
  const codaDur = coda.length ? Math.min(0.05, note.dur * 0.25) : 0
  const vEnd = end - codaDur
  const vDur = Math.max(0.03, vEnd - t)
  const G = 0.06
  const HOP = 0.04
  const nGrains = Math.max(1, Math.ceil((vDur - (G - HOP)) / HOP))
  for (let k = 0; k < nGrains; k++) {
    const gt = t + k * HOP
    const frac = (gt - time) / note.dur
    // ditongo: 2ª vogal ocupa o último terço
    const vi = vowels.length > 1 && (gt - t) / vDur > 0.62 ? vowels.length - 1 : 0
    const fade = note.shout ? 1 - 0.45 * ((gt - t) / vDur) : 1
    grain(vowels[vi] ?? 'a', gt, Math.min(G, vEnd - gt + 0.005), rateAt(frac), fade)
  }
  for (const c of coda) if (c.kind === 'letter') grain(c.l, vEnd, codaDur, rateAt(1), 0.8)
  // solta os nós da sílaba depois que ela acaba (o fim já está agendado)
  const ms = (end + 0.1 - ctx.currentTime) * 1000
  if (!(ctx instanceof OfflineAudioContext)) setTimeout(cleanup, Math.max(0, ms))
}

/**
 * Monta a nota de uma sílaba: contorno japonês + voz do jogador + variação
 * aleatória por grito. `gapSec` = tempo até a próxima sílaba da mesma linha.
 */
export function kyaiNote(e: Pick<KyaiEvent, 'syl' | 'idx' | 'n' | 'shout'>, gapSec: number, prof: KyaiProfile): KyaiNote {
  const v = prof.vary
  const r = () => Math.random() * 2 - 1
  const [c0, c1] = contour(e.idx, e.n, e.shout)
  const scale = prof.range * (1 + r() * 0.35 * v)
  const shift = r() * 1.2 * v
  const base = e.shout ? 0.36 + (e.n === 1 ? 0 : 0.08) : 0.2
  let dur = base * prof.speed * (1 + r() * 0.15 * v)
  if (!e.shout || e.idx < e.n - 1) dur = Math.min(dur, gapSec * 0.95)
  dur = Math.max(0.06, Math.min(dur, gapSec * 0.98))
  return {
    syl: e.syl,
    st0: c0 * scale + shift,
    st1: c1 * scale + shift,
    dur,
    shout: e.shout,
    // kiai fica BEM abaixo dos tambores (~16 dB): é voz de fundo, não solista
    gain: (e.shout ? 0.14 : 0.11) * (1 + r() * 0.12 * v),
  }
}

/** micro-atraso humano (s): gritos raramente caem exatamente no tempo */
export const kyaiLag = (vary: number) => Math.random() * 0.018 * vary
