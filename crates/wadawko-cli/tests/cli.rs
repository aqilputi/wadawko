//! Testes de ponta a ponta do binário, sempre em bancos temporários.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

use wadawko_core::Song;
use wadawko_core::demo::demo_song;
use wadawko_store::{Db, add_song, list_projects, read_workspace};

static N: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("wadawko-cli-test-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cli(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wadawko-cli"))
        .args(args)
        .current_dir(dir)
        // nunca o banco de verdade do usuário
        .env_remove("WADAWKO_DB")
        .env("XDG_DATA_HOME", dir.join("xdg"))
        .env("HOME", dir.join("home"))
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = cli(dir, args);
    assert!(out.status.success(), "{args:?} falhou: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn fail(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = cli(dir, args);
    assert!(!out.status.success(), "{args:?} deveria falhar");
    (out.status.code().unwrap(), String::from_utf8(out.stderr).unwrap())
}

fn backup_fixture() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../wadawko-core/tests/fixtures/backup-localstorage-2026-10-02.json").to_str().unwrap().to_string()
}

fn song_titled(title: &str) -> Song {
    Song { title: title.into(), ..demo_song() }
}

#[test]
fn help_and_usage_errors() {
    let tmp = TempDir::new("help");
    let help = ok(&tmp.0, &["--help"]);
    for cmd in ["list", "export <título|id> [saída]", "render <título|id|arquivo> <saída.wav|mp3>", "import <arquivo>", "import-backup <arquivo>", "--db CAMINHO"] {
        assert!(help.contains(cmd), "ajuda sem {cmd}:\n{help}");
    }
    assert!(ok(&tmp.0, &["export", "--help"]).contains("saída padrão"));
    assert!(ok(&tmp.0, &["render", "--help"]).contains("--region A..B"));
    assert!(ok(&tmp.0, &["--version"]).starts_with("wadawko-cli "));

    assert_eq!(fail(&tmp.0, &[]).0, 2);
    let (code, err) = fail(&tmp.0, &["frobnicate"]);
    assert_eq!(code, 2);
    assert!(err.contains("comando desconhecido"));
    assert_eq!(fail(&tmp.0, &["export"]).0, 2);
    assert_eq!(fail(&tmp.0, &["list", "sobrando"]).0, 2);
    assert_eq!(fail(&tmp.0, &["list", "--db"]).0, 2);
    assert_eq!(fail(&tmp.0, &["import", "a.json", "--nope"]).0, 2);
    assert!(std::fs::read_dir(&tmp.0).unwrap().next().is_none(), "erro de uso não cria nada");
}

#[test]
fn list_is_read_only_and_needs_an_existing_db() {
    let tmp = TempDir::new("list");
    let (code, err) = fail(&tmp.0, &["--db", "nao-existe.db", "list"]);
    assert_eq!(code, 1);
    assert!(err.contains("banco não encontrado"));
    assert!(!tmp.join("nao-existe.db").exists(), "list não cria banco");
    // sem --db: caminho XDG
    let (_, err) = fail(&tmp.0, &["list"]);
    assert!(err.contains("xdg/wadawko/wadawko.db"), "{err}");

    let path = tmp.join("w.db");
    let (a, b);
    {
        let db = Db::open(&path).unwrap();
        a = add_song(&db, &song_titled("Renshū"), false).unwrap();
        b = add_song(&db, &Song { tempo: 174.0, ..song_titled("Mikazuchi") }, true).unwrap();
        db.set("wadawko:project:p-quebrada-1", "{\"version\":7}").unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    let out = ok(&tmp.0, &["list", "--db", "w.db"]);
    assert_eq!(std::fs::read(&path).unwrap(), before, "list não altera o banco");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with("banco: ") && lines[0].ends_with("w.db"));
    assert!(lines[2].starts_with(&format!("  {a}  Renshū")), "{out}");
    assert!(lines[3].starts_with(&format!("* {b}  Mikazuchi")), "{out}");
    assert!(lines[3].contains("  174  "), "{out}");
    assert!(lines[4].starts_with("? p-quebrada-1") && lines[4].contains("ILEGÍVEL"), "{out}");
    assert_eq!(lines[5], "3 música(s), 1 fora do workspace (?), 1 ilegível(is)");

    // $WADAWKO_DB também vale
    let out = Command::new(env!("CARGO_BIN_EXE_wadawko-cli")).arg("list").env("WADAWKO_DB", &path).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("Mikazuchi"));
}

