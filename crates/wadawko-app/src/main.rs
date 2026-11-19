//! WaDAWko — app desktop (Rust + GPUI). Fase 3 de docs/planejamento/gpui.md + trilha
//! de teclado de docs/planejamento/teclado.md. Arquitetura: ver `docs/arquitetura/app.md`.
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
use views::grid::{Cur, Lane, Layout, Motion, VMode};

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
  --demo MODO          palette | hints | whichkey | roothints | cmdline | config | visual | play | loop | enter |
                       kiai | menu | dyn | simple | mixer | pessoas | kit | kit-new | esteira | lane | partitura |
                       probe (simula mouse e teclado e imprime o efeito; use com cópia do banco) |
                       drag (anima o arrasto com o bloco balançando e sai; use com cópia do banco) |
                       playstat-grade / playstat-esteira (toca 6 s, imprime frames e tempo de render, sai)
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

/// ID do app (reverse-DNS): nome do .desktop, do ícone, do metainfo e do Flatpak.
const APP_ID: &str = "io.github.aqilputi.WaDAWko";

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
    // primeira execução: ainda não há banco no caminho padrão (o app cria um com a música de exemplo)
    let first_run = args.db.is_none() && args.migrate_from.is_none() && wadawko_store::default_db_path().is_ok_and(|p| !p.exists());
    let (store, db_label) = match open_store(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("não consegui abrir o banco: {e:#}");
            std::process::exit(1);
        }
    };
    // tema escuro é o padrão: só abre claro se o banco guardou "claro" (ou com --light)
    let dark = args.dark.unwrap_or_else(|| !matches!(store.theme(), Ok(Some(wadawko_store::Theme::Light))));
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
        if first_run {
            state.update(cx, |s, _| s.flash("bem-vindo! para trazer músicas do app web ou de outro PC: Config → importar músicas (ou Ctrl+K “importar”)"));
        }

        // samples: decodificar bloqueia (~1 s) → thread de fundo; o áudio abre quando chegar
        let samples_dir = args.samples_dir.clone();
        let st = state.clone();
        cx.spawn(async move |cx: &mut AsyncApp| {
            let t0 = Instant::now();
            let bank = cx.background_executor().spawn(async move { SampleBank::load_from(samples_dir.as_deref()) }).await;
            st.update(cx, |s, cx| match bank {
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
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui_kit::TitlebarOptions { title: Some("WaDAWko".into()), ..Default::default() }),
                // liga a janela ao .desktop e ao ícone (Wayland: app_id; X11: WM_CLASS)
                app_id: Some(APP_ID.into()),
                ..Default::default()
            },
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
        if args.demo.as_deref() == Some("probe") {
            run_probe(state.clone(), win.1.clone(), win.0, cx);
        } else if args.demo.as_deref() == Some("drag") {
            run_drag_demo(win.1.clone(), win.0, cx);
        } else if let Some(demo) = args.demo.clone() {
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
        shell.update(cx, |sh, cx| {
            sh.grid.update(cx, |g, cx| {
                g.lane = Lane::Cells;
                g.cursor = Some(Cur { m: 0, t: 0, k: 1 });
                g.set_cell(Cur { m: 0, t: 0, k: 1 }, Some(new.to_string()), cx);
            })
        });
        let read = |cx: &mut AsyncApp| state.read_with(cx, |s, _| s.song().cell(0, 0, 1).map(str::to_string));
        let after = read(cx);
        state.update(cx, |s, cx| {
            s.undo(cx);
        });
        let undone = read(cx);
        state.update(cx, |s, cx| {
            s.redo(cx);
        });
        let redone = read(cx);
        println!("TESTE edição: célula (linha 1, compasso 1, célula 2) antes={before:?} → depois={after:?} · desfazer={undone:?} · refazer={redone:?}");
        sleep(cx, 700).await; // autosave (400 ms)
        state.update(cx, |s, _| {
            let dirty_before = s.store.has_unsaved_changes();
            let r = s.flush();
            println!(
                "TESTE gravação: pendente antes do flush={dirty_before} · flush={:?} · pendente depois={} · erro={:?}",
                r.map_err(|e| format!("{e:#}")),
                s.store.has_unsaved_changes(),
                s.store.autosave_error()
            );
        });
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// `--demo probe`: simula mouse e teclado PELO CAMINHO REAL (hitboxes, foco,
/// keymap) e imprime o efeito de cada passo. Use com uma cópia do banco.
const PPQ_PROBE: f64 = 960.0;

fn run_probe(state: Entity<AppState>, shell: Entity<Shell>, w: Win, cx: &mut App) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point};
    fn wait(cx: &mut AsyncApp, ms: u64) -> impl std::future::Future<Output = ()> {
        cx.background_executor().timer(Duration::from_millis(ms))
    }
    let alt = Modifiers { alt: true, ..Default::default() };
    let none = Modifiers::default();
    cx.spawn(async move |cx: &mut AsyncApp| {
        wait(cx, 1500).await;
        // verificações: cada uma imprime OK/FALHOU; com alguma falha o processo sai com 1
        let fails = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let check = {
            let fails = fails.clone();
            move |name: &str, ok: bool| {
                println!("PROBE {} {name}", if ok { "OK" } else { "FALHOU" });
                if !ok {
                    fails.set(fails.get() + 1);
                }
            }
        };
        let report = |label: &str, cx: &mut AsyncApp| {
            let _ = w.update(cx, |_, window, cx| {
                let sh = shell.read(cx);
                let focused = window.focused(cx);
                let on_ws = focused.as_ref() == Some(&sh.focus);
                let g = sh.grid.read(cx);
                let clip = cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default();
                let toast = state.read(cx).toast.as_ref().map(|(t, _)| t.clone()).unwrap_or_default();
                println!(
                    "PROBE {label}: foco={} (workspace={on_ws}) · modo={}/{:?} · cursor={:?} · âncora={:?} · clipboard={:?} · toast={toast:?}",
                    if focused.is_some() { "sim" } else { "NENHUM" },
                    if g.mode == views::grid::Mode::Select { "selecionar" } else { "editar" },
                    g.vmode,
                    g.cursor.map(|c| (c.m, c.t, c.k)),
                    g.anchor.map(|c| (c.m, c.t, c.k)),
                    clip.chars().take(40).collect::<String>()
                );
            });
        };
        let key = |k: &'static str, cx: &mut AsyncApp| {
            let _ = w.update(cx, |_, window, cx| {
                let handled = window.dispatch_keystroke(Keystroke::parse(k).unwrap(), cx);
                println!("PROBE tecla {k}: tratada={handled}");
            });
        };
        let cell = |m: usize, t: usize, k: usize, cx: &mut AsyncApp| -> Option<Point<Pixels>> { cx.update(|cx| shell.read(cx).grid.read(cx).cell_bounds(Cur { m, t, k }).map(|b| b.center())) };
        let mouse = |ev: PlatformInput, cx: &mut AsyncApp| {
            let _ = w.update(cx, |_, window, cx| {
                window.dispatch_event(ev, cx);
            });
        };
        let down = |p, m: Modifiers| PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: p, modifiers: m, click_count: 1, first_mouse: false });
        let up = |p, m: Modifiers| PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: p, modifiers: m, click_count: 1 });
        let mv = |p, m: Modifiers| PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: Some(MouseButton::Left), modifiers: m });

        // troca o layout do editor (o probe passa pelos dois; os passos com régua usam o horizontal)
        let set_layout = |l: Layout, cx: &mut AsyncApp| {
            cx.update(|cx| shell.read(cx).grid.clone().update(cx, |g, cx| g.set_layout(l, cx)));
        };
        let vertical = |cx: &mut AsyncApp| cx.update(|cx| shell.read(cx).grid.read(cx).layout == Layout::Vertical);
        report("início", cx);
        println!("PROBE layout do editor: {}", if vertical(cx) { "vertical" } else { "horizontal" });
        // linha 2 da música aberta (use uma com notas no compasso 0 dessa linha)
        let (Some(a), Some(b)) = (cell(0, 2, 0, cx), cell(0, 2, 1, cx)) else {
            println!("PROBE sem geometria de células");
            cx.update(|cx| cx.quit());
            return;
        };
        let row = |cx: &mut AsyncApp| cx.update(|cx| (0..4).map(|k| state.read(cx).song().cell(2, 0, k).map(str::to_string).unwrap_or("·".into())).collect::<Vec<_>>().join(" "));
        let drag_state = |cx: &mut AsyncApp| {
            cx.update(|cx| {
                let g = shell.read(cx).grid.read(cx);
                (g.move_drag.map(|m| (m.started, m.clone, m.dt, m.dg)), g.ghost.len())
            })
        };
        // arrasto com passos intermediários (como a mão), sem soltar
        let sweep = |from: gpui_kit::Point<Pixels>, to: gpui_kit::Point<Pixels>, m: Modifiers, cx: &mut AsyncApp| {
            for i in 1..=6 {
                let t = i as f32 / 6.0;
                mouse(mv(gpui_kit::point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t), m), cx);
            }
        };
        let row0 = row(cx);
        println!("PROBE linha 2, compasso 0 no início: {row0}");
        // 1) clique (sem arrastar) numa NOTA: só põe o cursor, sem seleção; Ctrl+C copia a célula
        mouse(down(a, none), cx);
        mouse(up(a, none), cx);
        wait(cx, 150).await;
        report("após clique na nota", cx);
        key("ctrl-c", cx);
        wait(cx, 100).await;
        report("após ctrl-c", cx);
        let (Some(e2), Some(e3)) = (cell(0, 2, 2, cx), cell(0, 2, 3, cx)) else { return };
        // 2) arrastar a partir de célula VAZIA seleciona; Esc limpa
        mouse(down(e2, none), cx);
        mouse(mv(e3, none), cx);
        mouse(up(e3, none), cx);
        wait(cx, 150).await;
        report("após arrasto a partir de célula vazia", cx);
        key("escape", cx);
        wait(cx, 100).await;
        // 3) Alt+arrastar direto numa nota (sem seleção antes) = copiar
        mouse(down(a, alt), cx);
        sweep(a, e2, alt, cx);
        wait(cx, 120).await;
        let (md, ghost) = drag_state(cx);
        println!("PROBE durante alt+arrasto: arrasto={md:?} · células flutuando={ghost}");
        mouse(up(e2, alt), cx);
        wait(cx, 150).await;
        println!("PROBE após alt+arrasto da nota 'do' para a célula 2: {} · arrasto={:?}", row(cx), drag_state(cx));
        check("alt+arrasto copia a nota", row(cx) != row0 && drag_state(cx).0.is_none());
        key("ctrl-z", cx);
        wait(cx, 100).await;
        // 4) arrastar (sem Alt) uma nota = mover
        mouse(down(b, none), cx);
        sweep(b, e2, none, cx);
        mouse(up(e2, none), cx);
        wait(cx, 150).await;
        println!("PROBE após arrastar a nota 'ko' (célula 1) para a 2: {}", row(cx));
        key("ctrl-z", cx);
        wait(cx, 100).await;
        // 5) Esc no meio do arrasto cancela
        mouse(down(a, none), cx);
        sweep(a, e3, none, cx);
        key("escape", cx);
        mouse(up(e3, none), cx);
        wait(cx, 150).await;
        println!("PROBE Esc no meio do arrasto: {} · arrasto={:?}", row(cx), drag_state(cx));
        check("Esc no meio do arrasto não muda nada", row(cx) == row0 && drag_state(cx).0.is_none());
        // 6) soltar FORA da grade (no painel da direita) também larga o bloco
        // no painel da direita (no editor vertical, x = 900 ainda é grade)
        let outside = {
            let vw = w.update(cx, |_, window, _| f32::from(window.viewport_size().width)).unwrap_or(1600.0);
            gpui_kit::point(gpui_kit::px(vw - 140.0), gpui_kit::px(500.0))
        };
        mouse(down(b, none), cx);
        sweep(b, e2, none, cx);
        wait(cx, 120).await; // como na vida real: a janela desenha frames durante o arrasto
        mouse(mv(outside, none), cx);
        wait(cx, 60).await;
        mouse(up(outside, none), cx);
        wait(cx, 150).await;
        println!("PROBE soltar fora da grade: {} · arrasto={:?}", row(cx), drag_state(cx));
        key("ctrl-z", cx);
        wait(cx, 100).await;
        println!("PROBE linha 2 no fim (deve ser igual ao início): {}", row(cx));
        check("arrastos + Ctrl+Z voltam a linha ao início", row(cx) == row0);
        // 5) barra da seção: clicar no texto "1×" (não é botão) (des)seleciona;
        //    clicar no botão "+" das repetições NÃO
        let hint = |id: &str| keys::HINTS.with(|h| h.borrow().targets.get(id).map(|t| t.bounds));
        // barra apertada: nenhum botão pode ficar por baixo do "clonar"
        // (limpa o registro de alvos e redesenha a grade: botões que sumiram deixam registro velho)
        keys::HINTS.with(|h| h.borrow_mut().targets.clear());
        cx.update(|cx| shell.read(cx).grid.clone().update(cx, |_, cx| cx.notify()));
        wait(cx, 200).await;
        if let Some(cl) = hint("s0-clone") {
            let ids = ["s0-rep-", "s0-rep+", "s0-col", "s0-m-", "s0-m+"];
            let over: Vec<&str> = ids.iter().copied().filter(|id| hint(id).is_some_and(|b| b.right() > cl.left())).collect();
            let grid_w = cx.update(|cx| f32::from(shell.read(cx).grid.read(cx).geom.borrow().root.size.width));
            println!("PROBE barra da seção (grade {grid_w:.0} px): botões por baixo do clonar = {over:?} · clonar {:.0} px de largura", f32::from(cl.size.width));
            check("nenhum botão por baixo do clonar", over.is_empty());
        }
        // posição do texto "N×" entre − e + da seção `s`, recalculada a cada uso (a barra
        // muda de layout ao (des)selecionar)
        let txt_of = |s: usize| {
            let (rm, rp) = (hint(&format!("s{s}-rep-"))?, hint(&format!("s{s}-rep+"))?);
            Some(gpui_kit::point((rm.right() + rp.left()) / 2.0, rm.center().y))
        };
        let click = async |p: Point<Pixels>, m: Modifiers, cx: &mut AsyncApp| {
            mouse(down(p, m), cx);
            wait(cx, 60).await;
            mouse(up(p, m), cx);
            wait(cx, 150).await;
        };
        let sel_ids = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).tp.selected.clone());
        if let Some(txt) = txt_of(0) {
            let sel0 = sel_ids(cx).len();
            click(txt, none, cx).await;
            let sel1 = sel_ids(cx).len();
            println!("PROBE clique no texto 1× da barra: selecionadas {sel0} → {sel1}");
            check("clique no texto N× seleciona a seção", sel0 == 0 && sel1 == 1);
            // outra seção: clique simples troca (só ela); Ctrl+clique junta
            let ctrl = Modifiers { control: true, ..Default::default() };
            if let Some(txt1) = txt_of(1) {
                click(txt1, none, cx).await;
                let a1 = sel_ids(cx);
                if let Some(t) = txt_of(0) {
                    click(t, ctrl, cx).await;
                }
                let a2 = sel_ids(cx);
                if let Some(t) = txt_of(0) {
                    click(t, none, cx).await;
                }
                let a3 = sel_ids(cx);
                println!("PROBE seções: clique na 2ª → {} selecionada(s) · Ctrl+clique na 1ª → {} · clique simples na 1ª → {}", a1.len(), a2.len(), a3.len());
                check("seleção de seções: clique troca, Ctrl+clique junta", a1.len() == 1 && a2.len() == 2 && a3.len() == 1);
            }
            // limpa: clique na única selecionada
            if let Some(t) = txt_of(0) {
                click(t, none, cx).await;
            }
            let left = sel_ids(cx).len();
            println!("PROBE clique na única selecionada limpa: {left} selecionada(s)");
            check("clique na única seção selecionada limpa a seleção", left == 0);
            let rep = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).song().sections.as_ref().and_then(|v| v.first()).and_then(|s| s.repeat));
            if let Some(rp) = hint("s0-rep+") {
                let (r0, s0) = (rep(cx), sel_ids(cx).len());
                click(rp.center(), none, cx).await;
                let (r1, s1) = (rep(cx), sel_ids(cx).len());
                println!("PROBE clique no + (repetições): repeat {r0:?} → {r1:?} · selecionadas {s0} → {s1}");
                check("+ das repetições aumenta o ritornello sem selecionar", r1.unwrap_or(1) == r0.unwrap_or(1) + 1 && s1 == s0);
                key("ctrl-z", cx);
                wait(cx, 100).await;
            }
        }
        // moldura em cache: botão "loop" da barra de cima liga/desliga e a barra redesenha
        if let Some(p) = keys::HINTS.with(|h| h.borrow().targets.get("loop").map(|t| t.bounds.center())) {
            let l0 = cx.update(|cx| state.read(cx).tp.looping);
            let k0 = cx.update(|cx| shell.read(cx).chrome[0].read(cx).key);
            mouse(down(p, none), cx);
            wait(cx, 60).await;
            mouse(up(p, none), cx);
            wait(cx, 150).await;
            let l1 = cx.update(|cx| state.read(cx).tp.looping);
            let k1 = cx.update(|cx| shell.read(cx).chrome[0].read(cx).key);
            println!("PROBE barra de cima (cache): loop {l0} → {l1} · chave mudou: {}", k0 != k1);
            check("botão loop da barra de cima liga e a barra redesenha", l1 != l0 && k0 != k1);
            key("ctrl-z", cx); // não desfaz loop (não é edição) — só por garantia de estado
            mouse(down(p, none), cx);
            wait(cx, 60).await;
            mouse(up(p, none), cx);
            wait(cx, 150).await;
        }
        // Ctrl+Space precisa achar os botões da moldura em cache como alvos
        key("ctrl-space", cx);
        wait(cx, 200).await;
        let found = keys::HINTS.with(|h| ["hamb", "loop", "title"].iter().map(|id| h.borrow().targets.contains_key(*id)).collect::<Vec<_>>());
        println!("PROBE Ctrl+Space acha alvos da barra de cima (hamb, loop, title): {found:?}");
        check("Ctrl+Space acha os alvos da barra de cima", found.iter().all(|f| *f));
        key("escape", cx);
        wait(cx, 100).await;
        // fórmula de compasso: "+" da barra (4/4 → 5/4 no compasso do cursor), Ctrl+Z volta
        if let Some(p) = keys::HINTS.with(|h| h.borrow().targets.get("beats+").map(|t| t.bounds.center())) {
            let beats = |cx: &mut AsyncApp| {
                cx.update(|cx| {
                    let m = shell.read(cx).grid.read(cx).cursor.map_or(0, |c| c.m);
                    let song = state.read(cx).song().clone();
                    (m, song.measures[m].beats, song.measures[m].cell_count())
                })
            };
            let (m, b0, n0) = beats(cx);
            mouse(down(p, none), cx);
            wait(cx, 60).await;
            mouse(up(p, none), cx);
            wait(cx, 150).await;
            let (_, b1, n1) = beats(cx);
            let len = cx.update(|cx| state.read(cx).schedule.len_ticks / PPQ_PROBE);
            println!("PROBE compasso {}: {b0}/4 ({n0} células) → {b1}/4 ({n1} células) · duração tocada {len:.0} tempos", m + 1);
            key("ctrl-z", cx);
            wait(cx, 100).await;
            let (_, b2, _) = beats(cx);
            println!("PROBE ctrl-z na fórmula: volta a {b2}/4");
            check("+ da fórmula de compasso acrescenta um tempo e Ctrl+Z volta", (b1 == b0 + 1 || b0 == 16) && n1 >= n0 && b2 == b0);
        }
        // andamento: rallentando pela ação da paleta no compasso do cursor → a música fica
        // mais longa; Ctrl+Z volta
        {
            let secs = |cx: &mut AsyncApp| {
                cx.update(|cx| {
                    // recompila agora (a recompilação normal tem um atraso, que num
                    // compositor lento pode passar das esperas do probe)
                    state.update(cx, |s, _| s.sync_audio());
                    let st = state.read(cx);
                    let m = shell.read(cx).grid.read(cx).cursor.map_or(0, |c| c.m);
                    (m, st.song().measures.get(m).and_then(|x| x.tempo.clone()), st.schedule.seconds(st.song().tempo))
                })
            };
            let (m, t0, s0) = secs(cx);
            let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::measure::TempoRall), cx));
            wait(cx, 150).await;
            let (_, t1, s1) = secs(cx);
            println!("PROBE andamento no compasso {}: {t0:?} → {t1:?} · duração {s0:.2} s → {s1:.2} s (rall. alonga: {})", m + 1, s1 > s0);
            key("ctrl-z", cx);
            wait(cx, 100).await;
            let (_, t2, s2) = secs(cx);
            println!("PROBE ctrl-z no andamento: {t2:?} · {s2:.2} s (igual ao início: {})", (s2 - s0).abs() < 1e-6);
            check("rallentando alonga a música e Ctrl+Z volta", t1.is_some() && s1 > s0 && t2 == t0 && (s2 - s0).abs() < 1e-6);
        }
        // cor da linha (F1): ⚙ da linha do cursor abre o popover; clicar num swatch
        // grava `track.color`; Ctrl+Z volta ao que era
        {
            use wadawko_core::model::TRACK_COLORS;
            let t = cx.update(|cx| shell.read(cx).grid.read(cx).cursor.map_or(0, |c| c.t));
            let color = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).song().tracks.get(t).and_then(|x| x.color.clone()));
            let c0 = color(cx);
            let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::track::Settings), cx));
            wait(cx, 200).await;
            let cur = cx.update(|cx| state.read(cx).song().tracks.get(t).map(|x| x.color_or_default().to_string()).unwrap_or_default());
            let i = TRACK_COLORS.iter().position(|c| !c.eq_ignore_ascii_case(&cur)).unwrap_or(0);
            let sw = hint(&format!("col-{t}-{i}"));
            if let Some(b) = sw {
                click(b.center(), none, cx).await;
            }
            let c1 = color(cx);
            key("ctrl-z", cx);
            wait(cx, 100).await;
            let c2 = color(cx);
            println!("PROBE cor da linha {t}: {c0:?} → {c1:?} → ctrl-z {c2:?} · swatch {i} achado: {}", sw.is_some());
            check("⚙ → swatch muda a cor da linha e Ctrl+Z volta", sw.is_some() && c1.as_deref() == Some(TRACK_COLORS[i]) && c2 == c0);
            key("escape", cx);
            wait(cx, 100).await;
        }
        // inspector (F4): o chip "f" da dinâmica do compasso grava a dinâmica; Ctrl+Z volta
        if let Some(p) = hint("ins-dyn-f").map(|b| b.center()) {
            let dyn_of = |cx: &mut AsyncApp| {
                cx.update(|cx| {
                    let m = shell.read(cx).grid.read(cx).cursor.map_or(0, |c| c.m);
                    state.read(cx).song().measures.get(m).and_then(|x| x.dynamic)
                })
            };
            let d0 = dyn_of(cx);
            click(p, none, cx).await;
            let d1 = dyn_of(cx);
            key("ctrl-z", cx);
            wait(cx, 150).await;
            let d2 = dyn_of(cx);
            println!("PROBE inspector: dinâmica {d0:?} → clique em f → {d1:?} → ctrl-z {d2:?}");
            check("inspector: chip de dinâmica edita o compasso e Ctrl+Z volta", d1 == Some(wadawko_core::Dynamic::F) && d1 != d0 && d2 == d0);
        } else {
            println!("PROBE inspector: chip ins-dyn-f não encontrado");
            check("inspector: chip de dinâmica edita o compasso e Ctrl+Z volta", false);
        }
        // inspector: clicar no slider de volume e usar as setas; clicar na grade solta o slider
        if let Some(p) = hint("ins-vol").map(|b| b.center()) {
            let vol = |cx: &mut AsyncApp| {
                cx.update(|cx| {
                    let song = state.read(cx).song().clone();
                    let t = shell.read(cx).grid.read(cx).cursor.map_or(0, |c| c.t);
                    song.tracks.get(t).map_or(0.0, |t| t.volume.unwrap_or(wadawko_core::Track::DEFAULT_VOLUME_DB))
                })
            };
            let v0 = vol(cx);
            click(p, none, cx).await;
            let v1 = vol(cx);
            key("right", cx);
            wait(cx, 80).await;
            let v2 = vol(cx);
            // o passo da rolagem deixou a grade fora da vista: volta ao topo
            cx.update(|cx| shell.read(cx).scroll.set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0))));
            let _ = w.update(cx, |_, window, _| window.refresh());
            wait(cx, 150).await;
            let c = cell(0, 2, 0, cx).unwrap_or_default();
            let hover = |p| PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: none });
            mouse(hover(c), cx);
            wait(cx, 50).await;
            mouse(down(c, none), cx);
            wait(cx, 60).await;
            mouse(up(c, none), cx);
            wait(cx, 120).await;
            println!("PROBE inspector: célula (0,2,0) em {c:?} · slider em {p:?}");
            report("inspector: depois do clique na grade", cx);
            let k0 = cx.update(|cx| shell.read(cx).grid.read(cx).cursor.map(|c| c.k));
            // no editor vertical o tempo anda com ↓ (→ é linha/sub-coluna)
            key(if vertical(cx) { "down" } else { "right" }, cx);
            wait(cx, 80).await;
            let v3 = vol(cx);
            let k1 = cx.update(|cx| shell.read(cx).grid.read(cx).cursor.map(|c| c.k));
            for _ in 0..3 {
                if vol(cx) == v0 {
                    break;
                }
                key("ctrl-z", cx);
                wait(cx, 80).await;
            }
            let v4 = vol(cx);
            println!("PROBE inspector setas: vol {v0} → clique {v1} → → {v2} · grade → {v3} (cursor {k0:?} → {k1:?}) · desfeito {v4}");
            check("inspector: → ajusta o slider em foco; depois de clicar na grade, → move o cursor", v2 == (v1 + 1.0).min(views::mixer::VOL_MAX) && v3 == v2 && k1 != k0 && v4 == v0);
        }
        // calibração: botão do cabeçalho fixo (+ seção) com clique sintético
        if let Some(p) = keys::HINTS.with(|h| h.borrow().targets.get("addsec").map(|t| t.bounds.center())) {
            let n0 = cx.update(|cx| state.read(cx).song().sections.as_ref().map_or(0, Vec::len));
            mouse(down(p, none), cx);
            wait(cx, 60).await;
            mouse(up(p, none), cx);
            wait(cx, 150).await;
            let n1 = cx.update(|cx| state.read(cx).song().sections.as_ref().map_or(0, Vec::len));
            println!("PROBE calibração + seção: seções {n0} → {n1}");
            check("+ seção acrescenta uma seção", n1 == n0 + 1);
            key("ctrl-z", cx);
            wait(cx, 100).await;
        }
        // 6) rolar a área do editor: a barra de contexto (+ seção) deve ficar no lugar
        let hint_y = |id: &str| keys::HINTS.with(|h| h.borrow().targets.get(id).map(|t| f32::from(t.bounds.origin.y)));
        let (bar0, grid0) = (hint_y("addsec"), cell(0, 2, 0, cx).map(|p| f32::from(p.y)));
        if let Some(p) = cell(0, 2, 0, cx) {
            mouse(
                PlatformInput::Scroll(gpui_kit::ScrollEvent {
                    position: p,
                    delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(-600.0))),
                    modifiers: none,
                    touch_phase: gpui_kit::TouchPhase::Moved,
                    source: Default::default(),
                }),
                cx,
            );
        }
        wait(cx, 400).await;
        let (bar1, grid1) = (hint_y("addsec"), cell(0, 2, 0, cx).map(|p| f32::from(p.y)));
        println!("PROBE rolagem: barra y {bar0:?} → {bar1:?} · grade y {grid0:?} → {grid1:?}");
        check("rolar o editor move a grade e a barra de contexto fica no lugar", bar0.is_some() && bar0 == bar1 && grid0 != grid1);
        // trecho de loop (F2): arrastar na faixa de loop da régua do compasso 2 ao 3 cria
        // Region{1,3} e liga o loop; Alt+clique na faixa limpa
        {
            use wadawko_core::timeline::Region;
            let lane = |m: usize, cx: &mut AsyncApp| {
                cx.update(|cx| {
                    shell
                        .read(cx)
                        .grid
                        .read(cx)
                        .cell_bounds(Cur { m, t: 0, k: 0 })
                        .map(|b| gpui_kit::point(b.origin.x + b.size.width / 2.0, b.origin.y - gpui_kit::px(views::grid::TIMELINE_H) + gpui_kit::px(3.0)))
                })
            };
            let range = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).tp.range);
            let looping = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).tp.looping);
            // a janela do probe é estreita: volta o editor ao topo, aproxima o zoom e rola até
            // o compasso 2 para os dois compassos caberem na vista (fora da vista não há mouse)
            let zoom_to = |cw: f32, m: usize, cx: &mut AsyncApp| {
                cx.update(|cx| {
                    shell.read(cx).scroll.set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0)));
                    shell.read(cx).grid.clone().update(cx, |g, cx| {
                        g.cw = cw;
                        g.cw_target = cw;
                        let song = g.song.clone();
                        let s = song.section_of(m);
                        let (a, _) = song.section_range(s);
                        let x = views::grid::measure_xs(&song, s, cw).get(m - a).copied().unwrap_or(0.0) - views::grid::CELL_X0;
                        g.offset = x.max(0.0);
                        g.offset_target = g.offset;
                        cx.notify();
                    })
                })
            };
            // a faixa de loop vertical (no gutter) é da T3b: este passo roda na grade horizontal
            let was_vertical = vertical(cx);
            set_layout(Layout::Horizontal, cx);
            wait(cx, 250).await;
            zoom_to(16.0, 1, cx);
            wait(cx, 200).await;
            match (lane(1, cx), lane(2, cx)) {
                (Some(p1), Some(p2)) => {
                    cx.update(|cx| state.update(cx, |s, cx| s.set_loop(false, cx)));
                    mouse(down(p1, none), cx);
                    wait(cx, 80).await;
                    sweep(p1, p2, none, cx);
                    wait(cx, 120).await;
                    mouse(up(p2, none), cx);
                    wait(cx, 150).await;
                    let (r1, l1) = (range(cx), looping(cx));
                    // com o trecho criado a geometria muda: recalcula o ponto da faixa
                    let p1 = lane(1, cx).unwrap_or(p1);
                    click(p1, alt, cx).await;
                    let r2 = range(cx);
                    println!("PROBE faixa de loop: arrasto c2→c3 = {r1:?} (loop ligado: {l1}) · Alt+clique = {r2:?}");
                    check("arrastar na faixa de loop cria o trecho c2–c3 e liga o loop; Alt+clique limpa", r1 == Some(Region { start_bar: 1, end_bar: 3 }) && l1 && r2.is_none());
                }
                _ => println!("PROBE sem compassos 2–3 para testar a faixa de loop"),
            }
            zoom_to(views::grid::BASE_CW, 0, cx);
            if was_vertical {
                set_layout(Layout::Vertical, cx);
            }
            wait(cx, 250).await;
            // Loop: seleção — arrasto numa célula vazia seleciona no compasso 0; a ação faz
            // o trecho c1; limpar (ação da barra) volta a tocar a música inteira
            if let (Some(e2), Some(e3)) = (cell(0, 2, 2, cx), cell(0, 2, 3, cx)) {
                mouse(down(e2, none), cx);
                mouse(mv(e3, none), cx);
                mouse(up(e3, none), cx);
                wait(cx, 100).await;
                let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::transport::LoopSelection), cx));
                wait(cx, 150).await;
                let r1 = range(cx);
                let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::transport::ClearRegion), cx));
                wait(cx, 150).await;
                let r2 = range(cx);
                println!("PROBE loop da seleção: {r1:?} · limpar: {r2:?}");
                check("Loop: seleção faz o trecho do compasso selecionado e limpar volta", r1 == Some(Region { start_bar: 0, end_bar: 1 }) && r2.is_none());
                key("escape", cx);
                wait(cx, 100).await;
            }
            // tocando com o trecho c2–c3: o playhead fica dentro dele (volta no lugar certo)
            let n = cx.update(|cx| state.read(cx).song().measures.len());
            let has_audio = cx.update(|cx| state.read(cx).audio.is_some());
            if n >= 3 && has_audio {
                cx.update(|cx| state.update(cx, |s, cx| s.set_range(Some(Region { start_bar: 1, end_bar: 3 }), cx)));
                cx.update(|cx| state.update(cx, |s, cx| s.play(0, cx)));
                wait(cx, 900).await;
                let mut seen = Vec::new();
                let (mut master, mut track) = (0.0f32, 0.0f32);
                for _ in 0..6 {
                    if let Some(mi) = cx.update(|cx| state.read(cx).playhead().map(|p| p.mi)) {
                        seen.push(mi);
                    }
                    // medidores (F3): o master e alguma linha mostram nível enquanto toca
                    let (m, t) = cx.update(|cx| {
                        let st = state.read(cx);
                        (st.levels.master[0].level.max(st.levels.master[1].level), (0..st.song().tracks.len()).map(|t| st.track_level(t).level).fold(0.0, f32::max))
                    });
                    master = master.max(m);
                    track = track.max(t);
                    wait(cx, 200).await;
                }
                cx.update(|cx| state.update(cx, |s, cx| s.stop(cx)));
                let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::transport::ClearRegion), cx));
                wait(cx, 150).await;
                println!("PROBE tocando no trecho c2–c3: compassos vistos = {seen:?} · medidores: master {master:.3} · linha {track:.3}");
                check("tocando com trecho de loop o playhead fica dentro dele", !seen.is_empty() && seen.iter().all(|&m| (1..3).contains(&m)));
                check("medidores: master e linha mostram nível tocando", master > 0.0 && track > 0.0);
            } else {
                println!("PROBE (sem áudio ou música curta: pulo o teste do playhead no trecho)");
            }
        }
        // (no fim: o Tab move o cursor e a grade rola para o lado, o que mudaria as
        // posições usadas pelos passos acima)
        // Tab: digitar "do" + Tab grava e vai para a próxima célula; Tab sozinho avança
        {
            let (Some(e2), Some(_)) = (cell(0, 2, 2, cx), cell(0, 2, 3, cx)) else { return };
            mouse(down(e2, none), cx);
            mouse(up(e2, none), cx);
            wait(cx, 100).await;
            report("Tab: antes (cursor na célula vazia 2)", cx);
            key("d", cx);
            key("o", cx);
            key("tab", cx);
            wait(cx, 100).await;
            report("Tab: depois de digitar do + Tab", cx);
            println!("PROBE Tab: linha 2 = {}", row(cx));
            key("tab", cx);
            wait(cx, 100).await;
            report("Tab: depois de Tab sozinho", cx);
            key("shift-tab", cx);
            wait(cx, 100).await;
            report("Tab: depois de Shift+Tab", cx);
            key("ctrl-z", cx);
            wait(cx, 100).await;
        }
        report("antes do ctrl-k", cx);
        let _ = w.update(cx, |_, window, _cx| {
            let ctx = window.context_stack().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>().join(" > ");
            println!("PROBE contexto de teclas: {ctx}");
        });
        // metrônomo (F7): ligar mostra os chips; ÷ dá a volta 1 → 2 → 3 → 4 → 1 (o settings.json fica como estava)
        let hint_at = |id: &str| keys::HINTS.with(|h| h.borrow().targets.get(id).map(|t| t.bounds.center()));
        if let Some(p) = hint_at("metro") {
            let click = |p, cx: &mut AsyncApp| {
                mouse(down(p, none), cx);
                mouse(up(p, none), cx);
            };
            let m0 = cx.update(|cx| state.read(cx).tp.metronome);
            if !m0 {
                click(p, cx);
                wait(cx, 200).await;
            }
            let sub = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).tp.metro.subdiv);
            let d0 = sub(cx);
            let mut seen = vec![d0];
            if let Some(q) = hint_at("m-sub") {
                for _ in 0..4 {
                    click(q, cx);
                    wait(cx, 120).await;
                    seen.push(sub(cx));
                }
            }
            println!("PROBE metrônomo: chips {:?} · subdivisões {seen:?}", (hint_at("m-pat").is_some(), hint_at("m-sub").is_some()));
            check("metrônomo: chips de acento e subdivisão; ÷ dá a volta", hint_at("m-pat").is_some() && seen.len() == 5 && seen[4] == d0 && seen[1] == d0 % 4 + 1);
            // os chips empurram o 🎵 para a esquerda: posição nova
            if !m0 && let Some(p) = hint_at("metro") {
                click(p, cx);
                wait(cx, 150).await;
            }
            let (m1, d1) = cx.update(|cx| (state.read(cx).tp.metronome, state.read(cx).tp.metro.subdiv));
            check("metrônomo volta ao estado inicial (desligado/subdivisão)", m1 == m0 && d1 == d0);
        }
        // tap tempo (F8): 4 batidas a 500 ms ≈ 120 BPM; um Ctrl+Z volta o andamento
        {
            let tempo = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).song().tempo);
            let t0 = tempo(cx);
            cx.update(|cx| shell.update(cx, |s, _| s.taps.clear()));
            for _ in 0..4 {
                key("alt-t", cx);
                wait(cx, 500).await;
            }
            let t1 = tempo(cx);
            // o BPM esperado sai dos instantes reais das batidas (o loop de eventos pode atrasar)
            let expect = cx.update(|cx| shell::tap_bpm(&shell.read(cx).taps)).map(|b| b.clamp(30.0, 300.0).round());
            key("ctrl-z", cx);
            wait(cx, 120).await;
            let t2 = tempo(cx);
            println!("PROBE tap tempo: {t0} → {t1} → Ctrl+Z {t2}");
            println!("PROBE tap tempo: esperado pelas batidas {expect:?}");
            check("tap tempo marca o BPM das batidas (~120) e um Ctrl+Z volta", expect.is_some_and(|e| (e - t1).abs() <= 1.0) && (90.0..=150.0).contains(&t1) && t2 == t0);
        }
        // altura das linhas (F16): Ctrl+Alt+↓ duas vezes = 28 px; o clique ainda acerta a célula; padrão volta a 36
        {
            let h = |cx: &mut AsyncApp| cx.update(|cx| shell.read(cx).grid.read(cx).cell_bounds(Cur { m: 0, t: 2, k: 1 }).map(|b| f32::from(b.size.height)));
            cx.update(|cx| shell.read(cx).scroll.set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0))));
            // vertical: 28 → 24 (uma vez); horizontal: 36 → 28 (duas)
            let vert = vertical(cx);
            let (want, default) = if vert { (24.0, views::grid::ROW_H) } else { (28.0, views::grid::CELL_H) };
            key("ctrl-alt-down", cx);
            if !vert {
                key("ctrl-alt-down", cx);
            }
            wait(cx, 250).await;
            let h1 = h(cx);
            let mut hit = None;
            if let Some(c) = cell(0, 2, 1, cx) {
                mouse(PlatformInput::MouseMove(MouseMoveEvent { position: c, pressed_button: None, modifiers: none }), cx);
                wait(cx, 60).await;
                mouse(down(c, none), cx);
                wait(cx, 60).await;
                mouse(up(c, none), cx);
                wait(cx, 120).await;
                hit = cx.update(|cx| shell.read(cx).grid.read(cx).cursor.map(|c| (c.m, c.t, c.k)));
            }
            let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::view::RowReset), cx));
            wait(cx, 250).await;
            let h2 = h(cx);
            println!("PROBE altura das linhas: {h1:?} · clique em (0,2,1) → {hit:?} · padrão {h2:?}");
            check("altura das linhas: Ctrl+Alt+↓ encolhe, clique acerta, padrão volta", h1 == Some(want) && hit == Some((0, 2, 1)) && h2 == Some(default));
        }
        // modo projeção (F7d): F10 abre por cima de tudo; Esc fecha
        {
            key("f10", cx);
            wait(cx, 200).await;
            let open = cx.update(|cx| shell.read(cx).director.is_some());
            key("escape", cx);
            wait(cx, 200).await;
            let closed = cx.update(|cx| shell.read(cx).director.is_none());
            println!("PROBE modo projeção: abriu {open} · Esc fechou {closed}");
            check("modo projeção: F10 abre e Esc fecha", open && closed);
        }
        // efeitos por célula (tracker T8): Ctrl+Shift+R duas vezes = R3 na célula do cursor → 3 golpes; Ctrl+Z volta
        {
            let target = cx.update(|cx| {
                let song = state.read(cx).song().clone();
                // 1ª célula com nota (linha 0 em diante)
                (0..song.tracks.len()).find_map(|t| (0..song.measures.first().map_or(0, |m| m.cell_count())).find(|&k| song.cell(t, 0, k).is_some()).map(|k| Cur { m: 0, t, k }))
            });
            if let Some(c) = target {
                cx.update(|cx| shell.read(cx).grid.clone().update(cx, |g, cx| {
                    g.cursor = Some(c);
                    g.anchor = None;
                    cx.notify();
                }));
                let fx_of = |cx: &mut AsyncApp| cx.update(|cx| {
                    let song = state.read(cx).song().clone();
                    song.measures[0].fx_at(&song.tracks[c.t].id, c.k).map(str::to_string)
                });
                let count = |cx: &mut AsyncApp| cx.update(|cx| {
                    state.update(cx, |s, _| s.sync_audio());
                    let st = state.read(cx);
                    let cell = st.song().flat(0, c.k) as u32;
                    st.schedule.hits().filter(|(_, h)| h.track as usize == c.t && h.cell == cell).count()
                });
                let n0 = count(cx);
                key("ctrl-shift-r", cx);
                key("ctrl-shift-r", cx);
                wait(cx, 120).await;
                let (f1, n1) = (fx_of(cx), count(cx));
                // cada toque é um passo de desfazer
                key("ctrl-z", cx);
                key("ctrl-z", cx);
                wait(cx, 120).await;
                let (f2, n2) = (fx_of(cx), count(cx));
                println!("PROBE efeitos: célula {c:?} · {n0} golpe(s) → {f1:?} {n1} golpes → Ctrl+Z {f2:?} {n2}");
                check("efeitos: Ctrl+Shift+R ×2 = R3 (3 golpes) e Ctrl+Z ×2 volta", f1.as_deref() == Some("R3") && n1 == 3 && f2.is_none() && n2 == n0);
            }
        }
        // painel de seções (tracker T5/T6): clique seleciona, Ctrl+clique junta, agrupar e Ctrl+Z, copiar/colar e Ctrl+Z
        {
            let ids = cx.update(|cx| wadawko_core::ops::sections::section_ranges(state.read(cx).song()).into_iter().map(|r| r.id).collect::<Vec<_>>());
            // as linhas do painel são alvos `sec-{i}` (na ordem tocada)
            let row = |id: &str| {
                let i = ids.iter().position(|x| x == id)?;
                keys::HINTS.with(|h| h.borrow().targets.get(format!("sec-{i}").as_str()).map(|t| t.bounds.center()))
            };
            if ids.len() >= 2
                && let (Some(p0), Some(p1)) = (row(&ids[0]), row(&ids[1]))
            {
                let sel = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).tp.selected.clone());
                let groups = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).song().section_groups.as_ref().map_or(0, Vec::len));
                let nsec = |cx: &mut AsyncApp| cx.update(|cx| state.read(cx).song().sections().len());
                cx.update(|cx| state.update(cx, |s, _| s.tp.selected.clear()));
                click(p0, none, cx).await;
                let s1 = sel(cx);
                let ctrl = Modifiers { control: true, ..Default::default() };
                let p1 = row(&ids[1]).unwrap_or(p1);
                click(p1, ctrl, cx).await;
                let s2 = sel(cx);
                let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::section::Group), cx));
                wait(cx, 150).await;
                let g1 = groups(cx);
                key("ctrl-z", cx);
                wait(cx, 120).await;
                let g2 = groups(cx);
                // copiar a 1ª e colar depois dela
                cx.update(|cx| state.update(cx, |s, cx| s.select_section(&ids[0], false, cx)));
                let n0 = nsec(cx);
                let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::section::Copy), cx));
                wait(cx, 150).await;
                let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::section::Paste), cx));
                wait(cx, 200).await;
                let n1 = nsec(cx);
                key("ctrl-z", cx);
                wait(cx, 150).await;
                let n2 = nsec(cx);
                cx.update(|cx| state.update(cx, |s, _| s.tp.selected.clear()));
                println!("PROBE painel de seções: clique → {} sel · Ctrl+clique → {} · agrupar {g1} → Ctrl+Z {g2} · colar {n0} → {n1} → Ctrl+Z {n2}", s1.len(), s2.len());
                check("painel de seções: seleção, agrupar/desfazer, copiar/colar/desfazer", s1 == vec![ids[0].clone()] && s2.len() == 2 && g1 == 1 && g2 == 0 && n1 == n0 + 1 && n2 == n0);
            } else {
                println!("PROBE painel de seções: linhas não encontradas ({} seções)", ids.len());
                check("painel de seções: linhas visíveis", false);
            }
        }
        // a grade acompanha a seleção: Shift+↓ ×40 estende e o cursor continua visível;
        // minimap: clicar perto do fim rola o editor para baixo
        if cx.update(|cx| shell.read(cx).grid.read(cx).layout == views::grid::Layout::Vertical) {
            cx.update(|cx| shell.read(cx).scroll.set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0))));
            cx.update(|cx| shell.read(cx).grid.clone().update(cx, |g, cx| {
                g.cursor = Some(Cur { m: 0, t: 0, k: 0 });
                g.anchor = None;
                cx.notify();
            }));
            wait(cx, 200).await;
            for _ in 0..40 {
                key("shift-down", cx);
            }
            wait(cx, 300).await;
            let visible = cx.update(|cx| {
                let sh = shell.read(cx);
                let vb = sh.scroll.bounds();
                sh.grid.read(cx).cursor.and_then(|c| sh.grid.read(cx).cell_bounds(c)).map(|b| {
                    let y = f32::from(b.origin.y);
                    (y, f32::from(vb.origin.y), f32::from(vb.origin.y) + f32::from(vb.size.height))
                })
            });
            let anchor_ok = cx.update(|cx| shell.read(cx).grid.read(cx).anchor.is_some());
            println!("PROBE seleção acompanhada: cursor y/vista {visible:?} · âncora {anchor_ok}");
            check("a grade acompanha a seleção (Shift+↓ ×40 mantém o cursor visível)", anchor_ok && visible.is_some_and(|(y, t, b)| y >= t - 1.0 && y <= b));
            key("escape", cx);
            // minimap
            let mini = cx.update(|cx| shell.read(cx).mini_geom.get().bounds);
            if let Some(mb) = mini {
                cx.update(|cx| shell.read(cx).scroll.set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0))));
                wait(cx, 150).await;
                let y0 = cx.update(|cx| f32::from(shell.read(cx).scroll.offset().y));
                let k = cx.update(|cx| shell.read(cx).mini_geom.get().k);
                let content_h = cx.update(|cx| shell.read(cx).grid.read(cx).grid_height());
                // ~80 % do conteúdo, dentro do desenho do minimap
                let py = f32::from(mb.origin.y) + (content_h * k * 0.8).min(f32::from(mb.size.height) - 4.0);
                let p = gpui_kit::point(mb.center().x, gpui_kit::px(py));
                mouse(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: none }), cx);
                mouse(down(p, none), cx);
                wait(cx, 60).await;
                mouse(up(p, none), cx);
                wait(cx, 200).await;
                let y1 = cx.update(|cx| f32::from(shell.read(cx).scroll.offset().y));
                println!("PROBE minimap: rolagem {y0} → {y1} (k {k:.3})");
                check("minimap: clicar perto do fim rola o editor", y1 < y0 - 50.0);
                cx.update(|cx| shell.read(cx).scroll.set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0))));
            } else {
                check("minimap visível no editor vertical", false);
            }
        }
        // teclado do editor vertical (T4): passo 2 → "don" + Enter grava e desce 2 células; Tab troca de linha
        if cx.update(|cx| shell.read(cx).grid.read(cx).layout == views::grid::Layout::Vertical) {
            let empty = cx.update(|cx| {
                let song = state.read(cx).song().clone();
                (0..song.measures.first().map_or(0, |m| m.cell_count())).find(|&k| song.cell(0, 0, k).is_none() && k + 2 < song.mlen(0))
            });
            if let Some(k0) = empty {
                cx.update(|cx| shell.read(cx).grid.clone().update(cx, |g, cx| {
                    g.cursor = Some(Cur { m: 0, t: 0, k: k0 });
                    g.anchor = None;
                    cx.notify();
                }));
                key("alt-2", cx);
                if cx.update(|cx| shell.read(cx).vim_on) {
                    key("i", cx);
                }
                for k in ["d", "o", "n", "enter"] {
                    key(k, cx);
                }
                wait(cx, 120).await;
                let (c1, wrote) = cx.update(|cx| (shell.read(cx).grid.read(cx).cursor, state.read(cx).song().cell(0, 0, k0).map(str::to_string)));
                key("tab", cx);
                wait(cx, 80).await;
                let c2 = cx.update(|cx| shell.read(cx).grid.read(cx).cursor);
                key("ctrl-z", cx);
                key("alt-1", cx);
                key("escape", cx);
                println!("PROBE teclado vertical: gravou {wrote:?} · cursor → {c1:?} · Tab → {c2:?}");
                check("teclado vertical: passo 2 desce 2 células; Tab vai para a próxima linha", wrote.as_deref() == Some("don") && c1.is_some_and(|c| c.k == k0 + 2 && c.t == 0) && c2.is_some_and(|c| c.t == 1));
            }
        }
        // trechos de ensaio (F7b): Ctrl+Shift+L guarda o trecho com nome; clicar na lista volta o loop; Ctrl+Z desfaz
        {
            let n_spots = |cx: &mut AsyncApp| cx.update(|cx| wadawko_core::ops::practice::spots(state.read(cx).song()).to_vec());
            let r0 = wadawko_core::timeline::Region { start_bar: 0, end_bar: 2 };
            cx.update(|cx| state.update(cx, |s, cx| s.set_range(Some(r0), cx)));
            wait(cx, 100).await;
            let before = n_spots(cx).len();
            key("ctrl-shift-l", cx);
            wait(cx, 120).await;
            for k in ["e", "n", "t", "r", "a", "d", "a", "enter"] {
                key(k, cx);
            }
            wait(cx, 200).await;
            let after = n_spots(cx);
            let saved = after.last().map(|s| s.name.clone());
            cx.update(|cx| state.update(cx, |s, cx| s.set_range(None, cx)));
            wait(cx, 200).await;
            let target = format!("spot-{}", after.len().saturating_sub(1));
            if let Some(p) = keys::HINTS.with(|h| h.borrow().targets.get(target.as_str()).map(|t| t.bounds.center())) {
                mouse(down(p, none), cx);
                wait(cx, 60).await;
                mouse(up(p, none), cx);
                wait(cx, 150).await;
            }
            let r1 = cx.update(|cx| state.read(cx).tp.range);
            key("ctrl-z", cx);
            wait(cx, 120).await;
            let undone = n_spots(cx).len();
            cx.update(|cx| state.update(cx, |s, cx| s.set_range(None, cx)));
            println!("PROBE trechos: {before} → {} ({saved:?}) · clique na lista → {r1:?} · Ctrl+Z → {undone}", after.len());
            check("trechos: guardar com nome, clicar põe em loop, Ctrl+Z desfaz", after.len() == before + 1 && saved.as_deref() == Some("entrada") && r1 == Some(r0) && undone == before);
        }
        // editor vertical (T3a): clique numa célula põe o cursor; ↓ desce no tempo, → muda de
        // linha; ToggleLayout alterna (a célula continua a mesma) e volta
        {
            set_layout(Layout::Vertical, cx);
            cx.update(|cx| shell.read(cx).scroll.set_offset(gpui_kit::point(gpui_kit::px(0.0), gpui_kit::px(0.0))));
            let _ = w.update(cx, |_, window, _| window.refresh());
            wait(cx, 250).await;
            let cur = |cx: &mut AsyncApp| cx.update(|cx| shell.read(cx).grid.read(cx).cursor.map(|c| (c.m, c.t, c.k)));
            let ctx_ok = w.update(cx, |_, window, _| window.context_stack().iter().any(|c| format!("{c:?}").contains("layout=vertical"))).unwrap_or(false);
            let mut clicked = None;
            if let Some(p) = cell(0, 1, 2, cx) {
                mouse(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: none }), cx);
                wait(cx, 50).await;
                click(p, none, cx).await;
                clicked = cur(cx);
            }
            key("down", cx);
            wait(cx, 80).await;
            let after_down = cur(cx);
            key("right", cx);
            wait(cx, 80).await;
            let after_right = cur(cx);
            // bounds de células vizinhas no tempo ficam uma embaixo da outra
            let (b0, b1) = cx.update(|cx| {
                let g = shell.read(cx).grid.read(cx);
                (g.cell_bounds(Cur { m: 0, t: 1, k: 2 }), g.cell_bounds(Cur { m: 0, t: 1, k: 3 }))
            });
            let stacked = matches!((b0, b1), (Some(a), Some(b)) if a.origin.x == b.origin.x && b.origin.y > a.origin.y);
            let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::view::ToggleLayout), cx));
            wait(cx, 250).await;
            let (l1, c1) = cx.update(|cx| {
                let g = shell.read(cx).grid.read(cx);
                (g.layout, g.cursor.map(|c| (c.m, c.t, c.k)))
            });
            let _ = w.update(cx, |_, window, cx| window.dispatch_action(Box::new(keys::view::ToggleLayout), cx));
            wait(cx, 250).await;
            let (l2, c2) = cx.update(|cx| {
                let g = shell.read(cx).grid.read(cx);
                (g.layout, g.cursor.map(|c| (c.m, c.t, c.k)))
            });
            println!("PROBE editor vertical: contexto layout=vertical {ctx_ok} · clique (0,1,2) → {clicked:?} · ↓ → {after_down:?} · → → {after_right:?} · células empilhadas {stacked} · ToggleLayout → {l1:?} {c1:?} → {l2:?} {c2:?}");
            check(
                "editor vertical: clique põe o cursor; ↓ desce no tempo; ToggleLayout alterna e a célula continua a mesma",
                ctx_ok && clicked == Some((0, 1, 2)) && after_down == Some((0, 1, 3)) && after_right == Some((0, 2, 3)) && stacked && l1 == Layout::Horizontal && c1 == after_right && l2 == Layout::Vertical && c2 == after_right,
            );
        }
        key("ctrl-k", cx);
        wait(cx, 100).await;
        let pal = cx.update(|cx| shell.read(cx).palette.is_some());
        println!("PROBE ctrl-k abriu a paleta: {pal}");
        let sel = |cx: &mut AsyncApp| cx.update(|cx| shell.read(cx).palette.as_ref().map(|p| p.sel));
        let s0 = sel(cx);
        key("tab", cx);
        wait(cx, 100).await;
        let s1 = sel(cx);
        println!("PROBE Tab na paleta desce a seleção: {s0:?} → {s1:?}");
        check("Ctrl+K abre a paleta e Tab desce a seleção", pal && s1.is_some() && s1 != s0);
        key("escape", cx);
        wait(cx, 100).await;
        let n = fails.get();
        if n == 0 {
            println!("PROBE tudo OK");
            cx.update(|cx| cx.quit());
        } else {
            println!("PROBE {n} verificação(ões) FALHARAM");
            cx.update(|cx| state.update(cx, |s, _| s.flush().ok()));
            std::process::exit(1);
        }
    })
    .detach();
}

