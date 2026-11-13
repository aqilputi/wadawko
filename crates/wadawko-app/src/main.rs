//! WaDAWko — app desktop (Rust + GPUI). Fase 3 de PLANEJAMENTO-gpui.md + trilha
//! de teclado de PLANEJAMENTO-teclado.md. Arquitetura: ver `ARQUITETURA.md`.
//!
//! Uso: `wadawko [--db caminho] [--read-only] [--samples-dir pasta] [--dark]
//! [--mute] [--song "título"] [--bench N] [--demo modo] [--selftest]`
//! (`wadawko --help` lista tudo).

extern crate gpui_kit as gpui;

mod keys;
mod shell;
mod song_ext;
mod state;
mod stats;
mod theme;
mod ui;
mod views;

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gpui_kit::{App, AsyncApp, Bounds, Entity, Keystroke, WindowBounds, WindowOptions, prelude::*, px, size};
use wadawko_core::ops;
use wadawko_samples::SampleBank;
use wadawko_store::{Db, ProjectStore};

use shell::{HintScope, RENDERS, Shell};
use song_ext::SongExt;
use state::AppState;
use theme::Theme;
use views::grid::{Cur, Lane, Motion, VMode};

const HELP: &str = "\
WaDAWko — editor/DAW de taiko

  --db CAMINHO         banco SQLite (padrão: ~/.local/share/wadawko/wadawko.db)
  --migrate-from ARQ   se o banco de --db/padrão não existir, copia ARQ para lá (ARQ só é lido)
  --read-only          abre o banco sem gravar (edições ficam em memória)
  --samples-dir PASTA  carrega os .ogg de PASTA em vez dos embutidos
  --song TÍTULO        abre a música com esse título (busca aproximada)
  --dark | --light     força o tema (padrão: o salvo no banco)
  --mute               volume de saída 0 (o engine roda normalmente)
  --bench N            toca com follow e zoom oscilando por N s e imprime o relatório
  --demo MODO          palette | hints | whichkey | roothints | cmdline | config | visual | play | enter |
                       kiai | menu | dyn | simple | mixer | pessoas | kit | kit-new | esteira | lane | partitura
  --selftest           abre, pinta, toca, edita, desfaz/refaz, grava e sai (imprime linhas TESTE)
  --help
";

#[derive(Debug, Default, PartialEq)]
struct Args {
    db: Option<PathBuf>,
    migrate_from: Option<PathBuf>,
    read_only: bool,
    samples_dir: Option<PathBuf>,
    song: Option<String>,
    dark: Option<bool>,
    mute: bool,
    bench: Option<u64>,
    demo: Option<String>,
    selftest: bool,
    help: bool,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args::default();
    let mut it = args.iter().peekable();
    let value = |it: &mut std::iter::Peekable<std::slice::Iter<String>>, flag: &str| it.next().cloned().ok_or_else(|| format!("{flag} precisa de um valor"));
    while let Some(a) = it.next() {
        match a.as_str() {
            "--db" => out.db = Some(value(&mut it, a)?.into()),
            "--migrate-from" => out.migrate_from = Some(value(&mut it, a)?.into()),
            "--read-only" => out.read_only = true,
            "--samples-dir" => out.samples_dir = Some(value(&mut it, a)?.into()),
            "--song" => out.song = Some(value(&mut it, a)?),
            "--dark" => out.dark = Some(true),
            "--light" => out.dark = Some(false),
            "--mute" => out.mute = true,
            "--bench" => {
                // o número é opcional (padrão 15 s)
                let n = it.peek().and_then(|s| s.parse::<u64>().ok());
                if n.is_some() {
                    it.next();
                }
                out.bench = Some(n.unwrap_or(15));
            }
            "--demo" => out.demo = Some(value(&mut it, a)?),
            "--selftest" => out.selftest = true,
            "--help" | "-h" => out.help = true,
            other => return Err(format!("argumento desconhecido: {other}")),
        }
    }
    Ok(out)
}

