//! `render` — renderiza uma música (do banco ou de um arquivo) em WAV/MP3.

use std::io::{IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context as _, Result, anyhow};
use wadawko_audio::{MasterMode, Region, SampleSet};
use wadawko_core::Song;
use wadawko_export::{AudioFormat, BitDepth, ExportOptions, ExportStage, export_song};
use wadawko_samples::SampleBank;
use wadawko_store::{find_project, read_song_file};

use super::Command;
use crate::args::{Args, usage};
use crate::context::Context;

pub const COMMAND: Command = Command {
    name: "render",
    usage: "render <título|id|arquivo> <saída.wav|mp3> [opções]",
    about: "renderiza uma música em WAV ou MP3",
    help: "Renderiza a música offline, com o mesmo engine do playback, e grava o áudio.
A música vem do banco (id exato, título exato ou trecho do título, como no
export) ou, se o argumento for um arquivo .wadawko.json/.taiko.json, do arquivo
(aí o banco nem é aberto). O formato sai da extensão: .wav ou .mp3 (192 kbps,
precisa do ffmpeg). Um arquivo de saída que já existe é substituído.

  --sr HZ            taxa de amostragem (padrão 48000)
  --bits 16|24       profundidade do WAV (padrão 16)
  --region A..B      só os compassos [A, B) (base 0)
  --seed N           semente da humanização (padrão 1; mesma semente = mesmo arquivo)
  --metronome        inclui o metrônomo
  --tail S           cauda depois do fim, em segundos (padrão 3)
  --live-master      cadeia do playback (compressor + soft clip) em vez do export limpo
  --no-limiter       sem o limiter com lookahead (teto −1,5 dBFS)
  --samples-dir DIR  samples de um diretório em vez dos embutidos
  --quiet            não mostra o progresso

O progresso vai para a saída de erro; o resumo, para a saída padrão.
Só lê o banco.",
    run,
};

/// Opção numérica: erro de uso se o valor não servir.
fn parsed<T: FromStr>(args: &mut Args, name: &str) -> Result<Option<T>> {
    match args.option(name)? {
        None => Ok(None),
        Some(v) => v.parse().map(Some).map_err(|_| usage(format!("{name}: valor inválido: {v}"))),
    }
}

fn parse_region(v: &str) -> Result<Region> {
    let bad = || usage(format!("--region espera A..B com A < B (compassos, base 0): {v}"));
    let (a, b) = v.split_once("..").ok_or_else(bad)?;
    let region = Region { start_bar: a.trim().parse().map_err(|_| bad())?, end_bar: b.trim().parse().map_err(|_| bad())? };
    if region.start_bar >= region.end_bar {
        return Err(bad());
    }
    Ok(region)
}

/// Um caminho de arquivo de música (e não um título/id do banco)?
fn is_song_file(query: &str) -> bool {
    let lower = query.to_lowercase();
    Path::new(query).is_file() || lower.ends_with(".wadawko.json") || lower.ends_with(".taiko.json")
}

