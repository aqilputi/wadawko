//! `export` — grava uma música do banco num arquivo `.wadawko.json`.

use std::io::Write as _;
use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use wadawko_store::{find_project, song_file_name, write_song_file};

use super::Command;
use crate::args::Args;
use crate::context::Context;

pub const COMMAND: Command = Command {
    name: "export",
    usage: "export <título|id> [saída]",
    about: "exporta uma música para .wadawko.json",
    help: "Acha a música pelo id exato, pelo título exato ou por um trecho do título e\ngrava o JSON dela (indentado, o mesmo formato do export do app web).\n\n  saída omitida     ./<título>.wadawko.json\n  saída = pasta     <pasta>/<título>.wadawko.json\n  saída = -         saída padrão\n\nSó lê o banco.",
    run,
};

fn run(ctx: &Context, mut args: Args) -> Result<()> {
    let query = args.positional("o título ou id da música")?;
    let out = args.next_positional();
    args.finish()?;

    let db = ctx.open_read_only()?;
    let project = find_project(&db, &query)?;
    let song = project.song.map_err(|why| anyhow!("a música {} ({}) não pôde ser lida: {why}", project.meta.id, project.meta.title))?;

    if out.as_deref() == Some("-") {
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{}", song.to_json_pretty()).context("escrever na saída padrão")?;
        return Ok(());
    }
    let path = match out.map(PathBuf::from) {
        None => PathBuf::from(song_file_name(&song)),
        Some(p) if p.is_dir() => p.join(song_file_name(&song)),
        Some(p) => p,
    };
    write_song_file(&song, &path)?;
    println!("{} ({}) → {}", project.meta.title, project.meta.id, path.display());
    Ok(())
}