fn open_store(args: &Args) -> anyhow::Result<(ProjectStore, String)> {
    let path = match &args.db {
        Some(p) => p.clone(),
        None => wadawko_store::default_db_path()?,
    };
    if let Some(src) = &args.migrate_from {
        match wadawko_store::migrate_db_if_missing(src, &path)? {
            wadawko_store::Migration::Copied { keys, bytes } => eprintln!("banco copiado de {} ({keys} chaves, {bytes} bytes)", src.display()),
            wadawko_store::Migration::DestinationExists => {}
            wadawko_store::Migration::SourceMissing => eprintln!("--migrate-from: {} não existe", src.display()),
        }
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty())
        && !args.read_only
    {
        std::fs::create_dir_all(dir)?;
    }
    let db = Db::open_with(&path, args.read_only)?;
    let mut store = ProjectStore::open(db)?;
    if let Some(title) = &args.song {
        let best = store.projects().iter().filter_map(|p| keys::fuzzy(title, &p.title).map(|s| (s + if p.title.eq_ignore_ascii_case(title) { 1000 } else { 0 }, p.id.clone()))).max_by_key(|x| x.0);
        match best {
            Some((_, id)) => {
                store.switch_project(&id)?;
            }
            None => eprintln!("--song: nenhuma música parecida com “{title}”"),
        }
    }
    let label = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    Ok((store, label))
}

fn main() {
    // pede ao PipeWire um quantum pequeno p/ este cliente (o padrão do grafo é 1024 = 21 ms,
    // somado ao buffer do ALSA). Feito antes de qualquer thread existir.
    if std::env::var_os("PIPEWIRE_LATENCY").is_none() {
        // SAFETY: ainda single-threaded (início do main, antes do GPUI/cpal).
        unsafe { std::env::set_var("PIPEWIRE_LATENCY", "256/48000") };
    }
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{HELP}");
            std::process::exit(2);
        }
    };
    if args.help {
        print!("{HELP}");
        return;
    }
    let (store, db_label) = match open_store(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("não consegui abrir o banco: {e:#}");
            std::process::exit(1);
        }
    };
    let dark = args.dark.unwrap_or_else(|| matches!(store.theme(), Ok(Some(wadawko_store::Theme::Dark))));
    let theme = if dark { Theme::dark() } else { Theme::light() };

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let fonts: Vec<Cow<'static, [u8]>> = vec![
            Cow::Borrowed(include_bytes!("../../../assets/fonts/ZenKakuGothicNew-Regular.ttf")),
            Cow::Borrowed(include_bytes!("../../../assets/fonts/ZenKakuGothicNew-Medium.ttf")),
            Cow::Borrowed(include_bytes!("../../../assets/fonts/ZenKakuGothicNew-Bold.ttf")),
            Cow::Borrowed(include_bytes!("../../../assets/fonts/ShipporiMincho-Bold.ttf")),
        ];
        cx.text_system().add_fonts(fonts).expect("registrar fontes");

        let state = cx.new(|_| AppState::new(store, theme, args.mute, db_label));

        // samples: decodificar bloqueia (~1 s) → thread de fundo; o áudio abre quando chegar
        let samples_dir = args.samples_dir.clone();
        let st = state.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let t0 = Instant::now();
            let bank = cx.background_executor().spawn(async move { SampleBank::load_from(samples_dir.as_deref()) }).await;
            let _ = st.update(cx, |s, cx| match bank {
                Ok(b) => {
                    eprintln!("samples: {} carregados em {:.2} s", b.len(), t0.elapsed().as_secs_f32());
                    s.attach_bank(b, cx)
                }
                Err(e) => {
                    s.audio_error = Some(format!("samples: {e:#}"));
                    s.flash(format!("não carreguei os samples: {e:#}"));
                    cx.notify();
                }
            });
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
        let bench = args.bench;
        let st = state.clone();
        let win = gpui_kit::open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), titlebar: Some(gpui_kit::TitlebarOptions { title: Some("WaDAWko".into()), ..Default::default() }), ..Default::default() },
            cx,
            move |window, cx| cx.new(|cx| Shell::new(window, cx, st.clone(), bench)),
        )
        .expect("abrir janela");

        // fechar a janela = gravar o que estiver pendente e sair
        let st = state.clone();
        cx.on_window_closed(move |cx, _| {
            let _ = st.update(cx, |s, _| s.flush());
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        if args.selftest {
            selftest(state.clone(), win.1.clone(), win.0, cx);
        }
        if let Some(demo) = args.demo.clone() {
            run_demo(demo, state.clone(), win.1.clone(), win.0, cx);
        }
        cx.activate(true);
    });
}

type Win = gpui_kit::AnyWindowHandle;

