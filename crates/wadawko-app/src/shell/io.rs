//! Arquivos: exportar/importar a música (`.taiko.json`/`.wadawko.json`) e
//! exportar áudio (WAV/MP3, com progresso e cancelamento em `AppState`).

use std::path::PathBuf;

use gpui_kit::Context;
use wadawko_store::{read_song_file, write_song_file};

use super::Shell;

/// Pasta sugerida para exports: ~/Downloads (ou ~/, ou a pasta atual).
pub fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    for cand in [std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from), home.as_ref().map(|h| h.join("Downloads")), home.clone()].into_iter().flatten() {
        if cand.is_dir() {
            return cand;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Garante a extensão (o portal de arquivos devolve o que o usuário digitou).
pub fn with_ext(p: PathBuf, ext: &str) -> PathBuf {
    let has = p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext));
    if has { p } else { PathBuf::from(format!("{}.{ext}", p.display())) }
}

impl Shell {
    pub fn export_json(&mut self, cx: &mut Context<Self>) {
        let song = self.song(cx);
        let dir = default_dir();
        let name = wadawko_store::song_file_name(&song);
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            // sem portal de arquivos: salva direto na pasta sugerida
            let path = match rx.await {
                Ok(Ok(Some(p))) => with_ext(p, "json"),
                Ok(Ok(None)) => return,
                _ => dir.join(&name),
            };
            let res = write_song_file(&song, &path);
            let _ = this.update(cx, |this, cx| match res {
                Ok(()) => this.flash(format!("exportado: {}", path.display()), cx),
                Err(e) => this.flash(format!("erro ao exportar: {e:#}"), cx),
            });
        })
        .detach();
    }

    pub fn import_json(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Importar música".into()) });
        cx.spawn(async move |this, cx| {
            let paths = match rx.await {
                Ok(Ok(Some(p))) => p,
                Ok(Ok(None)) => return,
                _ => {
                    let _ = this.update(cx, |this, cx| this.flash("diálogo de arquivos indisponível (xdg-desktop-portal?)", cx));
                    return;
                }
            };
            let Some(path) = paths.into_iter().next() else { return };
            let _ = this.update(cx, |this, cx| this.import_path(path, cx));
        })
        .detach();
    }

    /// Diálogo: importar as músicas de outro banco (`.db`) ou de um backup JSON.
    pub fn import_library(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Importar músicas (banco .db ou backup .json)".into()) });
        let state = self.state.clone();
        cx.spawn(async move |this, cx| {
            let path = match rx.await {
                Ok(Ok(Some(p))) => p.into_iter().next(),
                Ok(Ok(None)) => return,
                _ => {
                    let _ = this.update(cx, |this, cx| this.flash("diálogo de arquivos indisponível (xdg-desktop-portal?)", cx));
                    return;
                }
            };
            if let Some(path) = path {
                state.update(cx, |s, cx| s.import_library(&path, cx));
            }
        })
        .detach();
    }

    /// Importa um arquivo de música como música nova e abre.
    pub fn import_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match read_song_file(&path) {
            Ok(song) => {
                let t = song.title.clone();
                self.state.update(cx, |s, cx| s.add_project(song, cx));
                self.flash(format!("importado: {t}"), cx);
            }
            Err(e) => self.flash(format!("não importei {}: {e:#}", path.display()), cx),
        }
    }

    pub fn export_audio(&mut self, ext: &'static str, cx: &mut Context<Self>) {
        if self.state.read(cx).export.is_some() {
            self.flash("já tem um export em andamento", cx);
            return;
        }
        if ext == "mp3" && !wadawko_export::ffmpeg_available() {
            self.flash("MP3 precisa do ffmpeg — instale ou use Exportar WAV", cx);
            return;
        }
        let dir = default_dir();
        let name = format!("{}.{ext}", self.song(cx).slug());
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        let state = self.state.clone();
        cx.spawn(async move |_this, cx| {
            let path = match rx.await {
                Ok(Ok(Some(p))) => with_ext(p, ext),
                Ok(Ok(None)) => return,
                _ => dir.join(&name),
            };
            state.update(cx, |s, cx| s.start_export(path, cx));
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_ext_only_appends_when_missing() {
        assert_eq!(with_ext("a/b.mp3".into(), "mp3"), PathBuf::from("a/b.mp3"));
        assert_eq!(with_ext("a/b.MP3".into(), "mp3"), PathBuf::from("a/b.MP3"));
        assert_eq!(with_ext("a/b".into(), "wav"), PathBuf::from("a/b.wav"));
        assert_eq!(with_ext("a/b.taiko".into(), "json"), PathBuf::from("a/b.taiko.json"));
    }
}
