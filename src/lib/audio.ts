import * as Tone from 'tone'
import type { Song, Track, InstrumentId, Measure, InstrumentDef, ZoneParams } from './types'
import { cellsPerMeasure, BUILTIN_INSTRUMENTS, DYNAMIC_LEVELS } from './types'
import { resolveCell } from './phrase'
import { playOrder, dynamicsCurve, mainMul } from './timeline'
import { kyaiEvents, kyaiProfile, kyaiSeed, kyaiNote, kyaiLag, playKyai, type KyaiEvent } from './kyai'

/**
 * sforzando: ~+6 dB acima da dinâmica do trecho, nunca abaixo de f e com teto
 * um pouco acima de ff — num trecho p, um "don!" estoura pra cima.
 */
function sfzDyn(dyn: number): number {
  return Math.min(DYNAMIC_LEVELS.ff * 1.15, Math.max(DYNAMIC_LEVELS.f, dyn * 2))
}

/** resolver default (builtin do base) — o App injeta um que lê o kit da música */
function defaultResolveDef(track: Track): InstrumentDef {
  return BUILTIN_INSTRUMENTS.find((d) => d.base === track.instrument) ?? BUILTIN_INSTRUMENTS[0]
}

/** true se o instrumento é compartilhado (2+ pessoas) */
function isSharedTrack(track: Track): boolean {
  return (track.personIds?.length ?? 0) >= 2
}

/** dono efetivo de uma célula (espelha store.cellOwnerOf) */
function ownerOfCell(track: Track, meas: Measure, ci: number): string | null {
  const ids = track.personIds ?? []
  if (!ids.length) return null
  if (ids.length === 1) return ids[0]
  const raw = meas.cellOwners?.[track.id]?.[ci] ?? null
  return raw && ids.includes(raw) ? raw : ids[0]
}

/**
 * Vozes baseadas em samples reais (créditos em public/samples/CREDITS.md):
 *  - shime e okedō: golpes isolados de taikos reais numa sala (taikoset1,
 *    whatsanickname4u/Freesound, CC BY 3.0)
 *  - chū, nagadō, ō-nagadō, ōdaiko e bordas: SCC Taiko Drums (CC BY-SA 4.0),
 *    com as camadas de dinâmica do SFZ original
 *  - hira-daiko pequeno: derivado do chū (SCC) — +5 st, corpo curto, mais estalo
 *  - kane: gongo pequeno de santuário (RutgerMuller/Freesound, CC0)
 * Cada zona = lista de layers (suave → forte); cada layer = round robins.
 */

/** dyn = multiplicador de dinâmica (pp..ff); 1 = mf */
interface Voice {
  head: (time: number, vel: number, dyn?: number) => void
  rim: (time: number, vel: number, dyn?: number) => void
}

/** camada de dinâmica: usada quando vel <= maxVel; buffers alternam (round robin) */
interface Layer {
  maxVel: number
  buffers: Tone.ToneAudioBuffer[]
  rr: number
}

const SAMPLE_FILES: Record<InstrumentId, { head: string[][]; rim: string[][] }> = {
  chu: {
    head: [['chu-head-0-0', 'chu-head-0-1'], ['chu-head-1-0', 'chu-head-1-1']],
    rim: [['chu-rim-0-0', 'chu-rim-0-1'], ['chu-rim-1-0', 'chu-rim-1-1', 'chu-rim-1-2'], ['chu-rim-2-0', 'chu-rim-2-1']],
  },
  hira: {
    head: [['hira-head-0-0', 'hira-head-0-1', 'hira-head-0-2'], ['hira-head-1-0', 'hira-head-1-1', 'hira-head-1-2']],
    rim: [['chu-rim-0-0', 'chu-rim-0-1'], ['chu-rim-1-0', 'chu-rim-1-1', 'chu-rim-1-2'], ['chu-rim-2-0', 'chu-rim-2-1']],
  },
  shime: {
    head: [['shime-head-0-0', 'shime-head-0-1', 'shime-head-0-2', 'shime-head-0-3'], ['shime-head-1-0', 'shime-head-1-1', 'shime-head-1-2', 'shime-head-1-3'], ['shime-head-2-0', 'shime-head-2-1', 'shime-head-2-2', 'shime-head-2-3']],
    rim: [['shime-rim-0-0', 'shime-rim-0-1'], ['shime-rim-1-0'], ['shime-rim-2-0', 'shime-rim-2-1']],
  },
  odaiko: {
    head: [['odaiko-head-0-0', 'odaiko-head-0-1'], ['odaiko-head-1-0'], ['odaiko-head-2-0'], ['odaiko-head-3-0']],
    rim: [['chu-rim-0-0', 'chu-rim-0-1'], ['chu-rim-1-0', 'chu-rim-1-1', 'chu-rim-1-2'], ['chu-rim-2-0', 'chu-rim-2-1']],
  },
  nagado: {
    head: [['nagado-head-0-0', 'nagado-head-0-1'], ['nagado-head-1-0', 'nagado-head-1-1', 'nagado-head-1-2']],
    rim: [['chu-rim-0-0', 'chu-rim-0-1'], ['chu-rim-1-0', 'chu-rim-1-1', 'chu-rim-1-2'], ['chu-rim-2-0', 'chu-rim-2-1']],
  },
  onagado: {
    head: [['onagado-head-0-0', 'onagado-head-0-1'], ['onagado-head-1-0', 'onagado-head-1-1'], ['onagado-head-2-0']],
    rim: [['chu-rim-0-0', 'chu-rim-0-1'], ['chu-rim-1-0', 'chu-rim-1-1', 'chu-rim-1-2'], ['chu-rim-2-0', 'chu-rim-2-1']],
  },
  okedo: {
    head: [['okedo-head-0-0', 'okedo-head-0-1', 'okedo-head-0-2', 'okedo-head-0-3'], ['okedo-head-1-0', 'okedo-head-1-1', 'okedo-head-1-2', 'okedo-head-1-3'], ['okedo-head-2-0', 'okedo-head-2-1', 'okedo-head-2-2']],
    rim: [['okedo-rim-0-0', 'okedo-rim-0-1', 'okedo-rim-0-2', 'okedo-rim-0-3']],
  },
  kane: {
    head: [['kane-head-0-0', 'kane-head-0-1', 'kane-head-0-2', 'kane-head-0-3']],
    rim: [['kane-rim-0-0', 'kane-rim-0-1', 'kane-rim-0-2', 'kane-rim-0-3']],
  },
}

