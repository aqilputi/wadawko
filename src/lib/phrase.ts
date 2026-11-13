/**
 * Quebra de palavras kuchi shōga em sílabas do vocabulário.
 * "dokodoko" → doko+doko; alongamento falado é absorvido ("doooko" casa doko).
 */
import { VOCABULARY } from './vocabulary'

const KEYS = Object.keys(VOCABULARY).sort((a, b) => b.length - a.length)

export interface WordSyllable {
  syl: string
  /** peso de duração: 1 normal; letras alongadas (doooko, DOOON) somam +1 cada */
  stretch: number
  /** escrito em CAIXA ALTA sem variante maiúscula no vocabulário = acento genérico */
  accent: boolean
}

/**
 * Tenta casar uma sílaba do vocabulário a partir de `pos`, absorvendo letras
 * repetidas (alongamento falado: "dooon" casa com "don", stretch 3).
 */
function matchAt(word: string, pos: number, key: string): { consumed: number; stretch: number } | null {
  let wi = pos
  let extra = 0
  for (let ki = 0; ki < key.length; ki++) {
    const cur = key[ki].toLowerCase()
    if (word[wi]?.toLowerCase() !== cur) return null
    wi++
    const next = key[ki + 1]?.toLowerCase()
    while (wi < word.length && word[wi].toLowerCase() === cur && cur !== next) {
      wi++
      extra++
    }
  }
  return { consumed: wi - pos, stretch: 1 + extra }
}

/**
 * Quebra uma palavra em sílabas do vocabulário. Tenta o maior consumo primeiro
 * mas com backtracking: "dokon" falha como doko+n e recua para do+kon.
 */
export function splitWord(word: string): WordSyllable[] | null {
  const memo = new Map<number, WordSyllable[] | null>()

  const go = (pos: number): WordSyllable[] | null => {
    if (pos >= word.length) return []
    const cached = memo.get(pos)
    if (cached !== undefined) return cached

    // candidatos dedupe por (consumo, sílaba minúscula), variante pela caixa escrita
    const byShape = new Map<string, { key: string; consumed: number; stretch: number; accent: boolean }>()
    for (const key of KEYS) {
      const m = matchAt(word, pos, key)
      if (!m) continue
      const frag = word.slice(pos, pos + m.consumed)
      const wantUpper = frag === frag.toUpperCase()
      const shape = `${m.consumed}:${key.toLowerCase()}`
      const existing = byShape.get(shape)
      const matchesCase = (key === key.toUpperCase()) === wantUpper
      if (!existing || matchesCase) {
        const accent = wantUpper && key !== key.toUpperCase()
        byShape.set(shape, { key, consumed: m.consumed, stretch: m.stretch, accent })
      }
    }
    const candidates = [...byShape.values()].sort((a, b) => b.consumed - a.consumed)

    let result: WordSyllable[] | null = null
    for (const cand of candidates) {
      const rest = go(pos + cand.consumed)
      if (rest) {
        result = [{ syl: cand.key, stretch: cand.stretch, accent: cand.accent }, ...rest]
        break
      }
    }
    memo.set(pos, result)
    return result
  }

  return go(0)
}

/**
 * Texto livre → lista de palavras válidas (cada palavra = UMA célula; palavra
 * composta como "dokodoko" mantém todos os golpes dentro da própria célula).
 */
export function parseWords(text: string): string[] | null {
  const words = text.trim().split(/[\s,;|]+/).filter(Boolean)
  for (const word of words) {
    if (!splitWord(stripSfz(word))) return null
  }
  return words
}

/** sufixo de sforzando: "don!" = don com sfz (acento súbito no 1º golpe) */
export const SFZ_MARK = '!'

export function isSfz(value: string | null | undefined): boolean {
  return !!value && value.length > 1 && value.endsWith(SFZ_MARK)
}

/** valor sem a marca de sfz */
export function stripSfz(value: string): string {
  return isSfz(value) ? value.slice(0, -1) : value
}

/** liga/desliga o sfz de um valor de célula (null/vazio fica como está) */
export function toggleSfz(value: string | null): string | null {
  if (!value) return value
  return isSfz(value) ? stripSfz(value) : value + SFZ_MARK
}

export interface ResolvedHit {
  /** posição do golpe como fração da célula [0, 1) */
  offset: number
  zone: 'head' | 'rim'
  vel: number
  hand: 'R' | 'L'
  accent: boolean
  /** sforzando: golpe forçado acima da dinâmica do trecho */
  sfz?: boolean
}

export interface ResolvedPart {
  syl: string
  hits: ResolvedHit[]
}

export interface CellResolution {
  hits: ResolvedHit[]
  parts: ResolvedPart[]
  /** a célula tem sfz (marca "!" no fim) */
  sfz: boolean
}

const resolveMemo = new Map<string, CellResolution | null>()

/**
 * Resolve o valor de uma célula (sílaba simples ou palavra composta) nos golpes
 * que soam dentro dela. Sílabas dividem a célula por peso de alongamento;
 * os offsets 0/0.5 de cada sílaba caem dentro da fatia dela.
 */
export function resolveCell(value: string): CellResolution | null {
  const cached = resolveMemo.get(value)
  if (cached !== undefined) return cached
  const sfz = isSfz(value)
  const parts = splitWord(stripSfz(value))
  let result: CellResolution | null = null
  if (parts) {
    const total = parts.reduce((sum, p) => sum + p.stretch, 0)
    let cum = 0
    const hits: ResolvedHit[] = []
    const outParts: ResolvedPart[] = []
    for (const part of parts) {
      const entry = VOCABULARY[part.syl]
      const start = cum / total
      const width = part.stretch / total
      const partHits: ResolvedHit[] = entry.hits.map((h) => ({
        offset: start + h.offset * width,
        zone: h.zone,
        vel: part.accent ? Math.min(1, h.vel * 1.5) : h.vel,
        hand: h.hand,
        accent: part.accent || h.vel >= 1,
      }))
      hits.push(...partHits)
      outParts.push({ syl: part.syl, hits: partHits })
      cum += part.stretch
    }
    // sfz: o 1º golpe da nota sai na força máxima (o resto da nota segue normal)
    if (sfz && hits.length) hits[0] = { ...hits[0], vel: 1, accent: true, sfz: true }
    result = { hits, parts: outParts, sfz }
  }
  resolveMemo.set(value, result)
  return result
}
