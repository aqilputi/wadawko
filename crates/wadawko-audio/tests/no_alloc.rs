//! O caminho de tempo real (`drain_commands` + `publish_clock` + `process`)
//! não aloca nem desaloca: um allocator contador vigia a thread do "callback"
//! enquanto ela toca a música, recebe schedules novos, previews e comandos.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use common::{bank, big_song};
use wadawko_audio::{CompileOptions, EngineConfig, EngineEvent, MasterMode, PlayOptions, Region, SampleSet, TrainerOptions, engine_pair};
use wadawko_core::demo::demo_song;
use wadawko_core::model::BUILTIN_INSTRUMENTS;
use wadawko_core::ops::cells::{set_cell, set_kyai};

struct Counting;

thread_local! {
    static WATCH: Cell<bool> = const { Cell::new(false) };
}
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCH.try_with(Cell::get).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WATCH.try_with(Cell::get).unwrap_or(false) {
            FREES.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if WATCH.try_with(Cell::get).unwrap_or(false) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn realtime_path_does_not_allocate() {
    let samples = SampleSet::from_bank(bank());
    let (mut eng, mut ctl) = engine_pair(bank(), samples, EngineConfig { sample_rate: 48_000, seed: 3, master: MasterMode::Live, metronome: true });
    let mut out = vec![0f32; 512 * 2];
    // o "callback": só isto é vigiado
    let mut callback = |eng: &mut wadawko_audio::Engine, blocks: usize| {
        WATCH.with(|w| w.set(true));
        for _ in 0..blocks {
            eng.drain_commands();
            eng.publish_clock(0);
            eng.process(&mut out, 2);
        }
        WATCH.with(|w| w.set(false));
    };

    // música com kiai (grãos + ruído), ritornello em loop, treino e count-in
    let mut song = demo_song();
    for (ci, syl) in ["so", "re", "so", "re!", "sei", "ya!"].into_iter().enumerate() {
        set_kyai(&mut song, "chu1", 0, ci, Some(syl));
    }
    let region = CompileOptions { region: Some(Region { start_bar: 0, end_bar: 2 }), ..Default::default() };
    ctl.set_song(&song, &region);
    ctl.play(PlayOptions { looping: true, count_in: true, trainer: Some(TrainerOptions { start_pct: 0.6, step: 0.2 }), ..Default::default() });
    callback(&mut eng, 1500); // 16 s: contagem + várias voltas com o BPM subindo

    // edição durante o playback: schedules novos chegam e os velhos voltam pela fila de lixo
    for i in 0..20 {
        set_cell(&mut song, "odaiko", 1, i % 8, Some(if i % 2 == 0 { "DON" } else { "kara" }), None);
        ctl.set_song(&song, &region);
        ctl.apply_mix(&song);
        callback(&mut eng, 20);
    }

    // previews, loop de frase, seek, metrônomo, mix, easter egg, panic
    let def = BUILTIN_INSTRUMENTS.iter().find(|d| d.id == "preset-nagado-anelado").unwrap();
    ctl.preview_cell(&song, &song.tracks[0], "teke");
    ctl.preview_instrument(def, wadawko_core::Zone::Head);
    ctl.preview_kyai(&song.tracks[1], &["so", "ri", "ya!"], 0.2, None);
    ctl.start_phrase_loop(def, &["don", "doko", "kara"], 0.12, 0.5);
    ctl.play_fue();
    callback(&mut eng, 300);
    ctl.stop_phrase_loop();
    ctl.start_phrase_loop(def, &["DON", "tsuku"], 0.1, 0.0);
    ctl.seek_cell(9);
    ctl.set_metronome(false);
    ctl.set_bpm(137.0);
    ctl.set_master_gain(0.5);
    callback(&mut eng, 200);
    ctl.stop_phrase_loop();
    ctl.panic();
    ctl.stop();
    callback(&mut eng, 50);

    // a música grande das fixtures, sem loop, até o fim (muitas vozes simultâneas)
    let big = big_song();
    ctl.set_song(&big, &CompileOptions::default());
    ctl.play(PlayOptions::default());
    callback(&mut eng, 2000);

    assert_eq!((ALLOCS.load(Ordering::Relaxed), FREES.load(Ordering::Relaxed)), (0, 0), "alocações/liberações dentro do callback");

    // e o caminho vigiado realmente trabalhou
    let events = ctl.poll_events();
    assert!(events.iter().filter(|e| matches!(e, EngineEvent::HitFired { .. })).count() > 200);
    assert!(events.iter().any(|e| matches!(e, EngineEvent::LoopWrapped { .. })));
    assert!(events.iter().any(|e| matches!(e, EngineEvent::BpmChanged { .. })));
    assert!(eng.frames() > 48_000 * 40);
    // sanidade do próprio contador
    WATCH.with(|w| w.set(true));
    let v = std::hint::black_box(vec![1u8; 100]);
    drop(v);
    WATCH.with(|w| w.set(false));
    assert!(ALLOCS.load(Ordering::Relaxed) >= 1 && FREES.load(Ordering::Relaxed) >= 1);
}
