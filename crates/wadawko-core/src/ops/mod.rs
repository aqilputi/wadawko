//! Operações de edição da música: porte das funções de `src/lib/store.svelte.ts`
//! como funções PURAS sobre `&mut Song` (sem estado de UI, sem persistência).
//!
//! O que no store vinha do estado global entra como parâmetro:
//! - `pen` (`store.activePersonId`): a "caneta" que carimba o dono das notas
//!   escritas em linha compartilhada;
//! - seção alvo de `add_measure/remove_measure`: use
//!   `sections::target_section(song, &selecionadas)` quando o usuário não
//!   indicar uma seção explicitamente;
//! - efeitos de UI (limpar seleção, zerar `startCell`, trocar a caneta ao
//!   apagar a pessoa) ficam por conta de quem chama — estão anotados em cada função.
//!
//! Índices fora da música são ignorados em silêncio (como o `?.` do TS).

pub mod cells;
pub mod kit;
pub mod measures;
pub mod people;
pub mod practice;
pub mod sections;
pub mod tracks;

/// Posição de uma célula: compasso `mi`, célula `ci` dentro dele.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellPos {
    pub mi: usize,
    pub ci: usize,
}

impl CellPos {
    pub fn new(mi: usize, ci: usize) -> Self {
        CellPos { mi, ci }
    }
}