/// Espera o áudio ficar pronto (até `secs`). true = pronto.
async fn wait_audio(state: &Entity<AppState>, cx: &mut AsyncApp, secs: f32) -> bool {
    let t0 = Instant::now();
    loop {
        let ready = state.read_with(cx, |s, _| s.audio.is_some() || s.audio_error.is_some());
        if ready || t0.elapsed().as_secs_f32() > secs {
            return ready;
        }
        cx.background_executor().timer(Duration::from_millis(50)).await;
    }
}

/// `--selftest`: prova de ponta a ponta com janela. Imprime linhas `TESTE …`.
fn selftest(state: Entity<AppState>, shell: Entity<Shell>, w: Win, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let sleep = async |cx: &mut AsyncApp, ms: u64| cx.background_executor().timer(Duration::from_millis(ms)).await;
        sleep(cx, 800).await;
        let ready = wait_audio(&state, cx, 20.0).await;
        state.read_with(cx, |s, _| {
            let song = s.song();
            println!(
                "TESTE música: “{}” · {} linhas · {} compassos · {} seções · {} células · banco {}{} · {} músicas",
                song.title,
                song.tracks.len(),
                song.measures.len(),
                song.secs().len(),
                song.total_cells(),
                s.db_label,
                if s.read_only() { " (somente leitura)" } else { "" },
                s.store.projects().len()
            );
            println!("TESTE áudio: pronto={ready} · erro={:?} · eventos no schedule={} · mudo={}", s.audio_error, s.schedule.events.len(), s.mute);
        });
        sleep(cx, 700).await;
        let (paints, paint_max) = shell::STATS_SNAP.with(|s| s.get());
        let renders0 = RENDERS.load(Ordering::Relaxed);
        println!("TESTE janela: {renders0} renders da raiz · {paints} paints de seção da grade (máx {paint_max:.2} ms) · {} renders da grade", shell::DBG_GRID_RENDERS.load(Ordering::Relaxed));
        if renders0 == 0 || paints == 0 {
            println!("TESTE aviso: o compositor NÃO entregou frames (janela fora de vista?) — sem medida de pintura");
        }

        // tocar: o relógio do áudio tem de andar
        let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::transport::PlayFromStart), cx));
        sleep(cx, 600).await;
        let snap = |cx: &mut AsyncApp| state.read_with(cx, |s, _| s.audio.as_ref().map(|a| a.clock().snapshot()));
        let a = snap(cx);
        let r1 = RENDERS.load(Ordering::Relaxed);
        sleep(cx, 2000).await;
        let b = snap(cx);
        let r2 = RENDERS.load(Ordering::Relaxed);
        let (hits, ph) = state.read_with(cx, |s, _| (s.hits.len(), s.playhead()));
        match (a, b) {
            (Some(a), Some(b)) => println!(
                "TESTE tocar: relógio {:.0} → {:.0} ticks em 2 s (tocando={}) · {:.0} BPM · playhead {:?} · golpes recentes na fila={hits}",
                a.pos_tick,
                b.pos_tick,
                b.playing,
                b.bpm,
                ph.map(|p| (p.mi, (p.frac * 100.0).round() / 100.0))
            ),
            _ => println!("TESTE tocar: sem áudio"),
        }
        if r2 > r1 {
            println!("TESTE frames tocando: {} renders em 2 s (≈{:.0} fps)", r2 - r1, (r2 - r1) as f32 / 2.0);
        } else {
            println!("TESTE frames tocando: 0 renders em 2 s — o compositor não entregou frames (janela fora de vista); fps não medido");
        }
        let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::transport::Stop), cx));
        sleep(cx, 200).await;

        // edição programática pela grade (caminho único: GridView::set_cell → AppState::edit)
        let before = state.read_with(cx, |s, _| s.song().cell(0, 0, 1).map(str::to_string));
        let new = if before.as_deref() == Some("tsuku") { "doko" } else { "tsuku" };
        let _ = shell.update(cx, |sh, cx| {
            sh.grid.update(cx, |g, cx| {
                g.lane = Lane::Cells;
                g.cursor = Some(Cur { m: 0, t: 0, k: 1 });
                g.set_cell(Cur { m: 0, t: 0, k: 1 }, Some(new.to_string()), cx);
            })
        });
        let read = |cx: &mut AsyncApp| state.read_with(cx, |s, _| s.song().cell(0, 0, 1).map(str::to_string));
        let after = read(cx);
        let _ = state.update(cx, |s, cx| {
            s.undo(cx);
        });
        let undone = read(cx);
        let _ = state.update(cx, |s, cx| {
            s.redo(cx);
        });
        let redone = read(cx);
        println!("TESTE edição: célula (linha 1, compasso 1, célula 2) antes={before:?} → depois={after:?} · desfazer={undone:?} · refazer={redone:?}");
        sleep(cx, 700).await; // autosave (400 ms)
        let _ = state.update(cx, |s, _| {
            let dirty_before = s.store.has_unsaved_changes();
            let r = s.flush();
            println!("TESTE gravação: pendente antes do flush={dirty_before} · flush={:?} · pendente depois={} · erro={:?}", r.map_err(|e| format!("{e:#}")), s.store.has_unsaved_changes(), s.store.autosave_error());
        });
        let _ = cx.update(|cx| cx.quit());
    })
    .detach();
}

