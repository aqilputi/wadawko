/**
 * Vocabulário kuchi shōga default do protótipo.
 * offset em frações da célula (0 = início, 0.5 = metade — vira semicolcheia
 * quando a célula é colcheia). zone: head = pele, rim = borda (fuchi).
 * hand: R = direita, L = esquerda (convenção comum: don/ka/tsu = direita,
 * kon/ra/ku = esquerda; sílabas compostas alternam D-E).
 */
export type Hand = 'R' | 'L'

export interface Hit {
  offset: 0 | 0.5
  zone: 'head' | 'rim'
  vel: number
  hand: Hand
}

export interface Syllable {
  hits: Hit[]
  hint: string
}

export const VOCABULARY: Record<string, Syllable> = {
  DON: { hits: [{ offset: 0, zone: 'head', vel: 1.0, hand: 'R' }], hint: 'batida forte no centro (dir.)' },
  don: { hits: [{ offset: 0, zone: 'head', vel: 0.7, hand: 'R' }], hint: 'batida no centro (dir.)' },
  KON: { hits: [{ offset: 0, zone: 'head', vel: 1.0, hand: 'L' }], hint: 'batida forte no centro (esq.)' },
  kon: { hits: [{ offset: 0, zone: 'head', vel: 0.7, hand: 'L' }], hint: 'batida no centro (esq.)' },
  doko: {
    hits: [
      { offset: 0, zone: 'head', vel: 0.7, hand: 'R' },
      { offset: 0.5, zone: 'head', vel: 0.6, hand: 'L' },
    ],
    hint: 'duas batidas rápidas (dir.-esq.)',
  },
  doron: {
    hits: [
      { offset: 0, zone: 'head', vel: 1.0, hand: 'R' },
      { offset: 0, zone: 'head', vel: 1.0, hand: 'L' },
    ],
    hint: 'batida com as duas mãos ao mesmo tempo (uníssono forte)',
  },
  do: { hits: [{ offset: 0, zone: 'head', vel: 0.55, hand: 'R' }], hint: 'batida curta (dir.)' },
  ko: { hits: [{ offset: 0, zone: 'head', vel: 0.5, hand: 'L' }], hint: 'batida curta (esq.)' },
  tsu: { hits: [{ offset: 0, zone: 'head', vel: 0.05, hand: 'R' }], hint: 'ghost note, quase silêncio (dir.)' },
  ku: { hits: [{ offset: 0, zone: 'head', vel: 0.05, hand: 'L' }], hint: 'ghost note, quase silêncio (esq.)' },
  tsuku: {
    hits: [
      { offset: 0, zone: 'head', vel: 0.05, hand: 'R' },
      { offset: 0.5, zone: 'head', vel: 0.045, hand: 'L' },
    ],
    hint: 'duas ghost (dir.-esq.)',
  },
  KA: { hits: [{ offset: 0, zone: 'rim', vel: 1.0, hand: 'R' }], hint: 'borda forte (dir.)' },
  ka: { hits: [{ offset: 0, zone: 'rim', vel: 0.7, hand: 'R' }], hint: 'borda, fuchi (dir.)' },
  ra: { hits: [{ offset: 0, zone: 'rim', vel: 0.7, hand: 'L' }], hint: 'borda, fuchi (esq.)' },
  kara: {
    hits: [
      { offset: 0, zone: 'rim', vel: 0.65, hand: 'R' },
      { offset: 0.5, zone: 'rim', vel: 0.55, hand: 'L' },
    ],
    hint: 'duas na borda (dir.-esq.)',
  },
  karan: {
    hits: [
      { offset: 0, zone: 'rim', vel: 1.0, hand: 'R' },
      { offset: 0, zone: 'rim', vel: 1.0, hand: 'L' },
    ],
    hint: 'borda com as duas mãos ao mesmo tempo (uníssono forte)',
  },
  ten: { hits: [{ offset: 0, zone: 'head', vel: 0.7, hand: 'R' }], hint: 'shime, batida (dir.)' },
  teke: {
    hits: [
      { offset: 0, zone: 'head', vel: 0.6, hand: 'R' },
      { offset: 0.5, zone: 'head', vel: 0.5, hand: 'L' },
    ],
    hint: 'shime, duas rápidas (dir.-esq.)',
  },
  su: { hits: [], hint: 'pausa marcada' },
}

/** paleta agrupada por zona do tambor */
export const PALETTE_GROUPS: { label: string; syls: string[] }[] = [
  { label: 'couro', syls: ['DON', 'don', 'KON', 'kon', 'doron', 'do', 'ko', 'doko', 'tsu', 'ku', 'tsuku', 'ten', 'teke'] },
  { label: 'fuchi', syls: ['KA', 'ka', 'ra', 'kara', 'karan'] },
  { label: 'pausa', syls: ['su'] },
]

/**
 * Frases conhecidas do repertório taiko; clicar insere a partir da célula
 * focada. Default: 1 palavra POR TEMPO, sobrescrevendo o span inteiro.
 * perCell: true = 1 palavra por célula (frases com ritmo em meio-tempo).
 */
export interface KnownPhrase {
  name: string
  phrase: string
  hint: string
  perCell?: boolean
}

export const PHRASES: KnownPhrase[] = [
  {
    name: 'cavalinho',
    phrase: 'don doko don doko don doko don doko',
    hint: 'galope: don no tempo, doko no contratempo (1 compasso)',
    perCell: true,
  },
  { name: 'dondoko', phrase: 'don doko don don', hint: 'padrão dondoko clássico (1 compasso)' },
  { name: 'jiuchi dontsuku', phrase: 'DON tsuku DON tsuku', hint: 'jiuchi de festival, matsuri-jiuchi (1 compasso)' },
  {
    name: 'tontsuton',
    phrase: 'don su tsu kon su ku don su tsu kon su ku don su don su',
    hint: 'jiuchi tontsuton (2 compassos, meio-tempo por palavra)',
    perCell: true,
  },
  {
    name: 'matsuri',
    phrase: 'DON su KON su DON KARA KA RA don KON su kon DON KARA KA RA',
    hint: 'tema do Matsuri (2 compassos, meio-tempo por palavra)',
    perCell: true,
  },
  { name: 'jiuchi shime', phrase: 'ten teke ten teke', hint: 'jiuchi básico do shime (1 compasso)' },
  { name: 'rulo dokodoko', phrase: 'dokodoko dokodoko dokodoko dokodoko', hint: 'rulo contínuo (1 compasso, 4 golpes por tempo)' },
]

export function displaySyllable(syl: string): string {
  return syl === 'su' ? '·' : syl
}
