//! Vocabulário kuchi shōga default (`src/lib/vocabulary.ts`).
//!
//! `offset` em frações da célula (0 = início, 0.5 = metade — vira semicolcheia
//! quando a célula é colcheia). Zona: `Head` = pele, `Rim` = borda (fuchi).
//! Mão: `R` = direita, `L` = esquerda (convenção comum: don/ka/tsu = direita,
//! kon/ra/ku = esquerda; sílabas compostas alternam D-E).

pub use crate::model::Zone;

/// Mão que dá o golpe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hand {
    R,
    L,
}

/// Golpe de uma sílaba do vocabulário.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// 0 ou 0.5 (fração da fatia da sílaba dentro da célula)
    pub offset: f64,
    pub zone: Zone,
    /// 0..1
    pub vel: f64,
    pub hand: Hand,
}

/// Entrada do vocabulário.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Syllable {
    /// como se escreve na célula (a caixa importa: `DON` ≠ `don`)
    pub name: &'static str,
    pub hits: &'static [Hit],
    pub hint: &'static str,
}

const fn h(offset: f64, zone: Zone, vel: f64, hand: Hand) -> Hit {
    Hit { offset, zone, vel, hand }
}

use Hand::{L, R};
use Zone::{Head, Rim};

/// As sílabas, na ordem de declaração do TS (a ordem importa pro `split_word`).
pub static VOCABULARY: &[Syllable] = &[
    Syllable { name: "DON", hits: &[h(0.0, Head, 1.0, R)], hint: "batida forte no centro (dir.)" },
    Syllable { name: "don", hits: &[h(0.0, Head, 0.7, R)], hint: "batida no centro (dir.)" },
    Syllable { name: "KON", hits: &[h(0.0, Head, 1.0, L)], hint: "batida forte no centro (esq.)" },
    Syllable { name: "kon", hits: &[h(0.0, Head, 0.7, L)], hint: "batida no centro (esq.)" },
    Syllable { name: "doko", hits: &[h(0.0, Head, 0.7, R), h(0.5, Head, 0.6, L)], hint: "duas batidas rápidas (dir.-esq.)" },
    Syllable { name: "doron", hits: &[h(0.0, Head, 1.0, R), h(0.0, Head, 1.0, L)], hint: "batida com as duas mãos ao mesmo tempo (uníssono forte)" },
    Syllable { name: "do", hits: &[h(0.0, Head, 0.55, R)], hint: "batida curta (dir.)" },
    Syllable { name: "ko", hits: &[h(0.0, Head, 0.5, L)], hint: "batida curta (esq.)" },
    Syllable { name: "tsu", hits: &[h(0.0, Head, 0.05, R)], hint: "ghost note, quase silêncio (dir.)" },
    Syllable { name: "ku", hits: &[h(0.0, Head, 0.05, L)], hint: "ghost note, quase silêncio (esq.)" },
    Syllable { name: "tsuku", hits: &[h(0.0, Head, 0.05, R), h(0.5, Head, 0.045, L)], hint: "duas ghost (dir.-esq.)" },
    Syllable { name: "KA", hits: &[h(0.0, Rim, 1.0, R)], hint: "borda forte (dir.)" },
    Syllable { name: "ka", hits: &[h(0.0, Rim, 0.7, R)], hint: "borda, fuchi (dir.)" },
    Syllable { name: "ra", hits: &[h(0.0, Rim, 0.7, L)], hint: "borda, fuchi (esq.)" },
    Syllable { name: "kara", hits: &[h(0.0, Rim, 0.65, R), h(0.5, Rim, 0.55, L)], hint: "duas na borda (dir.-esq.)" },
    Syllable { name: "karan", hits: &[h(0.0, Rim, 1.0, R), h(0.0, Rim, 1.0, L)], hint: "borda com as duas mãos ao mesmo tempo (uníssono forte)" },
    Syllable { name: "ten", hits: &[h(0.0, Head, 0.7, R)], hint: "shime, batida (dir.)" },
    Syllable { name: "teke", hits: &[h(0.0, Head, 0.6, R), h(0.5, Head, 0.5, L)], hint: "shime, duas rápidas (dir.-esq.)" },
    Syllable { name: "su", hits: &[], hint: "pausa marcada" },
];

/// Busca exata (sensível à caixa) no vocabulário.
pub fn syllable(name: &str) -> Option<&'static Syllable> {
    VOCABULARY.iter().find(|s| s.name == name)
}

/// Grupo da paleta (por zona do tambor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaletteGroup {
    pub label: &'static str,
    pub syls: &'static [&'static str],
}

/// Paleta agrupada por zona do tambor.
pub static PALETTE_GROUPS: &[PaletteGroup] = &[
    PaletteGroup { label: "couro", syls: &["DON", "don", "KON", "kon", "doron", "do", "ko", "doko", "tsu", "ku", "tsuku", "ten", "teke"] },
    PaletteGroup { label: "fuchi", syls: &["KA", "ka", "ra", "kara", "karan"] },
    PaletteGroup { label: "pausa", syls: &["su"] },
];

/// Frase conhecida do repertório taiko; inserir escreve a partir da célula
/// focada. Default: 1 palavra POR TEMPO, sobrescrevendo o span inteiro
/// (`ops::cells::fill_phrase_beats`). `per_cell` = 1 palavra por célula
/// (`ops::cells::fill_sequential`; frases com ritmo em meio-tempo).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnownPhrase {
    pub name: &'static str,
    pub phrase: &'static str,
    pub hint: &'static str,
    pub per_cell: bool,
}

pub static PHRASES: &[KnownPhrase] = &[
    KnownPhrase { name: "cavalinho", phrase: "don doko don doko don doko don doko", hint: "galope: don no tempo, doko no contratempo (1 compasso)", per_cell: true },
    KnownPhrase { name: "dondoko", phrase: "don doko don don", hint: "padrão dondoko clássico (1 compasso)", per_cell: false },
    KnownPhrase { name: "jiuchi dontsuku", phrase: "DON tsuku DON tsuku", hint: "jiuchi de festival, matsuri-jiuchi (1 compasso)", per_cell: false },
    KnownPhrase { name: "tontsuton", phrase: "don su tsu kon su ku don su tsu kon su ku don su don su", hint: "jiuchi tontsuton (2 compassos, meio-tempo por palavra)", per_cell: true },
    KnownPhrase { name: "matsuri", phrase: "DON su KON su DON KARA KA RA don KON su kon DON KARA KA RA", hint: "tema do Matsuri (2 compassos, meio-tempo por palavra)", per_cell: true },
    KnownPhrase { name: "jiuchi shime", phrase: "ten teke ten teke", hint: "jiuchi básico do shime (1 compasso)", per_cell: false },
    KnownPhrase { name: "rulo dokodoko", phrase: "dokodoko dokodoko dokodoko dokodoko", hint: "rulo contínuo (1 compasso, 4 golpes por tempo)", per_cell: false },
];

/// Texto a mostrar na célula: a pausa `su` vira `·`.
pub fn display_syllable(syl: &str) -> &str {
    if syl == "su" { "·" } else { syl }
}
