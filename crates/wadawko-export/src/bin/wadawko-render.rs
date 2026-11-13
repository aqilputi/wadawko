//! `wadawko-render <arquivo .taiko.json|.wadawko.json> <saida.wav|mp3>` —
//! renderiza uma música offline com o mesmo engine do playback.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use wadawko_audio::{MasterMode, Region, SampleSet, compile, CompileOptions};
use wadawko_export::{AudioFormat, BitDepth, ExportOptions, ExportStage, export_song, load_song_file};
use wadawko_samples::SampleBank;

const USAGE: &str = "uso: wadawko-render <arquivo .taiko.json|.wadawko.json> <saida.wav|mp3> [opções]

opções:
  --project <título|id>   escolhe a música quando o arquivo é um backup do localStorage
  --sr <Hz>               taxa de amostragem (padrão 48000)
  --bits <16|24>          profundidade do WAV (padrão 16)
  --seed <n>              semente da humanização (padrão 1; mesma semente = mesmo arquivo)
  --region <a>..<b>       só os compassos [a, b) (base 0)
  --tail <s>              cauda depois do fim (padrão 3)
  --metronome             inclui o metrônomo
  --live-master           cadeia do playback (compressor + soft clip) em vez do export limpo
  --no-limiter            sem o limiter com lookahead (teto −1,5 dBFS)
  --samples-dir <dir>     samples de um diretório em vez dos embutidos
  --quiet                 não mostra o progresso";

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut positional: Vec<String> = Vec::new();
    let mut opts = ExportOptions::default();
    let (mut project, mut samples_dir, mut bits, mut quiet) = (None, None, None, false);
    while let Some(a) = args.next() {
        let mut value = |name: &str| args.next().with_context(|| format!("{name} precisa de um valor"));
        match a.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--project" => project = Some(value("--project")?),
            "--sr" => opts.render.sample_rate = value("--sr")?.parse().context("--sr")?,
            "--bits" => bits = Some(value("--bits")?),
            "--seed" => opts.render.seed = value("--seed")?.parse().context("--seed")?,
            "--tail" => opts.render.tail_secs = value("--tail")?.parse().context("--tail")?,
            "--region" => {
                let v = value("--region")?;
                let (a, b) = v.split_once("..").context("--region espera a..b")?;
                opts.render.region = Some(Region { start_bar: a.parse().context("--region")?, end_bar: b.parse().context("--region")? });
            }
            "--metronome" => opts.render.metronome = true,
            "--live-master" => opts.render.master = MasterMode::Live,
            "--no-limiter" => opts.render.limiter_ceiling_db = None,
            "--samples-dir" => samples_dir = Some(PathBuf::from(value("--samples-dir")?)),
            "--quiet" => quiet = true,
            s if s.starts_with("--") => bail!("opção desconhecida: {s}\n\n{USAGE}"),
            _ => positional.push(a),
        }
    }
    let [input, output] = positional.as_slice() else { bail!("{USAGE}") };
    let (input, output) = (PathBuf::from(input), PathBuf::from(output));
    let mut format = AudioFormat::from_path(&output)?;
    match (bits.as_deref(), &mut format) {
        (None | Some("16"), _) => {}
        (Some("24"), AudioFormat::Wav(d)) => *d = BitDepth::Pcm24,
        (Some("24"), _) => bail!("--bits 24 só vale para .wav"),
        (Some(other), _) => bail!("--bits {other}: use 16 ou 24"),
    }
    opts.format = Some(format);

    let song = load_song_file(&input, project.as_deref())?;
    let t0 = Instant::now();
    let bank = SampleBank::load_from(samples_dir.as_deref()).context("carregar samples")?;
    let samples = SampleSet::from_bank(&bank);
    let schedule = compile(&song, &bank, opts.render.sample_rate, &CompileOptions { region: opts.render.region, skip_inaudible: true, ..Default::default() });
    let bars = schedule.time.order.len();
    eprintln!(
        "{:?}: {} BPM, {} linhas, {} compassos ({} tocados com ritornellos), swing {} → {:.2} s de música",
        song.title,
        song.tempo,
        song.tracks.len(),
        song.measures.len(),
        bars,
        song.swing(),
        schedule.seconds(song.tempo)
    );
    let t1 = Instant::now();
    let mut last = (ExportStage::Rendering, -1i32);
    let summary = export_song(
        &song,
        &bank,
        &samples,
        &output,
        &opts,
        &mut |stage, p| {
            let pct = (p * 10.0) as i32;
            if !quiet && (stage, pct) != last {
                last = (stage, pct);
                eprint!("\r{} {:3.0} %   ", stage.label(), p * 100.0);
            }
        },
        &AtomicBool::new(false),
    )?;
    if !quiet {
        eprintln!();
    }
    let db = |x: f32| if x > 0.0 { 20.0 * x.log10() } else { f32::NEG_INFINITY };
    println!("arquivo:  {} ({:?}, {} Hz)", summary.path.display(), summary.format, summary.sample_rate);
    println!("duração:  {:.3} s (0.1 s de entrada + {:.3} s de música + {} s de cauda)", summary.seconds, schedule.seconds(song.tempo), opts.render.tail_secs);
    println!("golpes:   {}", summary.hits);
    println!("pico:     {:.4} ({:+.2} dBFS)", summary.peak, db(summary.peak));
    println!("rms:      {:.4} ({:+.2} dBFS)", summary.rms, db(summary.rms));
    if let (Some(pre), Some(min)) = (summary.limiter_pre_gain, summary.limiter_min_gain) {
        println!("limiter:  ganho de normalização {:+.2} dB, redução máxima {:.2} dB", db(pre), -db(min));
    }
    if summary.normalize_gain != 1.0 {
        println!("pico normalizado em 0,98 (ganho {:+.2} dB)", db(summary.normalize_gain));
    }
    println!("tempo:    samples {:.2} s, render + gravação {:.2} s ({:.0}× tempo real)", (t1 - t0).as_secs_f64(), t1.elapsed().as_secs_f64(), summary.seconds / t1.elapsed().as_secs_f64());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e:#}");
            ExitCode::FAILURE
        }
    }
}