#[test]
fn export_by_title_or_id() {
    let tmp = TempDir::new("export");
    let path = tmp.join("w.db");
    let song = song_titled("Yatai Bayashi");
    let (id, dup1);
    {
        let db = Db::open(&path).unwrap();
        id = add_song(&db, &song, false).unwrap();
        dup1 = add_song(&db, &song_titled("Renshū"), false).unwrap();
        add_song(&db, &song_titled("Renshū"), false).unwrap();
    }
    let before = std::fs::read(&path).unwrap();

    // por trecho do título, nome padrão na pasta atual
    let out = ok(&tmp.0, &["--db", "w.db", "export", "yatai"]);
    assert!(out.contains("yatai-bayashi.wadawko.json"), "{out}");
    let text = std::fs::read_to_string(tmp.join("yatai-bayashi.wadawko.json")).unwrap();
    assert_eq!(text, song.to_json_pretty());

    // por id, caminho explícito e pasta
    ok(&tmp.0, &["--db", "w.db", "export", &id, "saida.json"]);
    assert_eq!(std::fs::read_to_string(tmp.join("saida.json")).unwrap(), text);
    std::fs::create_dir(tmp.join("pasta")).unwrap();
    ok(&tmp.0, &["--db", "w.db", "export", &dup1, "pasta"]);
    assert!(tmp.join("pasta/renshū.wadawko.json").is_file());

    // saída padrão
    let out = ok(&tmp.0, &["--db", "w.db", "export", "Yatai Bayashi", "-"]);
    assert_eq!(Song::from_json(&out).unwrap(), song);

    // ambíguo e inexistente
    let (code, err) = fail(&tmp.0, &["--db", "w.db", "export", "Renshū"]);
    assert_eq!(code, 1);
    assert!(err.contains("ambíguo") && err.contains(&dup1), "{err}");
    assert!(fail(&tmp.0, &["--db", "w.db", "export", "nada"]).1.contains("nenhuma música"));
    assert_eq!(std::fs::read(&path).unwrap(), before, "export não altera o banco");
}

#[test]
fn import_adds_a_new_song() {
    let tmp = TempDir::new("import");
    let file = tmp.join("m.taiko.json");
    std::fs::write(&file, song_titled("Importada").to_json_pretty()).unwrap();

    // cria o banco no caminho XDG quando não há --db
    let out = ok(&tmp.0, &["import", "m.taiko.json"]);
    assert!(out.contains("importada: Importada (p-"), "{out}");
    let xdg_db = tmp.join("xdg/wadawko/wadawko.db");
    assert!(xdg_db.is_file());

    ok(&tmp.0, &["import", "m.taiko.json"]);
    ok(&tmp.0, &["import", "--open", "m.taiko.json"]);
    let db = Db::open_read_only(&xdg_db).unwrap();
    let list = list_projects(&db).unwrap();
    assert_eq!(list.len(), 3, "cada import é uma música nova; não há demo de brinde");
    assert!(list.iter().all(|p| p.meta.title == "Importada" && p.song.is_ok()));
    assert!(list[2].current, "--open troca a música aberta");
    drop(db);

    // arquivo inválido: erro, e nada de banco criado
    std::fs::write(tmp.join("ruim.json"), "{\"version\":2}").unwrap();
    let (code, err) = fail(&tmp.0, &["--db", "novo.db", "import", "ruim.json"]);
    assert_eq!(code, 1);
    assert!(err.contains("não parece ser uma música"), "{err}");
    assert!(fail(&tmp.0, &["--db", "novo.db", "import", "nao-existe.json"]).1.contains("nao-existe.json"));
    assert!(!tmp.join("novo.db").exists());
}

#[test]
fn import_backup_then_list_and_export() {
    let tmp = TempDir::new("backup");
    let out = ok(&tmp.0, &["--db", "w.db", "import-backup", &backup_fixture()]);
    assert_eq!(out.lines().filter(|l| l.starts_with("+ p-")).count(), 2, "{out}");
    assert!(out.contains("+ wadawko:theme"));
    assert!(out.contains("2 acrescentada(s), 0 substituída(s), 0 igual(is), 0 mantida(s), 0 inválida(s)"), "{out}");

    let list = ok(&tmp.0, &["--db", "w.db", "list"]);
    assert!(list.contains("2 música(s)") && !list.contains("ILEGÍVEL"), "{list}");
    assert_eq!(list.lines().filter(|l| l.starts_with('*')).count(), 1);

    // de novo: nada muda
    let out = ok(&tmp.0, &["--db", "w.db", "import-backup", &backup_fixture(), "--overwrite"]);
    assert!(out.contains("0 acrescentada(s), 0 substituída(s), 2 igual(is)"), "{out}");

    // o que sai do export é a música do backup
    let dict: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(backup_fixture()).unwrap()).unwrap();
    let db = Db::open_read_only(tmp.join("w.db")).unwrap();
    let current = read_workspace(&db).unwrap().unwrap().current_id.unwrap();
    drop(db);
    let exported: serde_json::Value = serde_json::from_str(&ok(&tmp.0, &["--db", "w.db", "export", &current, "-"])).unwrap();
    let original: serde_json::Value = serde_json::from_str(dict[format!("wadawko:project:{current}")].as_str().unwrap()).unwrap();
    assert_eq!(exported, original);

    let (code, err) = fail(&tmp.0, &["--db", "outro.db", "import-backup", "nao-existe.json"]);
    assert_eq!(code, 1);
    assert!(err.contains("não encontrado"));
    assert!(!tmp.join("outro.db").exists());
}