/** equilíbrio entre instrumentos (samples já estão no pitch natural de cada tambor) */
const TUNING: Record<InstrumentId, { headGain: number; rimGain: number; rimRate: number; headRate: number }> = {
  chu: { headGain: 1.0, rimGain: 1.0, rimRate: 1, headRate: 1 },
  shime: { headGain: 1.2, rimGain: 0.9, rimRate: 1, headRate: 1 },
  hira: { headGain: 1.1, rimGain: 0.95, rimRate: 1.12, headRate: 1 },
  odaiko: { headGain: 1.0, rimGain: 0.85, rimRate: 0.9, headRate: 1 },
  nagado: { headGain: 0.9, rimGain: 1.0, rimRate: 1, headRate: 1 },
  onagado: { headGain: 0.95, rimGain: 0.95, rimRate: 0.94, headRate: 1 },
  okedo: { headGain: 1.8, rimGain: 1.3, rimRate: 1, headRate: 1 },
  kane: { headGain: 0.8, rimGain: 0.75, rimRate: 1, headRate: 1 },
}

const bufferCache = new Map<string, Tone.ToneAudioBuffer>()

/**
 * Modo standalone (artifact): samples chegam embutidos como data: URI num mapa
 * global. A CSP do artifact bloqueia o fetch que o Tone faria da URL, então
 * decodamos o base64 na mão (atob → decodeAudioData) e populamos o cache. O
 * decode roda no init e `samplesReady` sincroniza o playback pra só tocar depois.
 */
function base64ToBytes(dataUri: string): ArrayBuffer {
  const b64 = dataUri.slice(dataUri.indexOf(',') + 1)
  const bin = atob(b64)
  const bytes = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
  return bytes.buffer
}

export let samplesReady: Promise<unknown> = Promise.resolve()

function decodeEmbedded() {
  const map = (globalThis as Record<string, unknown>).__SAMPLES__ as
    | Record<string, string>
    | undefined
  if (!map) return
  const ctx = Tone.getContext()
  samplesReady = Promise.all(
    Object.entries(map).map(async ([name, uri]) => {
      const audio = await ctx.decodeAudioData(base64ToBytes(uri))
      bufferCache.set(name, new Tone.ToneAudioBuffer(audio))
    }),
  )
}
decodeEmbedded()

function getBuffer(name: string): Tone.ToneAudioBuffer {
  let buf = bufferCache.get(name)
  if (!buf) {
    buf = new Tone.ToneAudioBuffer(`${import.meta.env.BASE_URL}samples/${name}.ogg`)
    bufferCache.set(name, buf)
  }
  return buf
}

/** letras do animalese (vozes dos kiai) */
const kyaiLetters = () => getBuffer('animalese')

function makeLayers(fileLayers: string[][]): Layer[] {
  const n = fileLayers.length
  return fileLayers.map((files, i) => ({
    // divide o range de velocity igualmente entre as layers; última pega até 1
    maxVel: i === n - 1 ? 1.01 : (i + 1) / n,
    buffers: files.map(getBuffer),
    rr: 0,
  }))
}

/** variação humana: ±amt em torno de 1 (cada golpe soa ligeiramente diferente) */
function jitter(amt: number): number {
  return 1 + (Math.random() * 2 - 1) * amt
}

/**
 * velocity → ganho. don (0.7) = referência 0.865; acima cresce em curva
 * (DON/acento ≈ +4 dB); abaixo de 0.5 cai linear até um piso de 10%, pra
 * ghost notes (tsu/ku, vel ≈ 0.05) soarem como toque fantasma (~−18 dB do don).
 */
function velGain(vel: number): number {
  const at = (v: number) => 0.865 * Math.pow(v / 0.7, 1.3)
  return vel >= 0.5 ? at(vel) : at(0.5) * (0.1 + 0.9 * (vel / 0.5))
}

/**
 * Chaves da mixagem (todas ligadas por padrão; expostas no DEV pra testes A/B).
 *  choke   — golpe novo no mesmo tambor abafa a ressonância do anterior
 *  tailCap — cada instrumento tem uma cauda máxima (evita acúmulo embolado)
 *  eq      — EQ por instrumento: corta sub-grave/embolo, realça o ataque
 *  cleanExport — export sem compressor/soft clip: limiter com lookahead em JS
 */
export const MIX = { choke: true, tailCap: true, eq: true, cleanExport: true }

/** cauda máxima (s) por instrumento/zona e duração do abafamento no choke */
const TAIL: Record<InstrumentId, { head: number; rim: number; choke: number }> = {
  shime: { head: 0.45, rim: 0.35, choke: 0.04 },
  chu: { head: 1.3, rim: 0.7, choke: 0.09 },
  hira: { head: 0.7, rim: 0.5, choke: 0.06 },
  nagado: { head: 1.8, rim: 0.7, choke: 0.1 },
  onagado: { head: 2.2, rim: 0.7, choke: 0.12 },
  okedo: { head: 0.8, rim: 0.4, choke: 0.06 },
  odaiko: { head: 2.8, rim: 0.8, choke: 0.14 },
  kane: { head: 1.2, rim: 0.3, choke: 0.05 },
}

/**
 * EQ por instrumento [tipo, freq, ganho dB, Q]: highpass tira sub-grave inútil,
 * corte em ~250–400 Hz desembola, realce em 2,5–4 kHz traz o estalo da baqueta.
 */
type Band = [BiquadFilterType, number, number, number]
const EQ: Record<InstrumentId, { head: Band[]; rim: Band[] }> = {
  shime: { head: [['highpass', 180, 0, 0.7], ['peaking', 3200, 3, 1]], rim: [['highpass', 450, 0, 0.7]] },
  chu: {
    head: [['highpass', 70, 0, 0.7], ['lowshelf', 130, -3, 0.7], ['peaking', 350, -4, 1.2], ['peaking', 2800, 3, 1]],
    rim: [['highpass', 350, 0, 0.7], ['peaking', 3500, 2, 1]],
  },
  nagado: {
    head: [['highpass', 60, 0, 0.7], ['lowshelf', 120, -3, 0.7], ['peaking', 300, -4, 1.2], ['peaking', 2500, 3, 1]],
    rim: [['highpass', 350, 0, 0.7], ['peaking', 3500, 2, 1]],
  },
  onagado: {
    head: [['highpass', 50, 0, 0.7], ['lowshelf', 110, -2, 0.7], ['peaking', 260, -4, 1.2], ['peaking', 2200, 2.5, 1]],
    rim: [['highpass', 350, 0, 0.7]],
  },
  hira: {
    head: [['highpass', 110, 0, 0.7], ['peaking', 380, -3, 1.2], ['peaking', 3000, 2.5, 1]],
    rim: [['highpass', 400, 0, 0.7], ['peaking', 3800, 2, 1]],
  },
  okedo: { head: [['highpass', 110, 0, 0.7], ['peaking', 420, -3, 1.2], ['peaking', 3000, 3, 1]], rim: [['highpass', 400, 0, 0.7]] },
  odaiko: { head: [['highpass', 40, 0, 0.7], ['peaking', 230, -4, 1], ['peaking', 1800, 2, 1]], rim: [['highpass', 350, 0, 0.7]] },
  kane: { head: [['highpass', 600, 0, 0.7]], rim: [['highpass', 700, 0, 0.7]] },
}

