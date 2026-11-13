//! Erro único do crate.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum DrawError {
    /// Falha de E/S ao ler/gravar um arquivo.
    Io { path: PathBuf, source: std::io::Error },
    /// Os bytes não são uma fonte TrueType/OpenType válida.
    FontParse(String),
    /// JSON de metadados SMuFL inválido.
    Metadata(String),
    /// Dimensões de superfície inválidas (zero ou grandes demais).
    InvalidSize { width: u32, height: u32 },
    /// Falha ao codificar PNG.
    Png(String),
}

impl fmt::Display for DrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DrawError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            DrawError::FontParse(m) => write!(f, "fonte inválida: {m}"),
            DrawError::Metadata(m) => write!(f, "metadados SMuFL inválidos: {m}"),
            DrawError::InvalidSize { width, height } => {
                write!(f, "tamanho de superfície inválido: {width}×{height}")
            }
            DrawError::Png(m) => write!(f, "falha ao codificar PNG: {m}"),
        }
    }
}

impl std::error::Error for DrawError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DrawError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, DrawError>;
