export type InstrumentId = 'shime' | 'chu' | 'hira' | 'nagado' | 'onagado' | 'okedo' | 'odaiko' | 'kane'

/** Pessoa que toca um ou mais instrumentos (agrupamento de tracks). */
export interface Person {
  id: string
  name: string
  /** cor do grupo (hex); default = ciclo de PERSON_COLORS ao criar */
  color?: string
  /** mudo do submix da pessoa */
  muted?: boolean
  /** solo do submix da pessoa */
  solo?: boolean
  /** volume do barramento da pessoa em dB (default 0) */
  volume?: number
}

/** filtro de timbre aplicado sobre o sample base */
export interface InstrumentTone {
  type: 'lowpass' | 'highpass' | 'bandpass'
  /** frequência de corte em Hz */
  cutoff: number
  /** ressonância base do filtro */
  q: number
}

/** ajustes de som de uma zona (couro ou borda) */
export interface ZoneParams {
  /** transposição em semitons (0 = base) → playbackRate = 2^(pitch/12) */
  pitch?: number
  /** trim de volume em dB (0 = base) */
  gain?: number
  /** filtro de timbre (ausente = sem filtro) */
  tone?: InstrumentTone
  /** 0..1 → realce de ressonância/overtones (soma no Q do filtro) */
  overtones?: number
  /** 0..1 → encurta (<0.5) ou alonga (>0.5) o sample; 0.5 = natural */
  decay?: number
}

/**
 * Definição de instrumento: um sample base + ajustes de som SEPARADOS por zona.
 * `head` = couro (pele), `rim` = borda (fuchi). Cada zona tem seus próprios params.
 */
export interface InstrumentDef {
  id: string
  name: string
  /** voz/sample base */
  base: InstrumentId
  /** def padrão (constante no código; não serializada) */
  builtin?: boolean
  /** ajustes do couro (pele) */
  head?: ZoneParams
  /** ajustes da borda (fuchi/rim) */
  rim?: ZoneParams
}

export interface Track {
  id: string
  name: string
  /** instrumento base / fallback se a def custom for deletada */
  instrument: InstrumentId
  /** ref à InstrumentDef (builtin id ou custom id) */
  instrumentId?: string
  muted: boolean
  solo: boolean
  /** volume do canal em dB (default -6) */
  volume?: number
  /** panorama -1 (esq) .. 1 (dir), default 0 */
  pan?: number
  /** pessoas que tocam este instrumento; ausente/[] = sem pessoa. ≥2 = compartilhado */
  personIds?: string[]
  /** voz dos kiai desta linha (soma à voz própria de cada pessoa) */
  kyai?: KyaiVoice
}

/** ajustes da voz de kiai de uma linha */
export interface KyaiVoice {
  /** transposição em semitons (soma ao tom da pessoa) */
  pitch?: number
  /** 0..1 — quanto cada grito varia (tom, contorno, tempo, duração) */
  vary?: number
  /** volume dos gritos em dB (default 0) */
  gain?: number
}

/** cells[trackId] tem beats*subdivision entradas; null = silêncio */
export interface Measure {
  beats: number
  subdivision: number
  cells: Record<string, (string | null)[]>
  /** paralelo a cells; personId dono de cada célula. Só usado em track compartilhada. */
  cellOwners?: Record<string, (string | null)[]>
  /** marcação de dinâmica no início do compasso; vale até a próxima */
  dynamic?: Dynamic
  /** crescendo/diminuendo ao longo do compasso (encadeia até a próxima marcação) */
  hairpin?: 'cresc' | 'dim'
  /** dinâmica por instrumento (trackId): tem prioridade sobre a geral neste compasso */
  trackDynamics?: Record<string, DynamicMark>
  /** kiai/kakegoe por linha, paralelo a cells: UMA sílaba por célula ("so", "re!") */
  kyai?: Record<string, (string | null)[]>
}

export interface DynamicMark {
  dynamic?: Dynamic
  hairpin?: 'cresc' | 'dim'
}

export type Dynamic = 'pp' | 'p' | 'mp' | 'mf' | 'f' | 'ff'

/**
 * multiplicador de ganho por dinâmica; mf = 1 (som atual/neutro). Degraus de
 * ~3–6 dB (pp −12, p −6, mp −3, f +6, ff +10), perto da faixa de um taiko real;
 * o limiter no master segura os picos de f/ff.
 */