/** golpes ainda soando numa zona (p/ choke) */
interface Ringing {
  g: Tone.Gain
  level: number
  end: number
}

function playLayered(
  layers: Layer[],
  out: Tone.ToneAudioNode,
  time: number,
  vel: number,
  gain: number,
  rate: number,
  decay = 0.5,
  dyn = 1,
  tail = Infinity,
): Ringing | null {
  // dinâmica mexe no ganho E na layer (pp soa mais suave, não só mais baixo)
  const layerVel = Math.min(1, vel * Math.sqrt(dyn))
  const layer = layers.find((l) => layerVel <= l.maxVel) ?? layers[layers.length - 1]
  const buffer = layer.buffers[layer.rr % layer.buffers.length]
  layer.rr++
  if (!buffer.loaded) return null
  // humanização: ganho ±12%, pitch ±1% (~17 cents), micro-timing ±6ms — sutil;
  // a variação principal vem dos round robins e das camadas reais
  const level = gain * dyn * velGain(vel) * jitter(0.12)
  const g = new Tone.Gain(level).connect(out)
  const src = new Tone.ToneBufferSource({
    url: buffer,
    playbackRate: rate * jitter(0.01),
    fadeOut: 0.02,
    onended: () => {
      src.dispose()
      g.dispose()
    },
  }).connect(g)
  const start = Math.max(0, time + (Math.random() * 2 - 1) * 0.006)
  src.start(start)
  // duração efetiva: decay < 0.5 encurta (abafado); tailCap limita a cauda.
  // Corta por rampa de ganho, não src.stop(): stop agendado adiantado num
  // OfflineAudioContext (export MP3) silenciava o golpe inteiro.
  const natural = buffer.duration / (rate || 1)
  let dur = natural
  if (decay < 0.5) dur = Math.max(0.03, natural * decay * 2)
  if (MIX.tailCap) dur = Math.min(dur, tail)
  if (dur < natural - 0.01) {
    const fade = Math.min(dur < tail ? 0.02 : 0.25, dur / 2)
    g.gain.setValueAtTime(level, start + dur - fade)
    g.gain.linearRampToValueAtTime(0, start + dur)
  }
  return { g, level, end: start + dur }
}

/** abafa (fade curto) os golpes ainda soando a partir de `t` */
function choke(ring: Ringing[], t: number, fade: number) {
  for (let i = ring.length - 1; i >= 0; i--) {
    const r = ring[i]
    if (r.end <= t) {
      ring.splice(i, 1)
      continue
    }
    if (r.end <= t + fade) continue
    r.g.gain.cancelScheduledValues(t)
    r.g.gain.setValueAtTime(r.level, t)
    r.g.gain.linearRampToValueAtTime(0, t + fade)
    r.end = t + fade
  }
}

const zPitchMul = (p?: ZoneParams) => Math.pow(2, (p?.pitch ?? 0) / 12)
const zGainMul = (p?: ZoneParams) => Math.pow(10, (p?.gain ?? 0) / 20)
const zDecay = (p?: ZoneParams) => p?.decay ?? 0.5

/**
 * Constrói a voz de uma InstrumentDef, com params SEPARADOS por zona (couro=head,
 * borda=rim): pitch (playbackRate), ganho (dB), filtro de timbre (tone + overtones
 * no Q) e decay. Retorna a voz e os nós auxiliares (p/ dispose ao recriar).
 */
function makeVoice(def: InstrumentDef, out: Tone.ToneAudioNode): { voice: Voice; nodes: Tone.ToneAudioNode[] } {
  const nodes: Tone.ToneAudioNode[] = []
  // saída de uma zona: EQ do instrumento + filtro de timbre opcional → out
  const zoneDest = (p: ZoneParams | undefined, bands: Band[]): Tone.ToneAudioNode => {
    let dest: Tone.ToneAudioNode = out
    if (MIX.eq) {
      for (const [type, frequency, g, Q] of [...bands].reverse()) {
        const f = new Tone.Filter({ type, frequency, Q, gain: g, rolloff: -12 }).connect(dest)
        nodes.push(f)
        dest = f
      }
    }
    if (!p?.tone) return dest
    const q = Math.max(0.1, p.tone.q + (p.overtones ?? 0) * 8)
    const filter = new Tone.Filter({ type: p.tone.type, frequency: p.tone.cutoff, Q: q }).connect(dest)
    nodes.push(filter)
    return filter
  }

  const files = SAMPLE_FILES[def.base]
  const tuning = TUNING[def.base]
  const tail = TAIL[def.base]
  const eq = EQ[def.base]
  const headLayers = makeLayers(files.head)
  const rimLayers = makeLayers(files.rim)
  const headDest = zoneDest(def.head, eq.head)
  const rimDest = zoneDest(def.rim, eq.rim)
  // choke: couro abafa couro e borda anteriores (a pele parou de vibrar);
  // borda só abafa borda (o couro continua soando)
  const headRing: Ringing[] = []
  const rimRing: Ringing[] = []
  const track = (ring: Ringing[], r: Ringing | null) => r && ring.push(r)

  return {
    voice: {
      head: (t, v, d = 1) => {
        if (MIX.choke) {
          choke(headRing, t, tail.choke)
          choke(rimRing, t, tail.choke)
        }
        track(headRing, playLayered(headLayers, headDest, t, v, tuning.headGain * zGainMul(def.head), tuning.headRate * zPitchMul(def.head), zDecay(def.head), d, tail.head))
      },
      rim: (t, v, d = 1) => {
        if (MIX.choke) choke(rimRing, t, tail.choke)
        track(rimRing, playLayered(rimLayers, rimDest, t, v, tuning.rimGain * zGainMul(def.rim), tuning.rimRate * zPitchMul(def.rim), zDecay(def.rim), d, tail.rim))
      },
    },
    nodes,
  }
}

/**
 * Warp de swing: desloca a metade do tempo em direção ao terço (hane/shuffle).
 * f = posição dentro do tempo [0,1); s = intensidade 0..1 (1 = feel ternário).
 */
export function swingWarp(f: number, s: number): number {
  if (s <= 0) return f
  const mid = 0.5 + s * (2 / 3 - 0.5)
  return f <= 0.5 ? (f * mid) / 0.5 : mid + ((f - 0.5) * (1 - mid)) / 0.5
}

