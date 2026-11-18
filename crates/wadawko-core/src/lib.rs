//! # wadawko-core
//!
//! Núcleo do WaDAWko (editor/DAW de taiko), sem UI e sem áudio: porte fiel do
//! domínio do app web (`src/lib/*.ts`).
//!
//! | módulo | origem (TS) | o que tem |
//! |---|---|---|
//! | [`model`] | `types.ts` | `Song` e companhia, serde no mesmo JSON do app web (`version: 1`), builtins do kit, workspace |
//! | [`vocab`] | `vocabulary.ts` | sílabas kuchi shōga, paleta, frases do repertório |
//! | [`phrase`] | `phrase.ts` | `split_word`, `parse_words`, sfz (`!`), `resolve_cell` |
//! | [`timeline`] | `timeline.ts` | `play_order`, `main_tracks/main_mul`, `dynamics_curve`, `swing_warp` |
//! | [`ops`] | `store.svelte.ts` | operações puras sobre `&mut Song`: seções, compassos, células, linhas, pessoas, kit |
//! | [`kyai`] | `kyai.ts` (texto) | silabação, perfil de voz, contorno, eventos |
//! | [`demo`] | `demo.ts` | música "Renshū" |
//! | [`ids`] | `store.svelte.ts` | ids `prefixo-tempo36-n` |
//!
//! Regras gerais: o JSON lido e regravado é equivalente ao original (chaves
//! desconhecidas incluídas); onde o TS e este crate divergirem, o TS manda.

pub mod demo;
pub mod ids;
pub mod kyai;
pub mod model;
pub mod ops;
pub mod phrase;
pub mod timeline;
pub mod vocab;

pub use model::{
    Dynamic, DynamicMark, Hairpin, InstrumentDef, InstrumentId, InstrumentTone, KyaiVoice, Measure, Person, Section, Song, SongError, TempoMark,
    ToneType, Track, Zone, ZoneParams,
};
pub use ops::CellPos;

/// `\s` do JavaScript (≈ `char::is_whitespace`, com U+FEFF e sem U+0085).
pub(crate) fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}
