//! WAV/MP3, normalização, export em thread com progresso/cancelamento e o
//! binário `wadawko-render`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

use wadawko_audio::{RenderOptions, SampleSet};
use wadawko_core::demo::demo_song;
use wadawko_export::*;
use wadawko_samples::SampleBank;

fn bank() -> &'static SampleBank {
    static BANK: OnceLock<SampleBank> = OnceLock::new();
    BANK.get_or_init(|| SampleBank::load_embedded().unwrap())
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wadawko-export-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

const BACKUP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../wadawko-core/tests/fixtures/backup-localstorage-2026-10-02.json");

/// lê um WAV PCM: (taxa, bits, amostras normalizadas)
fn read_wav(bytes: &[u8]) -> (u32, u16, Vec<f32>) {
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
    assert_eq!(u32_at(4) as usize, bytes.len() - 8);
    assert_eq!((u16_at(20), u16_at(22)), (1, 2), "PCM estéreo");
    let (sr, bits) = (u32_at(24), u16_at(34));
    assert_eq!(u32_at(28), sr * 2 * bits as u32 / 8);
    assert_eq!(u16_at(32), 2 * bits / 8);
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(u32_at(40) as usize, bytes.len() - 44);
    let n = (bits / 8) as usize;
    let samples = bytes[44..]
        .chunks(n)
        .map(|c| {
            let mut v = [0u8; 4];
            v[..n].copy_from_slice(c);
            let i = i32::from_le_bytes(v) << (32 - bits) >> (32 - bits);
            i as f32 / (1i64 << (bits - 1)) as f32
        })
        .collect();
    (sr, bits, samples)
}

#[test]
fn wav_16_and_24_bit_round_trip() {
    let src = [0.0f32, 1.0, -1.0, 0.5, -0.5, 0.123_456, 2.0, -3.0, f32::NAN, 1e-5];
    let w16 = encode_wav(&src, 44_100, BitDepth::Pcm16);
    assert_eq!(w16.len(), 44 + src.len() * 2);
    let (sr, bits, back) = read_wav(&w16);
    assert_eq!((sr, bits), (44_100, 16));
    // como o toInt16 da web: +1 → 32767, −1 → −32768; fora de [−1, 1] satura; NaN vira 0
    assert_eq!(&w16[44..52], &[0, 0, 0xff, 0x7f, 0x00, 0x80, 0xff, 0x3f]);
    assert_eq!((back[6], back[7], back[8]), (32767.0 / 32768.0, -1.0, 0.0));
    assert!(src.iter().zip(&back).take(6).all(|(a, b)| (a - b).abs() < 1.0 / 32768.0 + 1e-6));
    assert_eq!(back[9], 0.0, "abaixo de 1 LSB em 16 bits");
    let w24 = encode_wav(&src, 48_000, BitDepth::Pcm24);
    assert_eq!(w24.len(), 44 + src.len() * 3);
    let (sr, bits, back) = read_wav(&w24);
    assert_eq!((sr, bits), (48_000, 24));
    assert_eq!(&w24[44 + 3..44 + 9], &[0xff, 0xff, 0x7f, 0x00, 0x00, 0x80]);
    assert!(src.iter().zip(&back).take(6).all(|(a, b)| (a - b).abs() < 1.0 / 8_388_608.0 + 1e-7));
    assert!((back[9] - 1e-5).abs() < 2e-7, "24 bits guarda o que 16 perde");
}

#[test]
fn peak_normalization_only_attenuates() {
    let mut loud = vec![0.5, -1.4, 0.7];
    let g = normalize_peak(&mut loud, PEAK_CEILING);
    assert!((g - 0.7).abs() < 1e-6 && (loud[1] + 0.98).abs() < 1e-6 && (loud[0] - 0.35).abs() < 1e-6);
    let mut quiet = vec![0.1, -0.2];
    assert_eq!(normalize_peak(&mut quiet, PEAK_CEILING), 1.0);
    assert_eq!(quiet, vec![0.1, -0.2], "não infla músicas baixas");
    let mut edge = vec![0.98];
    assert_eq!(normalize_peak(&mut edge, PEAK_CEILING), 1.0);
}

#[test]
fn format_from_extension() {
    assert_eq!(AudioFormat::from_path(Path::new("a/b.WAV")).unwrap(), AudioFormat::Wav(BitDepth::Pcm16));
    assert_eq!(AudioFormat::from_path(Path::new("x.mp3")).unwrap(), AudioFormat::Mp3 { kbps: 192 });
    assert!(AudioFormat::from_path(Path::new("x.ogg")).is_err());
    assert!(AudioFormat::from_path(Path::new("semextensao")).is_err());
    assert_eq!(with_extension("m/renshu".into(), AudioFormat::Mp3 { kbps: 192 }), PathBuf::from("m/renshu.mp3"));
    assert_eq!(with_extension("m/renshu.MP3".into(), AudioFormat::Mp3 { kbps: 192 }), PathBuf::from("m/renshu.MP3"));
    assert_eq!(with_extension("v1.2".into(), AudioFormat::Wav(BitDepth::Pcm24)), PathBuf::from("v1.2.wav"));
}