export class Engine {
  /** canais keyed por channelKey (track.id ou `${track.id}::${personId}` p/ compart.) */
  private channels = new Map<string, Tone.Channel>()
  private voices = new Map<string, Voice>()
  /** barramento (submix) por pessoa → master */
  private personBuses = new Map<string, Tone.Channel>()
  /** ouvinte de golpe individual (highlight por nota na partitura) */
  private hitListener: ((key: string) => void) | null = null
  private loopTimer: ReturnType<typeof setInterval> | null = null
  private loopToken = 0
  /** canal separado p/ preview de frase: mutar mata a cauda já agendada */
  private previewChannel: Tone.Channel | null = null
  private previewVoices = new Map<string, Voice>()
  /** assinatura da def usada em cada canal (p/ recriar a voz quando os params mudam) */
  private voiceSig = new Map<string, string>()
  /** nós auxiliares (filtro/synth) por canal, p/ dispose ao recriar */
  private voiceNodes = new Map<string, Tone.ToneAudioNode[]>()
  /** canais dos kiai: 1 por linha/dono, roteado como os golpes (mudo/solo/pan da linha) */
  private kyaiChannels = new Map<string, { ch: Tone.Channel; trackId: string }>()
  /** resolvedor de def por track (injetado pelo App; lê o kit da música) */
  private resolveDef: (track: Track) => InstrumentDef = defaultResolveDef

  setInstrumentResolver(fn: (track: Track) => InstrumentDef) {
    this.resolveDef = fn
  }

  private previewVoice(def: InstrumentDef): Voice {
    if (!this.previewChannel) {
      this.previewChannel = new Tone.Channel({ volume: -6 }).toDestination()
    }
    const key = JSON.stringify(def)
    let voice = this.previewVoices.get(key)
    if (!voice) {
      voice = makeVoice(def, this.previewChannel).voice
      this.previewVoices.set(key, voice)
    }
    return voice
  }

  setHitListener(fn: ((key: string) => void) | null) {
    this.hitListener = fn
  }

  /**
   * Chave do canal do golpe. Codifica o DESTINO (barramento) na chave para que,
   * ao mudar a pertença/dono, uma nova chave seja criada com o roteamento certo
   * (o canal antigo fica órfão e silencioso, pois nada mais agenda nele).
   */
  private channelKey(track: Track, ownerId: string | null): string {
    if (isSharedTrack(track) && ownerId) return `${track.id}::${ownerId}`
    const ids = track.personIds ?? []
    return `${track.id}@${ids.length === 1 ? ids[0] : 'master'}`
  }

  private ensurePersonBus(personId: string): Tone.Channel {
    let bus = this.personBuses.get(personId)
    if (!bus) {
      bus = new Tone.Channel({ volume: 0 }).toDestination()
      this.personBuses.set(personId, bus)
    }
    return bus
  }

  private disposeChannel(key: string) {
    this.voiceNodes.get(key)?.forEach((n) => n.dispose())
    this.channels.get(key)?.dispose()
    this.voiceNodes.delete(key)
    this.channels.delete(key)
    this.voices.delete(key)
    this.voiceSig.delete(key)
  }

  /**
   * Cria (ou recria) o canal+voz de um golpe, roteando pro barramento certo.
   * Recria quando os params da InstrumentDef mudam (assinatura diferente).
   */
  private ensureVoice(track: Track, ownerId: string | null): Voice {
    const key = this.channelKey(track, ownerId)
    const def = this.resolveDef(track)
    const sig = JSON.stringify(def)
    const existing = this.voices.get(key)
    if (existing && this.voiceSig.get(key) === sig) return existing
    this.disposeChannel(key)
    const ids = track.personIds ?? []
    let dest: Tone.ToneAudioNode
    if (isSharedTrack(track) && ownerId) dest = this.ensurePersonBus(ownerId)
    else if (ids.length === 1) dest = this.ensurePersonBus(ids[0])
    else dest = Tone.getDestination()
    const channel = new Tone.Channel({ volume: track.volume ?? -6, pan: track.pan ?? 0 }).connect(dest)
    const { voice, nodes } = makeVoice(def, channel)
    this.channels.set(key, channel)
    this.voices.set(key, voice)
    this.voiceNodes.set(key, nodes)
    this.voiceSig.set(key, sig)
    return voice
  }

  /** canal dos kiai de uma linha (por dono, se compartilhada) */
  private kyaiChannel(track: Track, ownerId: string | null): Tone.Channel {
    const key = this.channelKey(track, ownerId) + '#kyai'
    let entry = this.kyaiChannels.get(key)
    if (!entry) {
      const ids = track.personIds ?? []
      let dest: Tone.ToneAudioNode
      if (isSharedTrack(track) && ownerId) dest = this.ensurePersonBus(ownerId)
      else if (ids.length === 1) dest = this.ensurePersonBus(ids[0])
      else dest = Tone.getDestination()
      const ch = new Tone.Channel({ volume: track.kyai?.gain ?? 0, pan: track.pan ?? 0 }).connect(dest)
      entry = { ch, trackId: track.id }
      this.kyaiChannels.set(key, entry)
    }
    return entry.ch
  }

  /** garante os canais de uma track: 1 por dono se compartilhada, senão 1 só */
  private ensureTrack(track: Track) {
    if (isSharedTrack(track)) for (const pid of track.personIds ?? []) this.ensureVoice(track, pid)
    else this.ensureVoice(track, null)
  }

  /** canais (keys) de uma track no grafo atual */
  private trackChannelKeys(track: Track): string[] {
    if (isSharedTrack(track)) return (track.personIds ?? []).map((pid) => `${track.id}::${pid}`)
    return [this.channelKey(track, null)]
  }

  /**
   * Aplica mix. Mudo/solo resolvidos MANUALMENTE (via channel.mute local, sem o
   * solo global do Tone) em 2 camadas: track (canais) e pessoa (barramentos).
   * Um golpe some se seu canal de track OU o barramento do dono estiver mudo.
   */
  applyMix(song: Pick<Song, 'tracks' | 'people'>) {
    const people = song.people ?? []
    const anyPersonSolo = people.some((p) => p.solo)
    for (const person of people) {
      const bus = this.personBuses.get(person.id)
      if (!bus) continue
      const audible = !person.muted && (!anyPersonSolo || !!person.solo)
      bus.mute = !audible
      bus.volume.value = person.volume ?? 0
    }
    const anyTrackSolo = song.tracks.some((t) => t.solo)
    for (const track of song.tracks) {
      const audible = !track.muted && (!anyTrackSolo || track.solo)
      for (const key of this.trackChannelKeys(track)) {
        const channel = this.channels.get(key)
        if (!channel) continue
        channel.mute = !audible
        channel.volume.value = track.volume ?? -6
        channel.pan.value = track.pan ?? 0
      }
      for (const { ch, trackId } of this.kyaiChannels.values()) {
        if (trackId !== track.id) continue
        ch.mute = !audible
        ch.volume.value = track.kyai?.gain ?? 0
        ch.pan.value = track.pan ?? 0
      }
    }
  }

  setBpm(bpm: number) {
    Tone.getTransport().bpm.value = bpm
  }

  /**
   * Posição do playback em segundos NA MÚSICA COMO ESCRITA (relógio da
   * esteira/animação): o compasso tocado é dobrado de volta pro compasso-fonte,
   * então numa repetição (ritornello) o playhead volta pro início da seção.
   */
  transportSeconds(): number {
    const transport = Tone.getTransport()
    const quarters = transport.ticks / transport.PPQ
    const u = Math.floor(quarters / 4)
    const src = this.order[u]
    const folded = src === undefined ? quarters : src * 4 + (quarters - u * 4)
    return (folded * 60) / transport.bpm.value
  }

