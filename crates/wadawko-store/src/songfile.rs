//! Arquivo de música: `.wadawko.json` (aceita o `.taiko.json` do app web).
//! O conteúdo é o mesmo nos dois: a `Song` em JSON indentado com 2 espaços.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow};
use wadawko_core::Song;
use wadawko_core::model::{FILE_EXT, LEGACY_FILE_EXT};

/// Extensões aceitas na importação, para o filtro do diálogo de arquivo
/// (sem o ponto inicial).
pub const IMPORT_EXTENSIONS: [&str; 3] = ["wadawko.json", "taiko.json", "json"];

/// Nome sugerido para exportar: `<slug do título>.wadawko.json`.
pub fn song_file_name(song: &Song) -> String {
    format!("{}{}", song.slug(), FILE_EXT)
}

/// O nome termina em `.wadawko.json` ou `.taiko.json`?
pub fn is_song_file(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    name.ends_with(FILE_EXT) || name.ends_with(LEGACY_FILE_EXT)
}

/// Lê um arquivo de música (`importSong`). A extensão não é exigida: vale o
/// conteúdo (`version: 1`, com `tracks` e `measures`).
pub fn read_song_file(path: impl AsRef<Path>) -> Result<Song> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).with_context(|| format!("ler {}", path.display()))?;
    // BOM de editores do Windows
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Song::from_json(text).map_err(|e| anyhow!("{}: {e} ({})", path.display(), e.detail()))
}

/// Grava a música em `path` (`exportSong`: JSON indentado). Escreve num
/// temporário ao lado e renomeia, para nunca deixar um arquivo pela metade.
pub fn write_song_file(song: &Song, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, song.to_json_pretty()).with_context(|| format!("gravar {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("gravar {}", path.display())).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Exporta para dentro de `dir` com o nome sugerido; devolve o caminho gravado.
pub fn export_song_to_dir(song: &Song, dir: impl AsRef<Path>) -> Result<PathBuf> {
    let path = dir.as_ref().join(song_file_name(song));
    write_song_file(song, &path)?;
    Ok(path)
}