#[test]
fn export_wav_matches_the_offline_render() {
    let song = demo_song();
    let samples = SampleSet::from_bank(bank());
    let path = tmp("demo.wav");
    let opts = ExportOptions { render: RenderOptions { seed: 5, ..Default::default() }, ..Default::default() };
    let mut stages = Vec::new();
    let summary = export_song(&song, bank(), &samples, &path, &opts, &mut |s, p| stages.push((s, p)), &AtomicBool::new(false)).unwrap();
    assert_eq!(summary.format, AudioFormat::Wav(BitDepth::Pcm16));
    assert_eq!((summary.sample_rate, summary.hits), (48_000, 67));
    assert!((summary.seconds - 12.7).abs() < 1e-3);
    assert!(summary.peak <= 0.8415 && summary.rms > 0.05, "{summary:?}");
    assert_eq!(summary.normalize_gain, 1.0, "o limiter já deixa abaixo de 0,98");
    // progresso: renderizando 0→~1, codificando, pronto
    assert_eq!(stages.first().unwrap(), &(ExportStage::Rendering, 0.0));
    assert_eq!(stages.last().unwrap(), &(ExportStage::Done, 1.0));
    assert!(stages.iter().any(|(s, _)| *s == ExportStage::Encoding));
    assert!(stages.windows(2).all(|w| (w[0].0 as u8, w[0].1) <= (w[1].0 as u8, w[1].1)));
    // o arquivo é o render (mesma semente) quantizado em 16 bits
    let (sr, bits, pcm) = read_wav(&std::fs::read(&path).unwrap());
    assert_eq!((sr, bits), (48_000, 16));
    let reference = wadawko_audio::render(&song, bank(), &opts.render);
    assert_eq!(pcm.len(), reference.samples.len());
    assert!(pcm.iter().zip(&reference.samples).all(|(a, b)| (a - b).abs() < 2.0 / 32768.0));
    // 24 bits pedido explicitamente
    let p24 = tmp("demo24.wav");
    let o24 = ExportOptions { format: Some(AudioFormat::Wav(BitDepth::Pcm24)), ..opts.clone() };
    export_song(&song, bank(), &samples, &p24, &o24, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    let (_, bits, pcm24) = read_wav(&std::fs::read(&p24).unwrap());
    assert_eq!(bits, 24);
    assert!(pcm24.iter().zip(&reference.samples).all(|(a, b)| (a - b).abs() < 2.0 / 8_388_608.0 + 1e-7));
    // extensão desconhecida: erro antes de renderizar
    assert!(export_song(&song, bank(), &samples, &tmp("demo.flac"), &opts, &mut |_, _| {}, &AtomicBool::new(false)).is_err());
}

#[test]
fn export_job_reports_progress_and_cancels() {
    let job = spawn_export(demo_song(), bank().clone(), tmp("job.wav"), ExportOptions::default());
    let summary = job.join().unwrap();
    assert!(summary.path.is_file() && summary.hits == 67);
    // cancelado antes de começar: erro e nenhum arquivo
    let path = tmp("cancelado.wav");
    let job = spawn_export(demo_song(), bank().clone(), path.clone(), ExportOptions::default());
    job.cancel();
    let err = job.join().unwrap_err();
    assert!(err.to_string().contains("cancelado"), "{err}");
    assert!(!path.exists());
    // progresso observável de fora
    let job = spawn_export(demo_song(), bank().clone(), tmp("job2.wav"), ExportOptions::default());
    assert!((0.0..=1.0).contains(&job.progress()));
    while !job.is_finished() {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!((job.stage(), job.progress()), (ExportStage::Done, 1.0));
    assert_eq!(job.stage().label(), "pronto");
    job.join().unwrap();
}

fn ffprobe(path: &Path, entries: &str) -> Option<String> {
    let out = Command::new("ffprobe").args(["-v", "error", "-show_entries", entries, "-of", "default=nw=1"]).arg(path).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

#[test]
fn export_mp3_192k_with_title() {
    if !ffmpeg_available() {
        eprintln!("ffmpeg ausente: teste de MP3 pulado");
        return;
    }
    let mut song = demo_song();
    song.title = "Renshū — teste \"aspas\"".into();
    let path = tmp("demo.mp3");
    let job = spawn_export(song.clone(), bank().clone(), path.clone(), ExportOptions::default());
    let summary = job.join().unwrap();
    assert_eq!(summary.format, AudioFormat::Mp3 { kbps: 192 });
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..3], b"ID3", "tag ID3v2 com o título");
    // 192 kbps × 12,7 s ≈ 305 kB
    assert!((bytes.len() as f64 - 192_000.0 / 8.0 * 12.7).abs() < 20_000.0, "{} bytes", bytes.len());
    if let Some(info) = ffprobe(&path, "format=duration,bit_rate:format_tags=title,comment:stream=codec_name,sample_rate,channels") {
        assert!(info.contains("codec_name=mp3") && info.contains("sample_rate=48000") && info.contains("channels=2"), "{info}");
        assert!(info.contains(&format!("TAG:title={}", song.title)), "{info}");
        assert!(info.contains("TAG:comment=WaDAWko"), "{info}");
        let field = |k: &str| info.lines().find_map(|l| l.strip_prefix(k)).unwrap().parse::<f64>().unwrap();
        assert!((field("duration=") - 12.7).abs() < 0.1, "{info}");
        assert!((field("bit_rate=") - 192_000.0).abs() < 4_000.0, "{info}");
    }
    // título explícito ganha do título da música
    let p2 = tmp("titulo.mp3");
    let samples = SampleSet::from_bank(bank());
    let opts = ExportOptions { title: Some("Outro nome".into()), ..Default::default() };
    export_song(&song, bank(), &samples, &p2, &opts, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    if let Some(info) = ffprobe(&p2, "format_tags=title") {
        assert!(info.contains("TAG:title=Outro nome"), "{info}");
    }
    // cancelar durante a codificação não deixa arquivo parcial
    let p3 = tmp("cancel.mp3");
    let cancel = AtomicBool::new(false);
    let res = export_song(
        &song,
        bank(),
        &samples,
        &p3,
        &ExportOptions::default(),
        &mut |s, _| {
            if s == ExportStage::Encoding {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
        &cancel,
    );
    assert!(res.unwrap_err().to_string().contains("cancelado"));
    assert!(!p3.exists());
}

#[test]
fn loads_song_files_and_backups() {
    let path = tmp("demo.taiko.json");
    std::fs::write(&path, demo_song().to_json()).unwrap();
    assert_eq!(load_song_file(&path, None).unwrap(), demo_song());
    // backup do localStorage: precisa escolher o projeto
    let err = load_song_file(Path::new(BACKUP), None).unwrap_err().to_string();
    assert!(err.contains("--project") && err.contains("papparapa"), "{err}");
    let p = load_song_file(Path::new(BACKUP), Some("papparapa")).unwrap();
    assert_eq!((p.title.as_str(), p.tempo, p.measures.len()), ("papparapa", 123.0, 18));
    // 3 projetos chamam "Renshū": pelo título é ambíguo, pelo id funciona
    let err = load_song_file(Path::new(BACKUP), Some("Renshū")).unwrap_err().to_string();
    assert!(err.contains("mais de um"), "{err}");
    assert_eq!(load_song_file(Path::new(BACKUP), Some("p-muog58u1-1")).unwrap().measures.len(), 131);
    assert!(load_song_file(Path::new(BACKUP), Some("nada")).is_err());
    // arquivo que não é música
    let bad = tmp("ruim.json");
    std::fs::write(&bad, "{\"version\": 2}").unwrap();
    assert!(load_song_file(&bad, None).is_err());
    assert!(load_song_file(&tmp("nao-existe.json"), None).is_err());
}

#[test]
fn wadawko_render_binary() {
    let bin = env!("CARGO_BIN_EXE_wadawko-render");
    let input = tmp("cli.taiko.json");
    std::fs::write(&input, demo_song().to_json()).unwrap();
    let out = tmp("cli.wav");
    let run = Command::new(bin).arg(&input).arg(&out).args(["--quiet", "--bits", "24", "--seed", "9"]).output().unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(stdout.contains("duração:  12.700 s") && stdout.contains("golpes:   67"), "{stdout}");
    let (sr, bits, pcm) = read_wav(&std::fs::read(&out).unwrap());
    assert_eq!((sr, bits, pcm.len()), (48_000, 24, 609_600 * 2));
    // mesma semente → arquivo idêntico; região e taxa pela linha de comando
    let out2 = tmp("cli2.wav");
    assert!(Command::new(bin).arg(&input).arg(&out2).args(["--quiet", "--bits", "24", "--seed", "9"]).status().unwrap().success());
    assert_eq!(std::fs::read(&out).unwrap(), std::fs::read(&out2).unwrap());
    let out3 = tmp("cli3.wav");
    assert!(Command::new(bin).arg(&input).arg(&out3).args(["--quiet", "--region", "0..1", "--sr", "44100", "--tail", "1"]).status().unwrap().success());
    let (sr, bits, pcm) = read_wav(&std::fs::read(&out3).unwrap());
    assert_eq!((sr, bits, pcm.len()), (44_100, 16, (3.5f64 * 44_100.0).ceil() as usize * 2));
    // erros: sem argumentos, extensão errada, backup sem --project
    assert!(!Command::new(bin).output().unwrap().status.success());
    assert!(!Command::new(bin).arg(&input).arg(tmp("x.ogg")).output().unwrap().status.success());
    let e = Command::new(bin).arg(BACKUP).arg(tmp("y.wav")).output().unwrap();
    assert!(!e.status.success() && String::from_utf8_lossy(&e.stderr).contains("--project"));
}