/// `--demo modo`: abre a janela num estado específico (screenshots e testes manuais).
fn run_demo(demo: String, state: Entity<AppState>, shell: Entity<Shell>, w: Win, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        if demo == "enter" {
            wait_audio(&state, cx, 20.0).await;
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::transport::PlayHere), cx));
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let a = RENDERS.load(Ordering::Relaxed);
            let ua = shell::RENDER_US.load(Ordering::Relaxed);
            shell::RENDER_MAX_US.store(0, Ordering::Relaxed);
            let (pa, pm) = shell::STATS_SNAP.with(|s| s.get());
            cx.background_executor().timer(Duration::from_millis(2000)).await;
            let b = RENDERS.load(Ordering::Relaxed);
            let ub = shell::RENDER_US.load(Ordering::Relaxed);
            let (pb, pmb) = shell::STATS_SNAP.with(|s| s.get());
            let n = (b - a).max(1);
            println!(
                "TESTE enter→play: {} renders em 2 s (≈{:.0} fps; 0 = sem frames do compositor) · render da raiz: média {:.2} ms, máx {:.2} ms · paint da grade: {} vezes, máx {:.2} ms",
                b - a,
                (b - a) as f32 / 2.0,
                (ub - ua) as f64 / n as f64 / 1000.0,
                shell::RENDER_MAX_US.load(Ordering::Relaxed) as f64 / 1000.0,
                pb - pa,
                pmb.max(pm),
            );
            println!(
                "  follow horizontal: {} · follow vertical (set_offset): {} · renders da grade: {}",
                shell::DBG_FOLLOW_H.load(Ordering::Relaxed),
                shell::DBG_FOLLOW_V.load(Ordering::Relaxed),
                shell::DBG_GRID_RENDERS.load(Ordering::Relaxed)
            );
            let _ = cx.update(|cx| cx.quit());
            return;
        }
        cx.background_executor().timer(Duration::from_millis(1500)).await;
        if demo == "play" || demo == "lane" {
            wait_audio(&state, cx, 20.0).await;
        }
        let _ = w.update(cx, |_, window, cx| {
            let _ = shell.update(cx, |s: &mut Shell, cx| match demo.as_str() {
                "palette" => {
                    s.open_palette(window, cx);
                    if let Some(p) = s.palette.as_mut() {
                        p.query = "metr".into();
                    }
                }
                "hints" => s.start_hints(HintScope::All, window, cx),
                "play" => s.toggle_play(false, cx),
                "roothints" => {
                    s.root_hints = true;
                    cx.notify();
                }
                "cmdline" => {
                    s.open_prompt(keys::modal::PromptKind::Command, window, cx);
                    if let Some(p) = s.prompt.as_mut() {
                        p.text = "bpm 140".into();
                    }
                }
                "config" => {
                    s.page = views::Page::Config;
                    s.last_key = Some(("ctrl-space".into(), Some("hints::Jump".into()), Instant::now()));
                    cx.notify();
                }
                "whichkey" => {
                    s.pending = vec![Keystroke::parse("space").unwrap()];
                    s.pending_since = Instant::now() - Duration::from_secs(1);
                    cx.notify();
                }
                "visual" | "select" => {
                    let t1 = s.song(cx).tracks.len().saturating_sub(1).min(2);
                    s.grid.update(cx, |g, cx| {
                        g.cursor = Some(Cur { m: 0, t: t1, k: 1 });
                        g.anchor = Some(Cur { m: 0, t: t1.min(1), k: 1 });
                        g.vmode = VMode::Visual;
                        g.motion(Motion::NextBeat, 2, true, cx);
                    });
                    s.count = Some(3);
                }
                "kiai" => {
                    // abre a sub-linha de kiai e a de dinâmica da 1ª linha, em insert
                    let id = s.song(cx).tracks.first().map(|t| t.id.clone());
                    s.grid.update(cx, |g, cx| {
                        if let Some(id) = id {
                            g.show_kyai.insert(id.clone());
                            g.show_dyn.insert(id);
                            g.lane = Lane::Kyai;
                            g.vmode = VMode::Insert;
                            g.cursor = Some(Cur { m: 0, t: 0, k: 0 });
                        }
                        cx.notify();
                    });
                }
                "menu" => s.grid.update(cx, |g, cx| {
                    g.menu = Some(views::grid::Menu::Row { t: 0, s: 0 });
                    cx.notify();
                }),
                "mixer" => window.dispatch_action(Box::new(keys::nav::Mixer), cx),
                "pessoas" | "people" => window.dispatch_action(Box::new(keys::nav::Pessoas), cx),
                "kit" => window.dispatch_action(Box::new(keys::nav::Kit), cx),
                // Kit com um instrumento custom novo (cartão + editores de zona); use com --read-only
                "kit-new" => {
                    window.dispatch_action(Box::new(keys::nav::Kit), cx);
                    window.dispatch_action(Box::new(views::kit::New), cx);
                }
                "esteira" => window.dispatch_action(Box::new(keys::view::Esteira), cx),
                // esteira já tocando (discos em movimento, pulso e pétalas)
                "lane" => {
                    window.dispatch_action(Box::new(keys::view::Esteira), cx);
                    s.toggle_play(false, cx);
                }
                "partitura" | "score" => window.dispatch_action(Box::new(keys::view::Partitura), cx),
                "dyn" => s.grid.update(cx, |g, cx| {
                    g.menu = Some(views::grid::Menu::Dyn { m: 0, track: None, x: 240.0, y: 80.0 });
                    g.toggle_hairpin(1, None, wadawko_core::Hairpin::Cresc, cx);
                    cx.notify();
                }),
                "simple" => s.set_vim(false, cx),
                other => eprintln!("--demo {other}: modo desconhecido"),
            });
        });
    })
    .detach();
}

