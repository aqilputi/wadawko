//! wadawko-samples — banco de samples do WaDAWko (docs/planejamento/gpui.md §3.4).
//!
//! * [`manifest`]: tabelas estáticas portadas de `src/lib/audio.ts`
//!   (`SAMPLE_FILES`, `TUNING`, `TAIL`, `EQ`) e a regra de seleção de camada.
//! * [`SampleBank`]: os `.ogg` de `assets/samples/` decodificados (symphonia)
//!   para `Arc<SampleData>` estéreo intercalado, com lookup por
//!   `(instrumento, zona, camada, round robin)`.
//! * [`CREDITS`]: texto das licenças (CC BY-SA etc.) para a UI mostrar.
//!
//! Feature `embed` (padrão): embute os `.ogg` no binário. Sem ela, carregue de
//! um diretório com [`SampleBank::load_dir`] (ex.: [`repo_samples_dir`]).

mod bank;
mod decode;
mod embedded;
pub mod manifest;

pub use bank::{MAX_LAYERS, Pick, RoundRobin, SampleBank, SampleId, SampleSource};
pub use decode::{SampleData, decode_ogg};
pub use embedded::{CREDITS, SAMPLE_NAMES};
pub use manifest::{
    ANIMALESE, BaseInstrument, EqBand, FilterKind, InstrumentManifest, Layers, MANIFEST, Tail, Tuning, UnknownInstrument, Zone, animalese, layer_max_vel, layer_velocity, select_layer,
};

/// `assets/samples/` do checkout de onde o crate foi compilado — útil em
/// desenvolvimento e nos testes sem `embed`. Não existe em binários distribuídos.
pub fn repo_samples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/samples")
}