  /** ordem de execução atual: compasso tocado → compasso-fonte */
  private order: number[] = []

  /** região de compassos [startBar, endBar) pra tocar/loopar só uma seção */
  private region: { startBar: number; endBar: number } | null = null

  private metroEnabled = false
  private metroSynth: Tone.Synth | null = null
  private trainerTimer: ReturnType<typeof setInterval> | null = null

  setMetronome(on: boolean) {
    this.metroEnabled = on
  }

  private metroClick(time: number, accent: boolean) {
    if (!this.metroSynth) {
      this.metroSynth = new Tone.Synth({
        oscillator: { type: 'triangle' },
        envelope: { attack: 0.001, decay: 0.05, sustain: 0, release: 0.02 },
        volume: -8,
      }).toDestination()
    }
    this.metroSynth.triggerAttackRelease(accent ? 'C6' : 'G5', 0.03, time, accent ? 1 : 0.6)
  }

  setLoop(loop: boolean, song: Song) {
    const transport = Tone.getTransport()
    transport.loop = loop
    transport.loopStart = 0
    transport.loopEnd = `${playOrder(song, this.region).length}:0:0`
  }

  /** Reagenda a música inteira no Transport (tempo musical: segue BPM). */
  schedule(song: Song, onCursor: (cellIndex: number) => void) {
    const transport = Tone.getTransport()
    transport.cancel()
    const swing = song.swing ?? 0
    // garante os canais do grafo atual (membership de pessoa pode ter mudado)
    for (const track of song.tracks) this.ensureTrack(track)

    // ordem desdobrada (ritornellos), da música ou só da região selecionada;
    // o transport segue essa ordem. Cursor/golpes usam o índice da célula-FONTE.
    const order = playOrder(song, this.region)
    this.order = order
    const bars = order.map((mi, u) => ({ pos: u, mi }))
    // curva de dinâmica por instrumento (geral + marcações próprias)
    const dynCurves = new Map(song.tracks.map((t) => [t.id, dynamicsCurve(song, order, t.id)]))
    const cellStart = measureCellStarts(song)

    for (const { pos, mi } of bars) {
      const meas = song.measures[mi]
      const nCells = cellsPerMeasure(meas)

      for (let ci = 0; ci < nCells; ci++) {
        const beat = Math.floor(ci / meas.subdivision)
        const cellFrac = (ci % meas.subdivision) / meas.subdivision
        const cellTime = `${pos}:${beat}:${swingWarp(cellFrac, swing) * 4}`
        const globalIdx = cellStart[mi] + ci

        for (const track of song.tracks) {
          const syl = meas.cells[track.id]?.[ci]
          if (!syl) continue
          const resolved = resolveCell(syl)
          if (!resolved) continue
          // instrumento compartilhado: golpe roteia pro barramento do dono da célula
          const owner = ownerOfCell(track, meas, ci)
          const voice = this.voices.get(this.channelKey(track, owner))
          if (!voice) continue
          resolved.hits.forEach((hit, hi) => {
            // posição do golpe como fração do TEMPO, com warp de swing
            const beatFrac = swingWarp(cellFrac + hit.offset / meas.subdivision, swing)
            const time = `${pos}:${beat}:${beatFrac * 4}`
            const key = `${track.id}:${globalIdx}:${hi}`
            const d0 = dynCurves.get(track.id)!(pos, (beat + beatFrac) / meas.beats)
            const dyn = (hit.sfz ? sfzDyn(d0) : d0) * mainMul(song, track.id)
            transport.schedule((t) => {
              voice[hit.zone](t, hit.vel, dyn)
              if (this.hitListener) {
                Tone.getDraw().schedule(() => this.hitListener?.(key), t)
              }
            }, time)
          })
        }

        transport.schedule((t) => {
          Tone.getDraw().schedule(() => onCursor(globalIdx), t)
        }, cellTime)
      }
    }

    // kiai: cada sílaba com a voz do dono da célula e entonação da palavra
    const letters = kyaiLetters()
    for (const e of kyaiEvents(song, order, (f) => swingWarp(f, swing))) {
      const meas = song.measures[e.mi]
      const owner = ownerOfCell(e.track, meas, e.ci)
      const ch = this.kyaiChannel(e.track, owner)
      const prof = kyaiProfile(kyaiSeed(e.track, meas, e.ci), e.track)
      transport.schedule((t) => {
        if (!letters.loaded) return
        const spb = 60 / transport.bpm.value
        playKyai(letters.get()!, ch, t + kyaiLag(prof.vary), kyaiNote(e, e.gapBeats * spb, prof), prof)
      }, `${e.u}:${e.beat}:${e.frac * 4}`)
    }

    // metrônomo: um clique por tempo, acento no tempo 1
    if (this.metroEnabled) {
      for (const { pos, mi } of bars) {
        for (let b = 0; b < song.measures[mi].beats; b++) {
          transport.schedule((t) => this.metroClick(t, b === 0), `${pos}:${b}:0`)
        }
      }
    }
    // canais recém-criados acima ganham o mix correto (mudo/solo/vol de track e pessoa)
    this.applyMix(song)
  }

  /**
   * Loop de preview de uma frase (hover no catálogo): agenda golpes com
   * lookahead fora do Transport. unitSec = duração de cada palavra.
   */
  async startPhraseLoop(def: InstrumentDef, words: string[], unitSec: number, swing = 0) {
    this.stopPhraseLoop()
    const token = ++this.loopToken
    void Tone.start()
    // hover não é gesto de usuário: se o contexto ainda não rodou (nenhum
    // clique na página), não dá pra tocar — sai em vez de pendurar no await
    if (Tone.getContext().state !== 'running') return
    const voice = this.previewVoice(def)
    await Promise.all([Tone.loaded(), samplesReady])
    if (token !== this.loopToken) return // hover já saiu durante o carregamento
    this.previewChannel!.mute = false
    const total = words.length * unitSec
    let next = Tone.now() + 0.08
    const pump = () => {
      const horizon = Tone.now() + total
      while (next < horizon) {
        const start = next
        words.forEach((word, i) => {
          const resolved = resolveCell(word)
          if (!resolved) return
          for (const hit of resolved.hits) {
            voice[hit.zone](start + (i + swingWarp(hit.offset, swing)) * unitSec, hit.vel)
          }
        })
        next += total
      }
    }
    pump()
    this.loopTimer = setInterval(pump, Math.min(250, total * 500))
  }

  stopPhraseLoop() {
    this.loopToken++
    if (this.loopTimer !== null) {
      clearInterval(this.loopTimer)
      this.loopTimer = null
    }
    // mata a cauda: fontes já agendadas no canal de preview ficam mudas
    if (this.previewChannel) this.previewChannel.mute = true
  }