fn run(ctx: &Context, mut args: Args) -> Result<()> {
    let mut opts = ExportOptions::default();
    if let Some(sr) = parsed::<u32>(&mut args, "--sr")? {
        if !(8_000..=192_000).contains(&sr) {
            return Err(usage(format!("--sr: use de 8000 a 192000 Hz: {sr}")));
        }
        opts.render.sample_rate = sr;
    }
    if let Some(seed) = parsed(&mut args, "--seed")? {
        opts.render.seed = seed;
    }
    if let Some(tail) = parsed::<f64>(&mut args, "--tail")? {
        if !tail.is_finite() || !(0.0..=600.0).contains(&tail) {
            return Err(usage(format!("--tail: use de 0 a 600 segundos: {tail}")));
        }
        opts.render.tail_secs = tail;
    }
    if let Some(v) = args.option("--region")? {
        opts.render.region = Some(parse_region(&v)?);
    }
    let bits = args.option("--bits")?;
    let samples_dir = args.option("--samples-dir")?.map(PathBuf::from);
    opts.render.metronome = args.flag("--metronome");
    if args.flag("--live-master") {
        opts.render.master = MasterMode::Live;
    }
    if args.flag("--no-limiter") {
        opts.render.limiter_ceiling_db = None;
    }
    let quiet = args.flag("--quiet");
    let query = args.positional("o título, id ou arquivo da música")?;
    let output = PathBuf::from(args.positional("o arquivo de saída (.wav ou .mp3)")?);
    args.finish()?;

    let mut format = AudioFormat::from_path(&output).map_err(|e| usage(format!("saída {}: {e}", output.display())))?;
    match (bits.as_deref(), &mut format) {
        (None | Some("16"), _) => {}
        (Some("24"), AudioFormat::Wav(depth)) => *depth = BitDepth::Pcm24,
        (Some("24"), _) => return Err(usage("--bits 24 só vale para .wav")),
        (Some(other), _) => return Err(usage(format!("--bits {other}: use 16 ou 24"))),
    }
    opts.format = Some(format);

    let (song, origin): (Song, String) = if is_song_file(&query) {
        (read_song_file(&query)?, query.clone())
    } else {
        let db = ctx.open_read_only()?;
        let project = find_project(&db, &query)?;
        let song = project.song.map_err(|why| anyhow!("a música {} ({}) não pôde ser lida: {why}", project.meta.id, project.meta.title))?;
        (song, format!("{} ({})", project.meta.title, project.meta.id))
    };
    if let Some(r) = opts.render.region
        && r.end_bar > song.measures.len()
    {
        anyhow::bail!("--region {}..{}: a música só tem {} compasso(s)", r.start_bar, r.end_bar, song.measures.len());
    }

    let t0 = Instant::now();
    let bank = SampleBank::load_from(samples_dir.as_deref()).context("carregar samples")?;
    let samples = SampleSet::from_bank(&bank);
    if !quiet {
        eprintln!("{origin}: {} BPM, {} linha(s), {} compasso(s)", song.tempo, song.tracks.len(), song.measures.len());
    }
    let t1 = Instant::now();
    // num terminal a linha se reescreve; redirecionado, uma linha a cada 10 %
    let tty = std::io::stderr().is_terminal();
    let mut last = (ExportStage::Rendering, -1i32);
    let result = export_song(
        &song,
        &bank,
        &samples,
        &output,
        &opts,
        &mut |stage, p| {
            let step = (p * 10.0) as i32;
            if quiet || stage == ExportStage::Done || (stage, step) == last {
                return;
            }
            last = (stage, step);
            let mut err = std::io::stderr().lock();
            let _ = if tty { write!(err, "\r{} {:3.0} %   ", stage.label(), p * 100.0) } else { writeln!(err, "{} {:3.0} %", stage.label(), p * 100.0) };
        },
        &AtomicBool::new(false),
    );
    if !quiet && tty {
        eprintln!();
    }
    let summary = result?;

    let db = |x: f32| if x > 0.0 { 20.0 * x.log10() } else { f32::NEG_INFINITY };
    let bytes = std::fs::metadata(&summary.path).map(|m| m.len()).unwrap_or(0);
    let kind = match summary.format {
        AudioFormat::Wav(depth) => format!("WAV {}-bit", depth.bits()),
        AudioFormat::Mp3 { kbps } => format!("MP3 {kbps} kbps"),
    };
    let music = (summary.seconds - opts.render.lead_secs - opts.render.tail_secs).max(0.0);
    println!("{origin} → {}", summary.path.display());
    println!("formato:  {kind}, {} Hz, estéreo", summary.sample_rate);
    println!("duração:  {:.3} s ({} s de entrada + {music:.3} s de música + {} s de cauda)", summary.seconds, opts.render.lead_secs, opts.render.tail_secs);
    println!("tamanho:  {bytes} bytes");
    println!("golpes:   {}", summary.hits);
    println!("pico:     {:.4} ({:+.2} dBFS)", summary.peak, db(summary.peak));
    println!("rms:      {:.4} ({:+.2} dBFS)", summary.rms, db(summary.rms));
    if let (Some(pre), Some(min)) = (summary.limiter_pre_gain, summary.limiter_min_gain) {
        println!("limiter:  ganho de normalização {:+.2} dB, redução máxima {:.2} dB", db(pre), -db(min));
    }
    if summary.normalize_gain != 1.0 {
        println!("pico normalizado em 0,98 (ganho {:+.2} dB)", db(summary.normalize_gain));
    }
    let took = t1.elapsed().as_secs_f64();
    println!("tempo:    samples {:.2} s, render + gravação {took:.2} s ({:.0}× tempo real)", (t1 - t0).as_secs_f64(), summary.seconds / took.max(1e-9));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions() {
        assert_eq!(parse_region("2..6").unwrap(), Region { start_bar: 2, end_bar: 6 });
        for bad in ["3", "4..4", "5..2", "a..b", "..", "-1..2"] {
            assert!(parse_region(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn song_files_by_extension() {
        assert!(is_song_file("pasta/Musica.WADAWKO.json"));
        assert!(is_song_file("x.taiko.json"));
        assert!(!is_song_file("Yatai Bayashi"));
        assert!(!is_song_file("p-abc-1"));
    }
}