/// Usado só para o compilador não reclamar de `ops` nos builds sem testes de integração.
#[allow(dead_code)]
fn _ops_is_linked(song: &wadawko_core::Song) -> usize {
    ops::measures::total_cols(song)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn parses_flags() {
        let x = parse_args(&a("--db /tmp/x.db --read-only --samples-dir /s --dark --mute --bench 7 --demo palette --selftest")).unwrap();
        assert_eq!(x.db, Some("/tmp/x.db".into()));
        assert!(x.read_only && x.mute && x.selftest);
        assert_eq!(x.samples_dir, Some("/s".into()));
        assert_eq!(x.dark, Some(true));
        assert_eq!(x.bench, Some(7));
        assert_eq!(x.demo.as_deref(), Some("palette"));
    }

    #[test]
    fn bench_number_is_optional_and_errors_are_reported() {
        assert_eq!(parse_args(&a("--bench --mute")).unwrap().bench, Some(15));
        assert_eq!(parse_args(&a("--bench")).unwrap().bench, Some(15));
        assert_eq!(parse_args(&[]).unwrap(), Args::default());
        assert!(parse_args(&a("--db")).is_err());
        assert!(parse_args(&a("--nope")).is_err());
        assert_eq!(parse_args(&a("--light")).unwrap().dark, Some(false));
    }

    #[test]
    fn open_store_honours_read_only_and_song() {
        let dir = std::env::temp_dir().join(format!("wadawko-app-main-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("sub").join("w.db");
        // cria (banco vazio ganha a música de exemplo)
        let (store, label) = open_store(&Args { db: Some(db.clone()), ..Default::default() }).unwrap();
        assert_eq!(label, "w.db");
        let title = store.song().title.clone();
        drop(store);
        let (store, _) = open_store(&Args { db: Some(db.clone()), read_only: true, song: Some(title.clone()), ..Default::default() }).unwrap();
        assert!(store.is_read_only());
        assert_eq!(store.song().title, title);
    }
}