/// (canais, taxa, bits, quadros) de um WAV PCM de cabeçalho canônico (44 bytes).
fn wav_info(path: &Path) -> (u16, u32, u16, usize) {
    let b = std::fs::read(path).unwrap();
    assert_eq!((&b[0..4], &b[8..16], &b[36..40]), (&b"RIFF"[..], &b"WAVEfmt "[..], &b"data"[..]), "cabeçalho WAV");
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (channels, rate, bits, data) = (u16_at(22), u32_at(24), u16_at(34), u32_at(40) as usize);
    assert_eq!(b.len(), 44 + data, "tamanho do bloco data");
    assert_eq!(u32_at(4) as usize, 36 + data);
    (channels, rate, bits, data / (channels as usize * bits as usize / 8))
}

/// valor de uma linha `rótulo:  valor` do resumo
fn field<'a>(out: &'a str, label: &str) -> &'a str {
    out.lines().find_map(|l| l.strip_prefix(label)).unwrap_or_else(|| panic!("resumo sem {label}:\n{out}")).trim()
}

#[test]
fn render_from_db_or_file() {
    let tmp = TempDir::new("render");
    let path = tmp.join("w.db");
    let song = song_titled("Yatai Bayashi");
    let (id, dup1);
    {
        let db = Db::open(&path).unwrap();
        id = add_song(&db, &song, false).unwrap();
        dup1 = add_song(&db, &song_titled("Renshū"), false).unwrap();
        add_song(&db, &song_titled("Renshū"), false).unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    // 1 compasso e cauda curta: o teste roda em build de debug
    let short = ["--region", "0..1", "--tail", "0.25"];

    // por trecho do título: WAV 16-bit 48 kHz, progresso no stderr, resumo no stdout
    let out = cli(&tmp.0, &[&["--db", "w.db", "render", "yatai", "a.wav"], &short[..]].concat());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (stdout, stderr) = (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap());
    assert!(stdout.starts_with(&format!("Yatai Bayashi ({id}) → a.wav\n")), "{stdout}");
    assert!(stderr.contains("renderizando…") && stderr.contains("codificando…"), "{stderr}");
    assert!(!stdout.contains("renderizando"), "progresso só no stderr: {stdout}");
    let (channels, rate, bits, frames) = wav_info(&tmp.join("a.wav"));
    assert_eq!((channels, rate, bits), (2, 48_000, 16));
    assert_eq!(field(&stdout, "formato:"), "WAV 16-bit, 48000 Hz, estéreo");
    assert_eq!(field(&stdout, "tamanho:"), format!("{} bytes", 44 + frames * 4));
    let secs: f64 = field(&stdout, "duração:").split(' ').next().unwrap().parse().unwrap();
    assert!((secs - frames as f64 / 48_000.0).abs() < 1e-3, "{secs} s × {frames} quadros");
    // 1 compasso de 4 tempos a `tempo` BPM + 0,1 s de entrada + 0,25 s de cauda
    let expected = 0.1 + 4.0 * 60.0 / song.tempo + 0.25;
    assert!((secs - expected).abs() < 0.05, "{secs} s, esperado ~{expected} s");
    assert!(field(&stdout, "golpes:").parse::<usize>().unwrap() > 0, "{stdout}");
    assert!(field(&stdout, "pico:").split(' ').next().unwrap().parse::<f32>().unwrap() > 0.01, "silêncio? {stdout}");

    // por id, 24-bit, 44,1 kHz, --quiet; opções em qualquer posição
    let out = cli(&tmp.0, &[&["render", "--bits=24", &id, "b.wav", "--sr", "44100", "--quiet", "--db", "w.db"], &short[..]].concat());
    assert!(out.status.success() && out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
    let (_, rate, bits, frames24) = wav_info(&tmp.join("b.wav"));
    assert_eq!((rate, bits), (44_100, 24));
    assert!((frames24 as f64 / 44_100.0 - secs).abs() < 1e-3);

    // de um arquivo de música: nem precisa de banco; mesma semente = mesmo áudio
    std::fs::write(tmp.join("m.taiko.json"), song.to_json_pretty()).unwrap();
    let out = ok(&tmp.0, &[&["render", "m.taiko.json", "c.wav", "--quiet"], &short[..]].concat());
    assert!(out.starts_with("m.taiko.json → c.wav\n"), "{out}");
    assert_eq!(std::fs::read(tmp.join("c.wav")).unwrap(), std::fs::read(tmp.join("a.wav")).unwrap());
    assert!(!tmp.join("xdg").exists(), "render de arquivo não abre nem cria banco");

    // metrônomo e a música inteira: mais longa que 1 compasso
    ok(&tmp.0, &["render", "m.taiko.json", "d.wav", "--quiet", "--metronome", "--tail", "0", "--seed", "7"]);
    assert!(wav_info(&tmp.join("d.wav")).3 > frames);

    // erros de uso (2): nada é gravado
    for args in [
        &["--db", "w.db", "render", "yatai"][..],
        &["--db", "w.db", "render", "yatai", "x.ogg"],
        &["--db", "w.db", "render", "yatai", "x.mp3", "--bits", "24"],
        &["--db", "w.db", "render", "yatai", "x.wav", "--bits", "8"],
        &["--db", "w.db", "render", "yatai", "x.wav", "--region", "3..1"],
        &["--db", "w.db", "render", "yatai", "x.wav", "--sr", "abc"],
        &["--db", "w.db", "render", "yatai", "x.wav", "--nope"],
    ] {
        let (code, err) = fail(&tmp.0, args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.contains("Uso: wadawko-cli [--db CAMINHO] render"), "{err}");
    }
    // erros de execução (1): ambíguo, inexistente, região fora, arquivo ruim, sem banco
    let (code, err) = fail(&tmp.0, &["--db", "w.db", "render", "Renshū", "x.wav"]);
    assert_eq!(code, 1);
    assert!(err.contains("ambíguo") && err.contains(&dup1), "{err}");
    assert!(fail(&tmp.0, &["--db", "w.db", "render", "nada", "x.wav"]).1.contains("nenhuma música"));
    assert!(fail(&tmp.0, &["--db", "w.db", "render", "yatai", "x.wav", "--region", "0..999"]).1.contains("compasso(s)"));
    assert!(fail(&tmp.0, &["render", "nao-existe.wadawko.json", "x.wav"]).1.contains("nao-existe.wadawko.json"));
    std::fs::write(tmp.join("ruim.json"), "{\"version\":2}").unwrap();
    assert!(fail(&tmp.0, &["render", "ruim.json", "x.wav"]).1.contains("não parece ser uma música"));
    assert!(fail(&tmp.0, &["--db", "nao-existe.db", "render", "yatai", "x.wav"]).1.contains("banco não encontrado"));
    assert!(!tmp.join("x.wav").exists() && !tmp.join("x.mp3").exists() && !tmp.join("x.ogg").exists());
    assert!(!tmp.join("nao-existe.db").exists());
    assert_eq!(std::fs::read(&path).unwrap(), before, "render não altera o banco");
}

#[test]
fn render_mp3_when_ffmpeg_is_available() {
    if !Command::new("ffmpeg").arg("-version").output().is_ok_and(|o| o.status.success()) {
        eprintln!("ffmpeg ausente: teste de MP3 pulado");
        return;
    }
    let tmp = TempDir::new("render-mp3");
    std::fs::write(tmp.join("m.wadawko.json"), song_titled("Em MP3").to_json_pretty()).unwrap();
    let out = ok(&tmp.0, &["render", "m.wadawko.json", "m.mp3", "--region", "0..1", "--tail", "0.25", "--quiet"]);
    assert_eq!(field(&out, "formato:"), "MP3 192 kbps, 48000 Hz, estéreo");
    let bytes = std::fs::read(tmp.join("m.mp3")).unwrap();
    assert!(bytes.starts_with(b"ID3") && bytes.len() > 4_000, "{} bytes", bytes.len());
    assert_eq!(field(&out, "tamanho:"), format!("{} bytes", bytes.len()));
}