  /** posição no transport (bars:beats:sixteenths) do início de uma célula global */
  private cellPosition(song: Song, globalCell: number): string {
    let idx = 0
    for (let mi = 0; mi < song.measures.length; mi++) {
      const meas = song.measures[mi]
      const n = cellsPerMeasure(meas)
      if (globalCell < idx + n) {
        const ci = globalCell - idx
        const beat = Math.floor(ci / meas.subdivision)
        const frac = (ci % meas.subdivision) / meas.subdivision
        // com ritornellos, vai pra 1ª vez que o compasso toca
        const u = Math.max(0, playOrder(song, this.region).indexOf(mi))
        return `${u}:${beat}:${frac * 4}`
      }
      idx += n
    }
    return '0:0:0'
  }

  /** reposiciona o playback em andamento pra uma célula (seek ao vivo) */
  seek(song: Song, startCell: number) {
    Tone.getTransport().position = this.cellPosition(song, startCell)
  }

  async play(
    song: Song,
    loop: boolean,
    onCursor: (cellIndex: number) => void,
    startCell = 0,
    region: { startBar: number; endBar: number } | null = null,
    onEnd?: () => void,
    opts: {
      countIn?: boolean
      trainer?: { startPct: number; step: number } | null
      onBpm?: (bpm: number) => void
    } = {},
  ) {
    this.stopPhraseLoop()
    if (this.trainerTimer) {
      clearInterval(this.trainerTimer)
      this.trainerTimer = null
    }
    this.region = region
    await Tone.start()
    kyaiLetters()
    for (const track of song.tracks) this.ensureTrack(track)
    await Promise.all([Tone.loaded(), samplesReady])
    this.applyMix(song)

    const base = song.tempo
    const useTrainer = !!(opts.trainer && region && loop)
    const startBpm = useTrainer ? Math.max(20, Math.round(base * opts.trainer!.startPct)) : base
    this.setBpm(startBpm)
    opts.onBpm?.(startBpm)

    this.schedule(song, onCursor)
    this.setLoop(loop, song)
    const transport = Tone.getTransport()
    transport.position = region ? 0 : this.cellPosition(song, startCell)

    // treino: a cada volta do loop, sobe o BPM até o tempo base
    if (useTrainer) {
      const step = opts.trainer!.step
      let lastTicks = transport.ticks
      this.trainerTimer = setInterval(() => {
        const t = transport.ticks
        if (t < lastTicks) {
          // o transport voltou pro início do loop = completou uma volta
          const next = Math.min(base, Math.round(transport.bpm.value + base * step))
          transport.bpm.value = next
          opts.onBpm?.(next)
        }
        lastTicks = t
      }, 60)
    }

    // seção sem loop: para no fim da região
    if (region && !loop) {
      transport.scheduleOnce((t) => {
        Tone.getDraw().schedule(() => onEnd?.(), t)
      }, `${playOrder(song, region).length}:0:0`)
    }

    // count-in: 1 compasso (4 tempos) de cliques antes de começar
    if (opts.countIn) {
      const spb = 60 / startBpm
      const n = 4
      const now = Tone.now() + 0.1
      for (let i = 0; i < n; i++) this.metroClick(now + i * spb, i === 0)
      transport.start(now + n * spb)
    } else {
      transport.start('+0.05')
    }
  }

  stop() {
    if (this.trainerTimer) {
      clearInterval(this.trainerTimer)
      this.trainerTimer = null
    }
    const transport = Tone.getTransport()
    transport.stop()
    transport.cancel()
    transport.position = 0
  }

  /**
   * Preview de um kiai ao escrever: as sílabas tocam uma por `unitSec`
   * (o passo da célula), com a voz do dono e a entonação da palavra.
   */
  async previewKyai(track: Track, syls: string[], unitSec: number, seedId?: string) {
    await Tone.start()
    const letters = kyaiLetters()
    await Promise.all([Tone.loaded(), samplesReady])
    if (!letters.loaded || !syls.length) return
    const owner = isSharedTrack(track) ? (seedId ?? track.personIds![0]) : null
    const ch = this.kyaiChannel(track, owner)
    ch.mute = false
    ch.volume.value = track.kyai?.gain ?? 0
    const prof = kyaiProfile(seedId ?? track.personIds?.[0] ?? track.id, track)
    // agrupa em palavras como na música (quebra depois de cada "!")
    let start = 0
    const now = Tone.now() + 0.03
    syls.forEach((syl, i) => {
      const shout = syl.endsWith('!')
      if (i > 0 && syls[i - 1].endsWith('!')) start = i
      let end = i
      while (end < syls.length - 1 && !syls[end].endsWith('!')) end++
      const e = { syl, idx: i - start, n: end - start + 1, shout }
      const gap = i < syls.length - 1 ? unitSec : Infinity
      playKyai(letters.get()!, ch, now + i * unitSec, kyaiNote(e, gap, prof), prof)
    })
  }

  /** Preview do conteúdo de uma célula ao escrever (fora do transport). */
  async preview(track: Track, syl: string) {
    await Tone.start()
    const voice = this.ensureVoice(track, track.personIds?.[0] ?? null)
    const resolved = resolveCell(syl)
    if (!resolved) return
    await Promise.all([Tone.loaded(), samplesReady])
    const now = Tone.now()
    for (const hit of resolved.hits) voice[hit.zone](now + hit.offset * 0.2, hit.vel, hit.sfz ? sfzDyn(1) : 1)
  }

