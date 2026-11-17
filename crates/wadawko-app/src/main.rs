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

        report("início", cx);
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
        let outside = gpui_kit::point(gpui_kit::px(900.0), gpui_kit::px(500.0));
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
        // `playstat-grade` / `playstat-esteira`: toca, mede 6 s (frames + render) e sai
        if let Some(tab) = demo.strip_prefix("playstat-") {
            wait_audio(&state, cx, 20.0).await;
            let _ = w.update(cx, |_, window, cx| {
                if tab == "esteira" {
                    window.dispatch_action(Box::new(keys::view::Esteira), cx);
                }
                shell.update(cx, |s: &mut Shell, cx| s.toggle_play(false, cx));
            });
            cx.background_executor().timer(Duration::from_millis(2000)).await;
            let (r0, u0, t0) = (RENDERS.load(Ordering::Relaxed), shell::RENDER_US.load(Ordering::Relaxed), Instant::now());
            println!("STAT início");
            cx.background_executor().timer(Duration::from_millis(6000)).await;
            let (r1, u1, secs) = (RENDERS.load(Ordering::Relaxed), shell::RENDER_US.load(Ordering::Relaxed), t0.elapsed().as_secs_f32());
            println!("STAT fim · {} frames em {secs:.1} s ({:.0} fps) · Shell::render média {:.2} ms", r1 - r0, (r1 - r0) as f32 / secs, (u1 - u0) as f64 / (r1 - r0).max(1) as f64 / 1000.0);
            cx.update(|cx| cx.quit());
            return;
        }
        cx.background_executor().timer(Duration::from_millis(1500)).await;
        if demo == "play" || demo == "lane" {
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
