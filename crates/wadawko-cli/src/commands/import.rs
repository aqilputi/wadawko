//! `import` — acrescenta ao banco a música de um arquivo `.wadawko.json`/`.taiko.json`.

use anyhow::Result;
use wadawko_store::{add_song, display_title, read_song_file};

use super::Command;
use crate::args::Args;
use crate::context::Context;

pub const COMMAND: Command = Command {
    name: "import",
    usage: "import <arquivo> [--open]",
    about: "importa um arquivo de música como música nova",
    help: "Lê um arquivo .wadawko.json (ou .taiko.json do app web) e o acrescenta ao banco\ncomo uma música nova, no fim da lista. Nenhuma música existente é alterada.\n\n  --open   faz dela a música aberta (currentId)\n\nCria o banco se ele não existir.",
    run,
};

fn run(ctx: &Context, mut args: Args) -> Result<()> {
    let open = args.flag("--open");
    let file = args.positional("o arquivo")?;
    args.finish()?;

    // valida o arquivo antes de abrir (e talvez criar) o banco
    let song = read_song_file(&file)?;
    let db = ctx.open_read_write()?;
    let id = add_song(&db, &song, open)?;
    println!("importada: {} ({id}) — {} linha(s), {} compasso(s) → {}", display_title(&song), song.tracks.len(), song.measures.len(), ctx.db_path().display());
    Ok(())
}
