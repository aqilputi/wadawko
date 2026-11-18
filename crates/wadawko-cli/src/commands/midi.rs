//! `midi` — exporta uma música do banco para MIDI (`.mid`).

use std::path::PathBuf;

use anyhow::{Context as _, Result, anyhow};
use wadawko_samples::SampleBank;
use wadawko_store::find_project;

use super::Command;
use crate::args::Args;
use crate::context::Context;

pub const COMMAND: Command = Command {
    name: "midi",
    usage: "midi <título|id> [saída] [--samples-dir <pasta>]",
    about: "exporta uma música para MIDI (.mid)",
    help: "Standard MIDI File tipo 1: faixa 0 com fórmulas de compasso, andamento (rall./accel.\nem degraus por tempo) e marcadores de seção; uma faixa por linha no canal 10\n(bateria GM; nota pelo instrumento e zona, velocity pela força do golpe).\nRepetições, swing, dinâmicas e mudo/solo como no áudio. Kiai não entra.\n\n  saída omitida     ./<título>.mid\n  saída = pasta     <pasta>/<título>.mid\n\nSó lê o banco.",
    run,
};

fn run(ctx: &Context, mut args: Args) -> Result<()> {
    let query = args.positional("o título ou id da música")?;
    let out = args.next_positional();
    let samples_dir = args.option("--samples-dir")?.map(PathBuf::from);
    args.finish()?;

    let db = ctx.open_read_only()?;
    let project = find_project(&db, &query)?;
    let song = project.song.map_err(|why| anyhow!("a música {} ({}) não pôde ser lida: {why}", project.meta.id, project.meta.title))?;
    let bank = SampleBank::load_from(samples_dir.as_deref()).context("carregar samples")?;
    let name = format!("{}.mid", song.slug());
    let path = match out.map(PathBuf::from) {
        None => PathBuf::from(&name),
        Some(p) if p.is_dir() => p.join(&name),
        Some(p) => p,
    };
    let bytes = wadawko_export::midi::song_to_midi(&song, &bank);
    std::fs::write(&path, &bytes).with_context(|| format!("gravar {}", path.display()))?;
    println!("{} ({}) → {} ({} bytes)", project.meta.title, project.meta.id, path.display(), bytes.len());
    Ok(())
}