/// `--demo drag`: pega duas notas com Alt, varre para a direita em
/// ~1,5 s e fica parado 2,5 s (o bloco balança e assenta); depois solta e sai.
/// Para ver/gravar a animação. Use com uma cópia do banco.
fn run_drag_demo(shell: Entity<Shell>, w: Win, cx: &mut App) {
    use gpui_kit::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, point, px};
    cx.spawn(async move |cx: &mut AsyncApp| {
        cx.background_executor().timer(Duration::from_millis(1500)).await;
        let cell = |m, t, k, cx: &mut AsyncApp| cx.update(|cx| shell.read(cx).grid.read(cx).cell_bounds(Cur { m, t, k }).map(|b| b.center()));
        let (Some(a), Some(b)) = (cell(0, 2, 0, cx), cell(0, 2, 1, cx)) else {
            cx.update(|cx| cx.quit());
            return;
        };
        let alt = Modifiers { alt: true, ..Default::default() };
        let send = |ev: PlatformInput, cx: &mut AsyncApp| {
            let _ = w.update(cx, |_, window, cx| {
                window.dispatch_event(ev, cx);
            });
        };
        let mv = |p, cx: &mut AsyncApp| send(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: Some(MouseButton::Left), modifiers: alt }), cx);
        // seleciona as duas notas (arrasto a partir da 1ª com Shift = estende) e pega com Alt
        send(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: a, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }), cx);
        send(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: a, modifiers: Modifiers::default(), click_count: 1 }), cx);
        send(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: b, modifiers: Modifiers { shift: true, ..Default::default() }, click_count: 1, first_mouse: false }), cx);
        send(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: b, modifiers: Modifiers { shift: true, ..Default::default() }, click_count: 1 }), cx);
        cx.background_executor().timer(Duration::from_millis(200)).await;
        send(PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: a, modifiers: alt, click_count: 1, first_mouse: false }), cx);
        let frames = || (RENDERS.load(Ordering::Relaxed), shell::RENDER_US.load(Ordering::Relaxed));
        let (r0, u0) = frames();
        let t0 = Instant::now();
        println!("DEMO pegou");
        let n = 90;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let e = t * t * (3.0 - 2.0 * t); // acelera e freia
            mv(point(a.x + px(280.0 * e), a.y + px(70.0 * e)), cx);
            cx.background_executor().timer(Duration::from_millis(16)).await;
        }
        let (r1, u1) = frames();
        let t1 = Instant::now();
        println!("DEMO parou · movendo: {} frames em {:.1} s · render() média {:.2} ms", r1 - r0, (t1 - t0).as_secs_f32(), (u1 - u0) as f64 / (r1 - r0).max(1) as f64 / 1000.0);
        // parado de verdade: nenhum evento de mouse (como a mão parada)
        let end = point(a.x + px(280.0), a.y + px(70.0));
        cx.background_executor().timer(Duration::from_millis(2400)).await;
        send(PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: end, modifiers: alt, click_count: 1 }), cx);
        let (r2, u2) = frames();
        println!("DEMO soltou · parado: {} frames em {:.1} s · render() média {:.2} ms", r2 - r1, t1.elapsed().as_secs_f32(), (u2 - u1) as f64 / (r2 - r1).max(1) as f64 / 1000.0);
        cx.background_executor().timer(Duration::from_millis(600)).await;
        cx.update(|cx| cx.quit());
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
            cx.update(|cx| cx.quit());
            return;
        }
        // `hoverstat`: parado, passa o mouse 200× alternando entre botões da grade (★ das
        // linhas) e mede quantas vezes a grade re-renderiza e quanto custa
        if demo == "hoverstat" {
            use gpui_kit::{Modifiers, MouseMoveEvent, PlatformInput};
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            // controle: notificar só a grade, sem mudar nada
            let sn2 = shell::SEC_RENDERS.load(Ordering::Relaxed);
            let g2 = shell::DBG_GRID_RENDERS.load(Ordering::Relaxed);
            for _ in 0..20 {
                cx.update(|cx| shell.read(cx).grid.clone().update(cx, |_, cx| cx.notify()));
                cx.background_executor().timer(Duration::from_millis(60)).await;
            }
            println!("STAT controle: 20 notify da grade → {} renders da grade, {} de seção", shell::DBG_GRID_RENDERS.load(Ordering::Relaxed) - g2, shell::SEC_RENDERS.load(Ordering::Relaxed) - sn2);
            let mut found: Vec<(String, gpui_kit::Point<gpui_kit::Pixels>)> =
                keys::HINTS.with(|h| h.borrow().targets.iter().filter(|(k, _)| k.ends_with("star") && !k.starts_with("ins")).map(|(k, t)| (k.to_string(), t.bounds.center())).collect());
            found.sort_by(|a, b| a.0.cmp(&b.0));
            println!("STAT alvos: {:?}", found.iter().take(2).collect::<Vec<_>>());
            let mut pts: Vec<_> = found.into_iter().map(|(_, p)| p).collect();
            pts.truncate(2);
            if pts.len() < 2 {
                println!("STAT hover: menos de 2 botões ★ na vista");
                cx.update(|cx| cx.quit());
                return;
            }
            // ponto neutro entre os dois (célula/rótulo sem hover próprio)
            let (g0, u0, r0) = (shell::DBG_GRID_RENDERS.load(Ordering::Relaxed), shell::GRID_RENDER_US.load(Ordering::Relaxed), RENDERS.load(Ordering::Relaxed));
            let (sn0, su0) = (shell::SEC_RENDERS.load(Ordering::Relaxed), shell::SEC_RENDER_US.load(Ordering::Relaxed));
            let (ch0, chu0) = (views::grid::COL_HEAD_RENDERS.load(Ordering::Relaxed), views::grid::COL_HEAD_US.load(Ordering::Relaxed));
            for i in 0..200 {
                let p = pts[i % 2];
                let _ = w.update(cx, |_, window, cx| {
                    window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: Modifiers::default() }), cx);
                });
                cx.background_executor().timer(Duration::from_millis(20)).await;
            }
            let (g1, u1, r1) = (shell::DBG_GRID_RENDERS.load(Ordering::Relaxed), shell::GRID_RENDER_US.load(Ordering::Relaxed), RENDERS.load(Ordering::Relaxed));
            let n = (g1 - g0).max(1);
            let (sn1, su1) = (shell::SEC_RENDERS.load(Ordering::Relaxed), shell::SEC_RENDER_US.load(Ordering::Relaxed));
            println!("STAT hover: {} renders de seção (média {:.2} ms)", sn1 - sn0, (su1 - su0) as f64 / (sn1 - sn0).max(1) as f64 / 1000.0);
            let (ch1, chu1) = (views::grid::COL_HEAD_RENDERS.load(Ordering::Relaxed), views::grid::COL_HEAD_US.load(Ordering::Relaxed));
            println!("STAT hover: {} renders do cabeçalho de colunas (editor vertical; média {:.2} ms)", ch1 - ch0, (chu1 - chu0) as f64 / (ch1 - ch0).max(1) as f64 / 1000.0);
            println!(
                "STAT hover: 200 movimentos · {} renders da grade (média {:.2} ms, total {:.1} ms) · {} renders da raiz",
                g1 - g0,
                (u1 - u0) as f64 / n as f64 / 1000.0,
                (u1 - u0) as f64 / 1000.0,
                r1 - r0
            );
            cx.update(|cx| cx.quit());
            return;
        }
        // `playstat-grade` / `playstat-esteira`: toca, mede 6 s (frames + render) e sai
        if let Some(tab) = demo.strip_prefix("playstat-") {
            wait_audio(&state, cx, 20.0).await;
            let _ = w.update(cx, |_, window, cx| {
                if tab == "esteira" {
                    window.dispatch_action(Box::new(keys::view::Esteira), cx);
                }
                // `grade` = grade horizontal (linha de base); `tracker` = editor vertical
                // `centro` = editor vertical com o seguir centrado (T3b)
                let layout = if tab == "tracker" || tab == "centro" { Layout::Vertical } else { Layout::Horizontal };
                shell.read(cx).grid.clone().update(cx, |g, cx| g.set_layout(layout, cx));
                shell.update(cx, |s: &mut Shell, _| s.follow_center = tab == "centro");
                shell.update(cx, |s: &mut Shell, cx| s.toggle_play(false, cx));
            });
            cx.background_executor().timer(Duration::from_millis(2000)).await;
            let (r0, u0, t0) = (RENDERS.load(Ordering::Relaxed), shell::RENDER_US.load(Ordering::Relaxed), Instant::now());
            let (sn0, su0) = (shell::SEC_RENDERS.load(Ordering::Relaxed), shell::SEC_RENDER_US.load(Ordering::Relaxed));
            println!("STAT início");
            cx.background_executor().timer(Duration::from_millis(6000)).await;
            let (r1, u1, secs) = (RENDERS.load(Ordering::Relaxed), shell::RENDER_US.load(Ordering::Relaxed), t0.elapsed().as_secs_f32());
            println!("STAT fim · {} frames em {secs:.1} s ({:.0} fps) · Shell::render média {:.2} ms", r1 - r0, (r1 - r0) as f32 / secs, (u1 - u0) as f64 / (r1 - r0).max(1) as f64 / 1000.0);
            let (sn1, su1) = (shell::SEC_RENDERS.load(Ordering::Relaxed), shell::SEC_RENDER_US.load(Ordering::Relaxed));
            println!("STAT seções · {} renders ({:.1} por frame) · {:.2} ms por frame", sn1 - sn0, (sn1 - sn0) as f64 / (r1 - r0).max(1) as f64, (su1 - su0) as f64 / (r1 - r0).max(1) as f64 / 1000.0);
            cx.update(|cx| cx.quit());
            return;
        }
        cx.background_executor().timer(Duration::from_millis(1500)).await;
        if demo == "play" || demo == "lane" || demo == "director" {
            wait_audio(&state, cx, 20.0).await;
        }
        let _ = w.update(cx, |_, window, cx| {
            shell.update(cx, |s: &mut Shell, cx| match demo.as_str() {
                "palette" => {
                    s.open_palette(window, cx);
                    if let Some(p) = s.palette.as_mut() {
                        p.query = "metr".into();
                    }
                }
                "hints" => s.start_hints(HintScope::All, window, cx),
                "play" => s.toggle_play(false, cx),
                // painel de seções com grupo e cores + efeitos numa célula (tracker)
                "estrutura" => {
                    let ids: Vec<String> = wadawko_core::ops::sections::section_ranges(s.state.read(cx).song()).into_iter().map(|r| r.id).collect();
                    s.edit(cx, |song| {
                        if ids.len() >= 3 {
                            if let Some(g) = wadawko_core::ops::structure::group_sections(song, &ids[1..3], "Parte A") {
                                wadawko_core::ops::structure::set_group_color(song, &g, Some("#5e9fd8"));
                            }
                            wadawko_core::ops::structure::set_section_color(song, &ids[0], Some("#d9775f"));
                        }
                        let t = song.tracks[0].id.clone();
                        let _ = wadawko_core::ops::cells::set_fx(song, &t, 0, 0, Some("R4<"));
                        let _ = wadawko_core::ops::cells::set_fx(song, &t, 0, 2, Some("F D+1"));
                    });
                    s.state.update(cx, |st, cx| st.select_section(&ids[1], false, cx));
                }
                // modo projeção tocando (F7d)
                "director" => {
                    s.director = Some(false);
                    s.toggle_play(false, cx);
                }
                // metrônomo ligado: chips de acento, subdivisão e som na barra de cima
                "metro" => {
                    s.state.update(cx, |st, cx| {
                        st.set_metronome(true, cx);
                        st.tp.looping = true;
                    });
                    // tocando: o compositor sem tela só entrega frames com animação
                    s.toggle_play(false, cx);
                }
                // trecho de loop c2–c3 (F2) tocando: banda na régua, chip na barra, ↺ na status bar
                "loop" => {
                    s.state.update(cx, |st, cx| st.set_range(Some(wadawko_core::timeline::Region { start_bar: 1, end_bar: 3 }), cx));
                    s.toggle_play(false, cx);
                }
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
                "tempo" => s.grid.update(cx, |g, cx| {
                    // rall. de −20 BPM ao longo dos compassos 1–2 e o popover aberto
                    let bpm = g.song.tempo;
                    g.set_tempo(0, Some(wadawko_core::TempoMark { target: bpm - 20.0, span: 2 }), cx);
                    g.menu = Some(views::grid::Menu::Tempo { m: 0, x: 300.0, y: 80.0 });
                    cx.notify();
                }),
                "mixer" => window.dispatch_action(Box::new(keys::nav::Mixer), cx),
                // mixer tocando (medidores, master com nível)
                "mixer-play" => {
                    window.dispatch_action(Box::new(keys::nav::Mixer), cx);
                    s.toggle_play(false, cx);
                }
                "pessoas" | "people" => window.dispatch_action(Box::new(keys::nav::Pessoas), cx),
                "kit" => window.dispatch_action(Box::new(keys::nav::Kit), cx),
                "probe" => {}
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