  /**
   * Renderiza a música inteira (com ritornellos, dinâmica, swing e mix atual)
   * num OfflineAudioContext — mais rápido que tempo real, sem tocar nada.
   */
  async renderOffline(song: Song, sampleRate = 44100): Promise<AudioBuffer> {
    await Tone.start()
    const letters = kyaiLetters()
    // pré-carrega os samples no contexto online; o AudioBuffer serve pro offline
    for (const track of song.tracks) {
      const files = SAMPLE_FILES[this.resolveDef(track).base]
      if (files) for (const name of [...files.head, ...files.rim].flat()) getBuffer(name)
    }
    await Promise.all([Tone.loaded(), samplesReady])

    const spb = 60 / song.tempo
    const swing = song.swing ?? 0
    const order = playOrder(song)
    const dynCurves = new Map(song.tracks.map((t) => [t.id, dynamicsCurve(song, order, t.id)]))
    const lead = 0.1
    const duration = lead + order.length * 4 * spb + 3 // cauda p/ ressonância final

    // mesma lógica de mudo/solo do applyMix, mas quem não soa nem é agendado
    const people = song.people ?? []
    const anyPersonSolo = people.some((p) => p.solo)
    const personAudible = (id: string) => {
      // pessoa inexistente (removida) soa, como no playback ao vivo
      const p = people.find((x) => x.id === id)
      if (!p) return true
      return !p.muted && (!anyPersonSolo || !!p.solo)
    }
    const anyTrackSolo = song.tracks.some((t) => t.solo)

    // Render em janelas: o OfflineAudioContext processa TODO nó conectado a cada
    // bloco de áudio, inclusive os que ainda não começaram. Criar todos os golpes
    // de uma vez deixava o custo quadrático (música de 3 min > 10 min de render).
    // Agora os golpes de cada janela de 2 s são criados só quando o render chega
    // perto dela (suspend/resume), e os que já acabaram são descartados.
    // contexto NATIVO (o wrapper padrão do Tone não expõe suspend())
    const raw = new OfflineAudioContext(2, Math.ceil(duration * sampleRate), sampleRate)
    const ctx = new Tone.OfflineContext(raw)
    const prevCtx = Tone.getContext()
    Tone.setContext(ctx)
    let rendered: AudioBuffer
    try {
      // export limpo: só headroom aqui; o teto vem do limiter com lookahead depois
      if (MIX.cleanExport) Tone.getDestination().chain(new Tone.Gain(0.5))
      else addMasterLimiter()
      const buses = new Map<string, Tone.Channel>()
      const bus = (pid: string) => {
        let b = buses.get(pid)
        if (!b) {
          b = new Tone.Channel({ volume: people.find((p) => p.id === pid)?.volume ?? 0 }).toDestination()
          buses.set(pid, b)
        }
        return b
      }
      const voices = new Map<string, Voice>()
      const voiceFor = (track: Track, owner: string | null): Voice | null => {
        const ids = track.personIds ?? []
        const busId = isSharedTrack(track) ? owner : ids.length === 1 ? ids[0] : null
        if (busId && !personAudible(busId)) return null
        const key = `${track.id}::${busId ?? 'master'}`
        let v = voices.get(key)
        if (!v) {
          const dest = busId ? bus(busId) : Tone.getDestination()
          const ch = new Tone.Channel({ volume: track.volume ?? -6, pan: track.pan ?? 0 }).connect(dest)
          v = makeVoice(this.resolveDef(track), ch).voice
          voices.set(key, v)
        }
        return v
      }

      // lista de golpes em ordem de tempo (o choke depende da ordem por voz)
      const hits: { t: number; voice: Voice; zone: 'head' | 'rim'; vel: number; dyn: number }[] = []
      order.forEach((mi, u) => {
        const meas = song.measures[mi]
        const nCells = cellsPerMeasure(meas)
        for (let ci = 0; ci < nCells; ci++) {
          const beat = Math.floor(ci / meas.subdivision)
          const cellFrac = (ci % meas.subdivision) / meas.subdivision
          for (const track of song.tracks) {
            if (track.muted || (anyTrackSolo && !track.solo)) continue
            const syl = meas.cells[track.id]?.[ci]
            if (!syl) continue
            const resolved = resolveCell(syl)
            if (!resolved) continue
            const voice = voiceFor(track, ownerOfCell(track, meas, ci))
            if (!voice) continue
            for (const hit of resolved.hits) {
              const beatFrac = swingWarp(cellFrac + hit.offset / meas.subdivision, swing)
              const t = lead + (u * 4 + beat + beatFrac) * spb
              const d0 = dynCurves.get(track.id)!(u, (beat + beatFrac) / meas.beats)
              hits.push({ t, voice, zone: hit.zone, vel: hit.vel, dyn: (hit.sfz ? sfzDyn(d0) : d0) * mainMul(song, track.id) })
            }
          }
        }
      })
      // kiai: entram na mesma fila (por tempo) como chamadas avulsas
      const kyaiCalls: { t: number; run: () => void }[] = []
      const kyaiChs = new Map<string, Tone.Channel>()
      const kyaiOut = (track: Track, owner: string | null): Tone.Channel | null => {
        const ids = track.personIds ?? []
        const busId = isSharedTrack(track) ? owner : ids.length === 1 ? ids[0] : null
        if (busId && !personAudible(busId)) return null
        const key = `${track.id}::${busId ?? 'master'}`
        let ch = kyaiChs.get(key)
        if (!ch) {
          const dest = busId ? bus(busId) : Tone.getDestination()
          ch = new Tone.Channel({ volume: track.kyai?.gain ?? 0, pan: track.pan ?? 0 }).connect(dest)
          kyaiChs.set(key, ch)
        }
        return ch
      }
      if (letters.loaded) {
        for (const e of kyaiEvents(song, order, (f) => swingWarp(f, swing)) as KyaiEvent[]) {
          if (e.track.muted || (anyTrackSolo && !e.track.solo)) continue
          const meas = song.measures[e.mi]
          const out = kyaiOut(e.track, ownerOfCell(e.track, meas, e.ci))
          if (!out) continue
          const prof = kyaiProfile(kyaiSeed(e.track, meas, e.ci), e.track)
          const t = lead + e.beats * spb + kyaiLag(prof.vary)
          kyaiCalls.push({ t, run: () => playKyai(letters.get()!, out, t, kyaiNote(e, e.gapBeats * spb, prof), prof) })
        }
        kyaiCalls.sort((a, b) => a.t - b.t)
      }
      hits.sort((a, b) => a.t - b.t)

      const WIN = 2
      let next = 0
      let nextK = 0
      const scheduleUntil = (tEnd: number) => {
        while (next < hits.length && hits[next].t < tEnd) {
          const h = hits[next++]
          h.voice[h.zone](h.t, h.vel, h.dyn)
        }
        while (nextK < kyaiCalls.length && kyaiCalls[nextK].t < tEnd) kyaiCalls[nextK++].run()
      }
      // janela atual + a próxima já agendadas; a cada suspend, agenda mais uma
      scheduleUntil(2 * WIN)
      for (let w = WIN; w < duration - 0.01; w += WIN) {
        const at = w
        void raw.suspend(at).then(() => {
          scheduleUntil(at + 2 * WIN)
          void raw.resume()
        })
      }
      rendered = await raw.startRendering()
    } finally {
      Tone.setContext(prevCtx)
    }
    const out = rendered
    if (MIX.cleanExport) lookaheadLimit(out)
    return out
  }

  /** Toca UMA batida de uma zona (couro/borda) de uma def — usado no editor de Kit. */
  async previewInstrument(def: InstrumentDef, zone: 'head' | 'rim') {
    await Tone.start()
    const voice = this.previewVoice(def)
    await Promise.all([Tone.loaded(), samplesReady])
    if (this.previewChannel) this.previewChannel.mute = false
    voice[zone](Tone.now() + 0.02, 1)
  }
}

export const engine = new Engine()

/**
 * Cadeia do master: headroom (−6 dB) → compressor rápido → soft clip (tanh).
 * f/ff somam até +10 dB por instrumento; sem headroom o mix em mf já ficava
 * perto de 0 dBFS e os fortes clipavam. O Tone.Limiter (ataque 3 ms) não
 * segurava o transiente do taiko, então o soft clip garante o teto.
 * Usado no play ao vivo e no export (contexto offline).
 */