export const DYNAMIC_LEVELS: Record<Dynamic, number> = {
  pp: 0.25,
  p: 0.5,
  mp: 0.71,
  mf: 1,
  f: 2,
  ff: 3.2,
}

/** divisão nomeada da música: começa no compasso `start`, vai até a próxima */
export interface Section {
  id: string
  name: string
  start: number
  /** quantas vezes a seção toca (ritornello); default 1 */
  repeat?: number
}

export interface Song {
  version: 1
  title: string
  tempo: number
  /** 0 = reto; 1 = swing total (colcheia atrasada pro terço, estilo matsuri/hane) */
  swing?: number
  tracks: Track[]
  measures: Measure[]
  sections?: Section[]
  /** pessoas; a ordem define a ordem dos trilhos de agrupamento */
  people?: Person[]
  /** instrumentos custom desta música (builtins ficam em BUILTIN_INSTRUMENTS) */
  instruments?: InstrumentDef[]
  /** "main": instrumentos em destaque (+6 dB, como f); o resto recua (−6 dB, como p) */
  mainTrackIds?: string[]
  /** formato antigo (um só instrumento em destaque); lido como [mainTrackId] */
  mainTrackId?: string
}

/** paleta de cores das pessoas — ciclada ao criar */
export const PERSON_COLORS = [
  '#e8663d', // laranja-terracota
  '#3d7ee8', // azul
  '#2fae6b', // verde
  '#b154d6', // roxo
  '#d6a319', // âmbar
  '#d64577', // rosa
  '#1ba7a7', // teal
  '#8a6d3b', // marrom
]

export const INSTRUMENTS: Record<InstrumentId, string> = {
  shime: 'Shime-daiko',
  chu: 'Chū-daiko',
  hira: 'Hira-daiko pequeno',
  nagado: 'Nagadō-daiko',
  onagado: 'Ō-nagadō',
  okedo: 'Okedō-daiko',
  odaiko: 'Ōdaiko',
  kane: 'Kane',
}

/**
 * Defs padrão sempre disponíveis. As 7 base (id = id do sample) + presets bem
 * distintos (agudo, grave, anelado…) pra começar o kit com variedade.
 */
const BASE_INSTRUMENTS: InstrumentDef[] = (Object.keys(INSTRUMENTS) as InstrumentId[]).map((base) => ({
  id: base,
  name: INSTRUMENTS[base],
  base,
  builtin: true,
}))

const PRESET_INSTRUMENTS: InstrumentDef[] = [
  {
    id: 'preset-shime-agudo',
    name: 'Shime agudo',
    base: 'shime',
    builtin: true,
    head: { pitch: 7, tone: { type: 'highpass', cutoff: 900, q: 1 }, decay: 0.4 },
    rim: { pitch: 7, gain: -2 },
  },
  {
    id: 'preset-chu-grave',
    name: 'Chū grave',
    base: 'chu',
    builtin: true,
    head: { pitch: -5, decay: 0.7, gain: 1 },
    rim: { pitch: -3 },
  },
  {
    id: 'preset-odaiko-profundo',
    name: 'Ōdaiko profundo',
    base: 'odaiko',
    builtin: true,
    head: { pitch: -7, decay: 0.85, tone: { type: 'lowpass', cutoff: 600, q: 1 }, gain: 2 },
    rim: { pitch: -4, gain: -2 },
  },
  {
    id: 'preset-nagado-anelado',
    name: 'Nagadō anelado',
    base: 'nagado',
    builtin: true,
    head: { overtones: 0.4, tone: { type: 'bandpass', cutoff: 320, q: 1.2 }, gain: 8 },
    rim: { overtones: 0.5, tone: { type: 'bandpass', cutoff: 2200, q: 2 } },
  },
  {
    id: 'preset-okedo-brilhante',
    name: 'Okedō brilhante',
    base: 'okedo',
    builtin: true,
    head: { pitch: 3, tone: { type: 'highpass', cutoff: 380, q: 1.2 }, decay: 0.45, gain: 3 },
    rim: { pitch: 5, overtones: 0.5 },
  },
]

export const BUILTIN_INSTRUMENTS: InstrumentDef[] = [...BASE_INSTRUMENTS, ...PRESET_INSTRUMENTS]

export function cellsPerMeasure(m: Measure): number {
  return m.beats * m.subdivision
}