function softClipCurve(n = 4096): Float32Array {
  const c = new Float32Array(n)
  for (let i = 0; i < n; i++) {
    const x = (i / (n - 1)) * 2 - 1
    // linear até ~−1 dBFS (só pega o que escapou do compressor), satura até 0.98
    c[i] = Math.abs(x) < 0.9 ? x : Math.sign(x) * (0.9 + 0.08 * Math.tanh((Math.abs(x) - 0.9) / 0.08))
  }
  return c
}

function addMasterLimiter() {
  const headroom = new Tone.Gain(0.5)
  const comp = new Tone.Compressor({ threshold: -8, ratio: 4, attack: 0.002, release: 0.15, knee: 6 })
  const clip = new Tone.WaveShaper(softClipCurve())
  Tone.getDestination().chain(headroom, comp, clip)
}
addMasterLimiter()

/**
 * Limiter com lookahead (só no export, onde o áudio inteiro já existe): o ganho
 * começa a descer 5 ms ANTES do pico e volta devagar (release ~80 ms) — segura
 * o teto sem cortar a onda (sem distorção). Antes aplica um ganho de
 * normalização que deixa no máx. ~5 dB pro limiter trabalhar.
 */
function lookaheadLimit(buf: AudioBuffer, ceilingDb = -1.5) {
  const sr = buf.sampleRate
  const chans = Array.from({ length: buf.numberOfChannels }, (_, c) => buf.getChannelData(c))
  const n = chans[0].length
  const ceiling = Math.pow(10, ceilingDb / 20)
  const peakAt = new Float32Array(n)
  let pk = 0
  for (let i = 0; i < n; i++) {
    let v = 0
    for (const ch of chans) v = Math.max(v, Math.abs(ch[i]))
    peakAt[i] = v
    pk = Math.max(pk, v)
  }
  if (pk === 0) return
  const pre = Math.min(4, (ceiling * 1.78) / pk) // até +12 dB de ganho, no máx. ~5 dB de limitação
  const look = Math.max(1, Math.round(0.005 * sr))
  // ganho necessário por amostra e mínimo na janela à frente (lookahead)
  const need = new Float32Array(n)
  for (let i = 0; i < n; i++) need[i] = Math.min(1, ceiling / Math.max(1e-9, peakAt[i] * pre))
  const ahead = new Float32Array(n)
  // mínimo deslizante (deque monotônico em array fixo; head avança em vez de shift)
  const dq = new Int32Array(n)
  let head = 0
  let tail = 0
  for (let i = n - 1; i >= 0; i--) {
    while (tail > head && need[dq[tail - 1]] >= need[i]) tail--
    dq[tail++] = i
    while (dq[head] > i + look) head++
    ahead[i] = need[dq[head]]
  }
  // ataque suave (média móvel do tamanho do lookahead, buffer circular) + release exponencial
  const rel = 1 - Math.exp(-1 / (0.08 * sr))
  let g = 1
  let acc = 0
  const ring = new Float32Array(look)
  for (let i = 0; i < n; i++) {
    const slot = i % look
    if (i >= look) acc -= ring[slot]
    ring[slot] = ahead[i]
    acc += ahead[i]
    const target = Math.min(ahead[i], acc / Math.min(i + 1, look))
    g = target < g ? target : g + (target - g) * rel
    const k = pre * g
    for (const ch of chans) ch[i] *= k
  }
}

/** índice global da 1ª célula de cada compasso */
function measureCellStarts(song: Song): number[] {
  const out: number[] = []
  let acc = 0
  for (const m of song.measures) {
    out.push(acc)
    acc += cellsPerMeasure(m)
  }
  return out
}

/**
 * Easter egg 🌸 — "Sakura Sakura" em fue (shinobue) sintetizado.
 * Triângulo + vibrato + sopro de ruído rosa filtrado. Escala in (lá si dó mi fá).
 * Retorna a duração total em ms (p/ sincronizar animação do logo).
 */
let fuePlaying = false

export async function playFue(): Promise<number> {
  if (fuePlaying) return 0
  fuePlaying = true
  await Tone.start()

  const q = 0.42 // segundos por semínima
  // [nota, duração em semínimas] — frase de abertura de Sakura Sakura
  const MELODY: [string, number][] = [
    ['A5', 1], ['A5', 1], ['B5', 2],
    ['A5', 1], ['A5', 1], ['B5', 2],
    ['A5', 1], ['B5', 1], ['C6', 1], ['B5', 1],
    ['A5', 1], ['B5', 0.5], ['A5', 0.5], ['F5', 1],
    ['E5', 3],
  ]
  const totalBeats = MELODY.reduce((acc, [, b]) => acc + b, 0)
  const totalSec = totalBeats * q

  const out = new Tone.Gain(0.4).toDestination()
  const vibrato = new Tone.Vibrato({ frequency: 5.2, depth: 0.1 }).connect(out)
  const synth = new Tone.Synth({
    oscillator: { type: 'triangle' },
    portamento: 0.02,
    envelope: { attack: 0.07, decay: 0.12, sustain: 0.8, release: 0.35 },
  }).connect(vibrato)

  // sopro: ruído rosa em bandpass acompanha a melodia, bem baixo
  const breathGain = new Tone.Gain(0).connect(out)
  const breathFilter = new Tone.Filter(2600, 'bandpass').connect(breathGain)
  const breath = new Tone.Noise('pink').connect(breathFilter)

  const now = Tone.now() + 0.08
  breath.start(now)
  breathGain.gain.setValueAtTime(0, now)
  breathGain.gain.linearRampToValueAtTime(0.035, now + 0.15)
  breathGain.gain.setValueAtTime(0.035, now + totalSec - 0.3)
  breathGain.gain.linearRampToValueAtTime(0, now + totalSec + 0.3)
  breath.stop(now + totalSec + 0.4)

  let t = now
  for (const [note, beats] of MELODY) {
    synth.triggerAttackRelease(note, beats * q * 0.92, t)
    t += beats * q
  }

  setTimeout(
    () => {
      synth.dispose()
      vibrato.dispose()
      breath.dispose()
      breathFilter.dispose()
      breathGain.dispose()
      out.dispose()
      fuePlaying = false
    },
    (totalSec + 1) * 1000,
  )

  return totalSec * 1000
}

// autoplay policy: destrava o AudioContext no primeiro gesto real da página,
// pra hover-preview (que não é gesto) já encontrar o contexto rodando
if (typeof document !== 'undefined') {
  const unlock = () => void Tone.start()
  document.addEventListener('pointerdown', unlock, { once: true })
  document.addEventListener('keydown', unlock, { once: true })
}

// inspeção no console durante desenvolvimento
if (import.meta.env.DEV) {
  ;(window as unknown as Record<string, unknown>).__engine = engine
  ;(window as unknown as Record<string, unknown>).__MIX = MIX
  ;(window as unknown as Record<string, unknown>).__Tone = Tone
}
