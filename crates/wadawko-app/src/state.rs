//! Estado compartilhado do app: UMA entidade GPUI (`Entity<AppState>`) que é
//! dona do `ProjectStore` (banco + autosave), do histórico (desfazer/refazer),
//! do áudio e do estado do transporte. Todas as views recebem um clone do
//! `Entity<AppState>` — ver `docs/arquitetura/app.md`.
//!
//! Regra de ouro: NINGUÉM muda a música fora de [`AppState::edit`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::{Context, EventEmitter};
use wadawko_audio::{Audio, AudioConfig, CompileOptions, EngineEvent, Metronome, PPQ, PlayOptions, Schedule, TrainerOptions};
use wadawko_core::Song;
use wadawko_core::ops::sections::selected_region;
use wadawko_core::timeline::Region;
use wadawko_export::{ExportJob, ExportOptions, ExportStage};
use wadawko_samples::SampleBank;
use wadawko_store::{ProjectStore, SongHistory};

use crate::song_ext::SongExt;
use crate::theme::Theme;

/// O que mudou (para quem assina com `cx.subscribe(&state, …)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StateEvent {
    /// a música atual foi editada (inclui desfazer/refazer)
    Edited,
    /// outra música passou a ser a atual: zere cursor/seleção/rolagem
    SongReplaced,
}

/// O que se ouve no modo "ensaiar como…" (ux-pesquisa #1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Listen {
    /// a música sem a pessoa (ela toca junto)
    #[default]
    Without,
    /// só a pessoa
    Only,
}

impl Listen {
    pub fn label(self) -> &'static str {
        match self {
            Listen::Without => "sem",
            Listen::Only => "só",
        }
    }
}

/// Modo "ensaiar como…": estado de transporte (não vai para o arquivo nem para o desfazer).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Rehearse {
    pub person: String,
    pub listen: Listen,
}

/// Cópia da música para o áudio com o modo de ensaio aplicado (mudo/solo da pessoa).
pub fn rehearsal_song(song: &Song, r: Option<&Rehearse>) -> Song {
    let mut s = song.clone();
    let Some(r) = r else { return s };
    if let Some(people) = s.people.as_mut() {
        for p in people.iter_mut() {
            let me = p.id == r.person;
            match r.listen {
                Listen::Without if me => p.muted = Some(true),
                Listen::Only => {
                    p.solo = me.then_some(true);
                    if me {
                        p.muted = None;
                    }
                }
                _ => {}
            }
        }
    }
    s
}

/// Opções do transporte (não fazem parte da música).
#[derive(Clone, Debug, PartialEq)]
pub struct Transport {
    pub looping: bool,
    pub metronome: bool,
    /// padrão de acentos, subdivisão e volume do metrônomo (F7; salvo no settings.json)
    pub metro: Metronome,
    pub count_in: bool,
    /// count-in a cada volta do loop (F7c; salvo no settings.json)
    pub count_in_loop: bool,
    pub trainer: bool,
    /// treino: fração do andamento na 1ª volta (0.2..=1.0)
    pub trainer_pct: f64,
    /// treino: quanto sobe por volta (0.01..=0.5)
    pub trainer_step: f64,
    /// ids das seções selecionadas para tocar (vazio = música inteira)
    pub selected: Vec<String>,
    /// trecho de loop em compassos `[start_bar, end_bar)` (F2): arraste na
    /// régua, `Loop: seleção`, `[`/`]`. Tem precedência sobre `selected`.
    pub range: Option<Region>,
    /// "ensaiar como…": a pessoa e o que se ouve (sem ela / só ela)
    pub rehearse: Option<Rehearse>,
}

impl Default for Transport {
    fn default() -> Self {
        Transport {
            looping: true,
            metronome: false,
            metro: Metronome::default(),
            count_in: false,
            count_in_loop: false,
            trainer: false,
            trainer_pct: 0.7,
            trainer_step: 0.05,
            selected: Vec::new(),
            range: None,
            rehearse: None,
        }
    }
}

/// Região que o engine toca: o trecho de loop (`range`), senão as seções
/// selecionadas, senão a música inteira (`None`).
pub fn effective_region(range: Option<Region>, selected: Option<Region>) -> Option<Region> {
    range.or(selected)
}

/// Prende um trecho ao número de compassos da música; vazio = `None`.
pub fn clamp_region(r: Region, measures: usize) -> Option<Region> {
    let end = r.end_bar.min(measures);
    (r.start_bar < end).then_some(Region { start_bar: r.start_bar, end_bar: end })
}

/// Rótulo do trecho: `c3–c6` (1-based, fim inclusivo) ou `c3` para um compasso.
pub fn range_label(r: Region) -> String {
    if r.end_bar <= r.start_bar + 1 { format!("c{}", r.start_bar + 1) } else { format!("c{}–c{}", r.start_bar + 1, r.end_bar) }
}

/// Golpe disparado pelo engine (`EngineEvent::HitFired`), com o instante em que
/// ele fica AUDÍVEL — é o que as views usam para piscar a nota.
#[derive(Clone, Copy, Debug)]
pub struct HitFlash {
    pub track: usize,
    /// célula-fonte global
    pub cell: usize,
    /// índice do golpe dentro da célula
    pub hit: usize,
    pub at: Instant,
}

/// Posição do playhead na música como escrita.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playhead {
    /// compasso-fonte
    pub mi: usize,
    /// fração (0..1) dentro do compasso
    pub frac: f32,
    /// célula-fonte global sob o cursor
    pub cell: Option<usize>,
    pub tick: f64,
}

pub struct ExportRun {
    pub job: ExportJob,
    pub path: PathBuf,
    pub started: Instant,
}

/// Um medidor na UI (F3): nível com queda de 36 dB/s e pico retido por 1,5 s
/// que depois cai a 12 dB/s (docs/planejamento/daw.md §2.5).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    pub level: f32,
    pub peak: f32,
    /// quando o pico foi marcado (segundos desde um instante de referência)
    pub peak_at: f32,
}

impl Level {
    /// Aplica um novo pico lido do engine (`raw`, linear) `dt` segundos depois
    /// da última leitura, em `now` segundos.
    pub fn feed(&mut self, raw: f32, dt: f32, now: f32) {
        let fall = 10f32.powf(-36.0 * dt / 20.0);
        self.level = (self.level * fall).max(raw);
        if raw >= self.peak {
            self.peak = raw;
            self.peak_at = now;
        } else if now - self.peak_at > 1.5 {
            self.peak *= 10f32.powf(-12.0 * dt / 20.0);
        }
        if self.level < 1e-3 {
            self.level = 0.0;
        }
        if self.peak < 1e-3 {
            self.peak = 0.0;
        }
    }

    pub fn is_silent(&self) -> bool {
        self.level <= 0.0 && self.peak <= 0.0
    }
}

/// Medidores da UI por canal (slot do schedule), barramento (slot) e master L/R.
#[derive(Clone, Debug)]
pub struct Levels {
    pub chan: Vec<Level>,
    pub bus: Vec<Level>,
    pub master: [Level; 2],
    /// quando o LED de clip acendeu
    pub clip_at: Option<Instant>,
    epoch: Instant,
    last: f32,
}

impl Default for Levels {
    fn default() -> Self {
        let now = Instant::now();
        Levels { chan: vec![Level::default(); wadawko_audio::MAX_CHANNELS], bus: vec![Level::default(); wadawko_audio::MAX_BUSES], master: [Level::default(); 2], clip_at: None, epoch: now, last: 0.0 }
    }
}

impl Levels {
    /// Lê os picos publicados pelo engine e aplica a balística.
    pub fn poll(&mut self, meters: &wadawko_audio::Meters) {
        let now = self.epoch.elapsed().as_secs_f32();
        let dt = (now - self.last).clamp(0.0, 0.5);
        self.last = now;
        for (i, l) in self.chan.iter_mut().enumerate() {
            l.feed(wadawko_audio::Meters::take(&meters.chan[i]), dt, now);
        }
        for (i, l) in self.bus.iter_mut().enumerate() {
            l.feed(wadawko_audio::Meters::take(&meters.bus[i]), dt, now);
        }
        for (i, l) in self.master.iter_mut().enumerate() {
            l.feed(wadawko_audio::Meters::take(&meters.master[i]), dt, now);
        }
        if meters.take_clip() {
            self.clip_at = Some(Instant::now());
        }
        if self.clip_at.is_some_and(|t| t.elapsed() > Duration::from_secs(1)) {
            self.clip_at = None;
        }
    }

    /// Algum medidor ainda mostra nível (precisa de frames para cair)?
    pub fn active(&self) -> bool {
        self.clip_at.is_some() || self.master.iter().any(|l| !l.is_silent()) || self.chan.iter().any(|l| !l.is_silent()) || self.bus.iter().any(|l| !l.is_silent())
    }

    pub fn clip(&self) -> bool {
        self.clip_at.is_some()
    }
}

pub struct AppState {
    pub store: ProjectStore,
    pub history: SongHistory,
    /// `None` até o banco de samples carregar (ou se não há dispositivo de saída)
    pub audio: Option<Audio>,
    pub bank: Option<SampleBank>,
    pub audio_error: Option<String>,
    /// último schedule compilado (o `TimeMap` dele dá playhead/cursor)
    pub schedule: Arc<Schedule>,
    pub theme: Theme,
    pub tp: Transport,
    /// "caneta": pessoa que assina as notas escritas em linha compartilhada
    pub pen: Option<String>,
    pub mute: bool,
    /// calibração visual × som (ms)
    pub av_offset_ms: f32,
    /// muda a cada edição/troca de música (chave barata de cache para as views)
    pub revision: u64,
    /// andamento que está soando (≠ `song.tempo` durante o treino)
    pub playback_bpm: f32,
    /// BPM que está soando AGORA, arredondado: o do treino × o mapa de andamento
    /// (rall./accel.). É o que a barra de cima mostra.
    pub sounding_bpm: f32,
    pub hits: Vec<HitFlash>,
    /// medidores de nível (F3), com balística aplicada em `poll`
    pub levels: Levels,
    /// volume de saída (dB) do strip master do mixer — não faz parte da música
    pub master_db: f64,
    pub toast: Option<(String, Instant)>,
    pub export: Option<ExportRun>,
    pub db_label: String,
    /// célula-fonte global de onde o play começa (clique na régua)
    pub start_cell: usize,
    want_play: bool,
    play_cmd_at: Instant,
    audio_stale: bool,
}

impl EventEmitter<StateEvent> for AppState {}

/// Núcleo puro de uma edição (testável sem janela): clona a música, roda `f`
/// e, se algo mudou, registra o estado anterior no histórico e devolve a nova.
pub fn apply_edit(current: &Arc<Song>, history: &mut SongHistory, tag: Option<u64>, f: impl FnOnce(&mut Song)) -> Option<Arc<Song>> {
    let mut song = (**current).clone();
    f(&mut song);
    if song == **current {
        return None;
    }
    match tag {
        Some(t) => {
            history.record_coalesced(current.clone(), t);
        }
        None => history.record(current.clone()),
    }
    Some(Arc::new(song))
}

impl AppState {
    pub fn new(store: ProjectStore, theme: Theme, mute: bool, db_label: String) -> Self {
        let bpm = store.song().tempo as f32;
        let mut history = SongHistory::new();
        history.sync_epoch(store.epoch());
        AppState {
            store,
            history,
            audio: None,
            bank: None,
            audio_error: None,
            schedule: Arc::new(Schedule::empty()),
            theme,
            tp: Transport::default(),
            pen: None,
            mute,
            av_offset_ms: 0.0,
            revision: 0,
            playback_bpm: bpm,
            sounding_bpm: bpm,
            hits: Vec::new(),
            levels: Levels::default(),
            master_db: 0.0,
            toast: None,
            export: None,
            db_label,
            start_cell: 0,
            want_play: false,
            play_cmd_at: Instant::now() - Duration::from_secs(1),
            audio_stale: false,
        }
    }

    // ───────────── leitura ─────────────

    /// A música atual (snapshot imutável e barato de clonar).
    pub fn song(&self) -> &Arc<Song> {
        self.store.song()
    }

    pub fn read_only(&self) -> bool {
        self.store.is_read_only()
    }

    pub fn flash(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    // ───────────── edição (caminho único) ─────────────

    /// Aplica uma edição: grava o estado anterior no histórico, roda `f` (use
    /// as funções de `wadawko_core::ops`), entrega ao store (autosave em 400 ms)
    /// e agenda a recompilação do áudio. Edição que não muda nada não gera
    /// entrada de desfazer.
    pub fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Song)) -> bool {
        self.edit_inner(cx, None, f)
    }

    /// Como [`edit`](Self::edit), mas edições seguidas com a mesma `tag` (ex.:
    /// arrastar um slider, girar o BPM) viram UMA entrada de desfazer.
    pub fn edit_coalesced(&mut self, cx: &mut Context<Self>, tag: u64, f: impl FnOnce(&mut Song)) -> bool {
        self.edit_inner(cx, Some(tag), f)
    }

    fn edit_inner(&mut self, cx: &mut Context<Self>, tag: Option<u64>, f: impl FnOnce(&mut Song)) -> bool {
        match apply_edit(self.store.song(), &mut self.history, tag, f) {
            Some(song) => {
                self.commit(song, cx);
                true
            }
            None => false,
        }
    }

    fn commit(&mut self, song: Arc<Song>, cx: &mut Context<Self>) {
        // seções apagadas saem da seleção de região; o trecho encolhe com a música
        let ids: Vec<&str> = song.sections().iter().map(|s| s.id.as_str()).collect();
        self.tp.selected.retain(|id| ids.contains(&id.as_str()));
        self.tp.range = self.tp.range.and_then(|r| clamp_region(r, song.measures.len()));
        if self.pen.as_deref().is_some_and(|p| song.person(p).is_none()) {
            self.pen = song.people().first().map(|p| p.id.clone());
        }
        self.store.update_song(song);
        self.revision += 1;
        self.request_audio_sync(cx);
        cx.emit(StateEvent::Edited);
        cx.notify();
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        match self.history.undo(self.store.song().clone()) {
            Some(prev) => {
                self.commit(prev, cx);
                true
            }
            None => {
                self.flash("nada para desfazer");
                cx.notify();
                false
            }
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        match self.history.redo(self.store.song().clone()) {
            Some(next) => {
                self.commit(next, cx);
                true
            }
            None => {
                self.flash("nada para refazer");
                cx.notify();
                false
            }
        }
    }

    /// Grava agora o que o autosave tiver pendente.
    pub fn flush(&mut self) -> anyhow::Result<()> {
        self.store.flush()
    }

    // ───────────── áudio ─────────────

    /// Chamado quando o banco de samples termina de carregar.
    pub fn attach_bank(&mut self, bank: SampleBank, cx: &mut Context<Self>) {
        let buffer: u32 = std::env::var("WADAWKO_BUFFER").ok().and_then(|s| s.parse().ok()).unwrap_or(256);
        let cfg = AudioConfig { buffer_frames: buffer, master_gain: if self.mute { 0.0 } else { 1.0 }, ..AudioConfig::default() };
        match Audio::start(&bank, cfg) {
            Ok(a) => self.audio = Some(a),
            Err(e) => {
                self.audio_error = Some(format!("{e:#}"));
                self.flash(format!("sem áudio: {e:#}"));
            }
        }
        self.bank = Some(bank);
        self.sync_audio();
        cx.notify();
    }

    /// Debounce: várias edições no mesmo instante viram uma recompilação.
    fn request_audio_sync(&mut self, cx: &mut Context<Self>) {
        if self.audio_stale {
            return;
        }
        self.audio_stale = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(30)).await;
            let _ = this.update(cx, |this, cx| {
                this.sync_audio();
                cx.notify();
            });
        })
        .detach();
    }

    /// Região efetiva do playback: trecho de loop, senão seções selecionadas.
    pub fn region(&self) -> Option<Region> {
        effective_region(self.tp.range, selected_region(self.store.song(), &self.tp.selected))
    }

    /// Recompila a música para o engine agora (região = trecho ou seções selecionadas).
    pub fn sync_audio(&mut self) {
        self.audio_stale = false;
        let song = rehearsal_song(self.store.song(), self.tp.rehearse.as_ref());
        let region = self.region();
        let Some(audio) = self.audio.as_mut() else { return };
        self.schedule = audio.set_song(&song, &CompileOptions { region, ..CompileOptions::default() });
        audio.apply_mix(&song);
        audio.set_metronome(self.tp.metronome);
        audio.set_metronome_config(self.tp.metro);
        audio.collect_garbage();
        if !self.playing() {
            self.playback_bpm = song.tempo as f32;
        }
    }

    /// Tocando? Logo após um comando vale a INTENÇÃO (o thread de áudio só
    /// confirma no próximo callback); depois, o estado real do relógio.
    pub fn playing(&self) -> bool {
        if self.play_cmd_at.elapsed() < Duration::from_millis(300) { self.want_play } else { self.audio.as_ref().is_some_and(|a| a.clock().snapshot().playing) }
    }

    /// Começa a tocar da célula-fonte global `start_cell` (com região, do início dela).
    pub fn play(&mut self, start_cell: usize, cx: &mut Context<Self>) {
        if self.audio_stale {
            self.sync_audio();
        }
        let trainer = self.tp.trainer.then_some(TrainerOptions { start_pct: self.tp.trainer_pct, step: self.tp.trainer_step });
        let opts = PlayOptions { start_cell, looping: self.tp.looping, count_in: self.tp.count_in, count_in_loop: self.tp.count_in_loop, trainer };
        match self.audio.as_mut() {
            Some(audio) => {
                audio.set_metronome(self.tp.metronome);
                audio.set_metronome_config(self.tp.metro);
                self.playback_bpm = audio.play(opts);
                self.want_play = true;
                self.play_cmd_at = Instant::now();
                if self.tp.trainer && !(self.schedule.has_region && self.tp.looping) {
                    self.flash("treino: selecione uma seção (clique no nome) e ligue o loop");
                }
            }
            None => self.flash(match &self.audio_error {
                Some(e) => format!("sem áudio: {e}"),
                None => "carregando samples…".to_string(),
            }),
        }
        cx.notify();
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(a) = self.audio.as_mut() {
            a.stop();
            a.stop_phrase_loop();
        }
        self.want_play = false;
        self.play_cmd_at = Instant::now();
        self.hits.clear();
        self.playback_bpm = self.song().tempo as f32;
        cx.notify();
    }

    pub fn seek_cell(&mut self, cell: usize) {
        if let Some(a) = self.audio.as_mut() {
            a.seek_cell(cell);
        }
    }

    pub fn set_bpm(&mut self, bpm: f64, cx: &mut Context<Self>) {
        let bpm = bpm.clamp(30.0, 300.0).round();
        if self.edit_coalesced(cx, 0xB9_0001, |s| s.tempo = bpm) {
            if let Some(a) = self.audio.as_mut() {
                a.set_bpm(bpm as f32);
            }
            self.playback_bpm = bpm as f32;
        }
    }

    pub fn set_loop(&mut self, on: bool, cx: &mut Context<Self>) {
        self.tp.looping = on;
        if let Some(a) = self.audio.as_mut() {
            a.set_loop(on);
        }
        cx.notify();
    }

    /// Define (ou limpa) o trecho de loop (F2). Com trecho, o loop liga; se
    /// estiver tocando, o playhead continua onde está quando ainda cai dentro
    /// do trecho novo, senão volta ao início dele.
    pub fn set_range(&mut self, range: Option<Region>, cx: &mut Context<Self>) {
        let song = self.song().clone();
        let range = range.and_then(|r| clamp_region(r, song.measures.len()));
        let was_playing = self.playing();
        let before = if was_playing { self.playhead().and_then(|p| p.cell) } else { None };
        let changed = range != self.tp.range;
        self.tp.range = range;
        if changed {
            self.sync_audio();
        }
        if range.is_some() && !self.tp.looping {
            self.tp.looping = true;
            if let Some(a) = self.audio.as_mut() {
                a.set_loop(true);
            }
        }
        if changed && was_playing {
            let region = self.region();
            let inside = |cell: usize| {
                let (m, _) = song.unflat(cell);
                region.is_none_or(|r| m >= r.start_bar && m < r.end_bar)
            };
            let target = match before {
                Some(cell) if inside(cell) => cell,
                _ => region.map_or(0, |r| song.flat(r.start_bar.min(song.measures.len().saturating_sub(1)), 0)),
            };
            self.seek_cell(target);
        }
        cx.notify();
    }

    pub fn set_metronome(&mut self, on: bool, cx: &mut Context<Self>) {
        self.tp.metronome = on;
        if let Some(a) = self.audio.as_mut() {
            a.set_metronome(on);
        }
        cx.notify();
    }

    /// Liga/desliga o modo "ensaiar como…" (vale na hora, tocando ou não).
    pub fn set_rehearse(&mut self, r: Option<Rehearse>, cx: &mut Context<Self>) {
        if self.tp.rehearse != r {
            self.tp.rehearse = r;
            self.sync_audio();
        }
        cx.notify();
    }

    /// Padrão de acentos / subdivisão / volume do metrônomo (vale na hora, tocando ou não).
    pub fn set_metronome_config(&mut self, m: Metronome, cx: &mut Context<Self>) {
        self.tp.metro = m;
        if let Some(a) = self.audio.as_mut() {
            a.set_metronome_config(m);
        }
        cx.notify();
    }

    /// Count-in a cada volta do loop (vale já na próxima volta).
    pub fn set_count_in_loop(&mut self, on: bool, cx: &mut Context<Self>) {
        self.tp.count_in_loop = on;
        if let Some(a) = self.audio.as_mut() {
            a.set_count_in_loop(on);
        }
        cx.notify();
    }

    /// Liga/desliga uma seção na região de playback, mantendo as outras
    /// (Ctrl+clique na barra da seção, ação `section::ToggleSelect`).
    pub fn toggle_section_selected(&mut self, id: &str, cx: &mut Context<Self>) {
        self.select_section(id, true, cx);
    }

    /// Clique na barra da seção: sem `additive` seleciona SÓ ela (clicar de novo
    /// na única selecionada limpa); com `additive` (Ctrl) liga/desliga mantendo as outras.
    pub fn select_section(&mut self, id: &str, additive: bool, cx: &mut Context<Self>) {
        let next = next_selection(&self.tp.selected, id, additive);
        if next == self.tp.selected {
            return;
        }
        self.tp.selected = next;
        let was_playing = self.playing();
        self.sync_audio();
        if was_playing {
            // a região mudou: recomeça do início dela
            self.play(0, cx);
        }
        cx.notify();
    }

    /// Tick AUDÍVEL (relógio do áudio − latência − offset do usuário).
    pub fn audible_tick(&self) -> Option<f64> {
        self.audio.as_ref().map(|a| a.clock().audible_tick(self.av_offset_ms))
    }

    /// Playhead dobrado para o compasso-fonte (numa repetição volta ao início da seção).
    pub fn playhead(&self) -> Option<Playhead> {
        let tick = self.audible_tick()?;
        let tm = &self.schedule.time;
        let u = tm.bar_at(tick)?;
        let mi = *tm.order.get(u)?;
        let (beats, _) = *tm.shape.get(mi)?;
        let q = (tick - tm.bar_tick(u)) / PPQ;
        let frac = if beats == 0 { 0.0 } else { (q / beats as f64).clamp(0.0, 1.0) as f32 };
        Some(Playhead { mi, frac, cell: tm.cell_at(tick), tick })
    }

    /// Duração do flash de um golpe (ms), como no `Score.svelte`.
    pub fn flash_ms(&self) -> f32 {
        (30_000.0 / self.playback_bpm.max(1.0)).clamp(140.0, 400.0)
    }

    /// Drena os eventos do engine. Chame uma vez por frame (o Shell faz isso).
    pub fn poll(&mut self) {
        let Some(audio) = self.audio.as_mut() else { return };
        let clock = audio.clock().clone();
        let lat_ns = clock.latency_ns.load(std::sync::atomic::Ordering::Relaxed) as f64 + clock.fixed_delay_ns.load(std::sync::atomic::Ordering::Relaxed) as f64;
        let delay = Duration::from_secs_f64(((lat_ns / 1e9) + self.av_offset_ms as f64 / 1000.0).clamp(0.0, 0.5));
        let now = Instant::now();
        for ev in audio.poll_events() {
            match ev {
                EngineEvent::HitFired { track, cell, hit, .. } => self.hits.push(HitFlash { track: track as usize, cell: cell as usize, hit: hit as usize, at: now + delay }),
                EngineEvent::BpmChanged { bpm } => self.playback_bpm = bpm,
                EngineEvent::Ended { .. } => {
                    self.want_play = false;
                }
                EngineEvent::LoopWrapped { .. } | EngineEvent::Xrun { .. } => {}
            }
        }
        let snap = clock.snapshot();
        self.sounding_bpm = if snap.playing { snap.bpm.round() } else { self.playback_bpm };
        self.levels.poll(audio.meters());
        audio.collect_garbage();
        let keep = Duration::from_millis(self.flash_ms() as u64 + 20);
        self.hits.retain(|h| now < h.at + keep);
    }

    // ───────────── medidores (F3) ─────────────

    /// Nível da linha `t`: máximo entre os canais dela no schedule (linha
    /// compartilhada = um canal por dono; o kiai entra junto).
    pub fn track_level(&self, t: usize) -> Level {
        let Some(track) = self.song().tracks.get(t) else { return Level::default() };
        let mut best = Level::default();
        for (i, ch) in self.schedule.channels.iter().enumerate() {
            if ch.is_used()
                && ch.track_id == track.id
                && let Some(l) = self.levels.chan.get(i)
                && l.level >= best.level
            {
                best = Level { level: l.level, peak: l.peak.max(best.peak), peak_at: l.peak_at };
            }
        }
        best
    }

    /// Nível do barramento da pessoa `pid`.
    pub fn person_level(&self, pid: &str) -> Level {
        self.schedule.buses.iter().position(|b| b.person_id == pid).and_then(|i| self.levels.bus.get(i).copied()).unwrap_or_default()
    }

    /// Volume de saída (strip master): −40..+6 dB; com `--mute` o som continua calado.
    pub fn set_master_db(&mut self, db: f64, cx: &mut Context<Self>) {
        self.master_db = db.clamp(-40.0, 6.0);
        if let Some(a) = self.audio.as_mut() {
            a.set_master_gain(if self.mute { 0.0 } else { 10f32.powf(self.master_db as f32 / 20.0) });
        }
        cx.notify();
    }

    // ───────────── projetos ─────────────

    fn after_project_change(&mut self, cx: &mut Context<Self>) {
        self.history.sync_epoch(self.store.epoch());
        if let Some(a) = self.audio.as_mut() {
            a.stop();
        }
        self.want_play = false;
        self.play_cmd_at = Instant::now();
        self.hits.clear();
        self.tp.selected.clear();
        self.tp.range = None;
        self.pen = None;
        self.start_cell = 0;
        self.revision += 1;
        self.sync_audio();
        self.playback_bpm = self.song().tempo as f32;
        cx.emit(StateEvent::SongReplaced);
        cx.notify();
    }

    fn report<T>(&mut self, what: &str, r: anyhow::Result<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.flash(format!("{what}: {e:#}"));
                None
            }
        }
    }

    pub fn switch_project(&mut self, id: &str, cx: &mut Context<Self>) {
        let r = self.store.switch_project(id);
        if self.report("não abri a música", r) == Some(true) {
            self.after_project_change(cx);
        }
        cx.notify();
    }

    pub fn new_project(&mut self, cx: &mut Context<Self>) {
        let r = self.store.new_project();
        if self.report("não criei a música", r).is_some() {
            self.after_project_change(cx);
        }
        cx.notify();
    }

    /// Acrescenta `song` como música nova e abre (importar, `:new título`).
    pub fn add_project(&mut self, song: Song, cx: &mut Context<Self>) {
        let r = self.store.add_project(song);
        if self.report("não criei a música", r).is_some() {
            self.after_project_change(cx);
        }
        cx.notify();
    }

    /// Traz as músicas de outro banco (`.db`) ou de um backup JSON (a fusão não
    /// sobrescreve nada; a música aberta continua aberta).
    pub fn import_library(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let r = self.store.import_library(path);
        if let Some(rep) = self.report("não importei", r) {
            let name = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let mut msg = match rep.added.len() {
                0 => format!("nada novo em {name}"),
                1 => format!("1 música importada de {name}: {}", rep.added[0].1),
                n => format!("{n} músicas importadas de {name}"),
            };
            let kept = rep.unchanged.len() + rep.kept_existing.len();
            if kept > 0 {
                msg.push_str(&format!(" · {kept} já estava(m) aqui"));
            }
            if !rep.invalid.is_empty() {
                msg.push_str(&format!(" · {} entrada(s) ilegível(is)", rep.invalid.len()));
            }
            self.flash(msg);
            self.revision += 1;
        }
        cx.notify();
    }

    pub fn duplicate_project(&mut self, cx: &mut Context<Self>) {
        let id = self.store.current_id().to_string();
        let r = self.store.duplicate_project(&id);
        if self.report("não dupliquei", r).is_some() {
            self.after_project_change(cx);
        }
        cx.notify();
    }

    /// Renomear a música atual é uma edição (entra no desfazer); as outras, só na lista.
    pub fn rename_project(&mut self, id: &str, title: &str, cx: &mut Context<Self>) {
        if id == self.store.current_id() {
            let t = title.to_string();
            self.edit(cx, |s| s.title = t);
        } else {
            let r = self.store.rename_project(id, title);
            self.report("não renomeei", r);
        }
        cx.notify();
    }

    pub fn delete_project(&mut self, id: &str, cx: &mut Context<Self>) {
        let r = self.store.delete_project(id);
        if self.report("não excluí", r).is_some() {
            self.after_project_change(cx);
        }
        cx.notify();
    }

    // ───────────── tema / preferências ─────────────

    pub fn set_theme(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.theme = if dark { Theme::dark() } else { Theme::light() };
        if !self.read_only() {
            let _ = self.store.set_theme(if dark { wadawko_store::Theme::Dark } else { wadawko_store::Theme::Light });
        }
        cx.notify();
    }

    // ───────────── export de áudio ─────────────

    pub fn start_export(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(bank) = self.bank.clone() else {
            self.flash("samples ainda carregando");
            return;
        };
        if self.export.is_some() {
            self.flash("já tem um export em andamento");
            return;
        }
        let song = (**self.song()).clone();
        let job = wadawko_export::spawn_export(song, bank, path.clone(), ExportOptions::default());
        self.export = Some(ExportRun { job, path, started: Instant::now() });
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(120)).await;
                let done = this.update(cx, |this, cx| {
                    cx.notify();
                    if this.export.as_ref().is_some_and(|e| e.job.is_finished()) {
                        let run = this.export.take().expect("export");
                        let took = run.started.elapsed().as_secs_f32();
                        match run.job.join() {
                            Ok(s) => this.flash(format!("exportado {:.0} s de áudio em {:.1} s → {}", s.seconds, took, s.path.display())),
                            Err(e) => this.flash(format!("export: {e:#}")),
                        }
                        true
                    } else {
                        this.export.is_none()
                    }
                });
                if !matches!(done, Ok(false)) {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    pub fn cancel_export(&mut self) {
        if let Some(e) = &self.export {
            e.job.cancel();
        }
    }

    /// Texto de progresso do export em andamento ("renderizando 42%").
    pub fn export_label(&self) -> Option<String> {
        let e = self.export.as_ref()?;
        let stage = match e.job.stage() {
            ExportStage::Rendering => "renderizando",
            ExportStage::Encoding => "codificando",
            ExportStage::Done => "finalizando",
        };
        let name = e.path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        Some(format!("{stage} {name} {:.0}%", e.job.progress() * 100.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::ops;
    use wadawko_store::Db;

    fn tmp_db(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wadawko-app-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("t.db")
    }

    #[test]
    fn edit_records_history_and_persists() {
        let path = tmp_db("edit");
        let mut store = ProjectStore::open(Db::open(&path).unwrap()).unwrap();
        let mut history = SongHistory::new();
        history.sync_epoch(store.epoch());
        let tid = store.song().tracks[0].id.clone();
        let before = store.song().clone();

        // edição que não muda nada não entra no histórico
        assert!(apply_edit(store.song(), &mut history, None, |_| {}).is_none());
        assert!(!history.can_undo());

        let next = apply_edit(store.song(), &mut history, None, |s| ops::cells::set_cell(s, &tid, 0, 0, Some("tsuku"), None)).unwrap();
        store.update_song(next);
        assert_eq!(ops::cells::get_cell(store.song(), &tid, 0, 0), Some("tsuku"));
        assert!(history.can_undo());
        store.flush().unwrap();
        drop(store);

        // sobrevive a reabrir
        let mut store = ProjectStore::open(Db::open(&path).unwrap()).unwrap();
        assert_eq!(ops::cells::get_cell(store.song(), &tid, 0, 0), Some("tsuku"));

        // desfazer devolve o estado anterior e também é gravado
        let prev = history.undo(store.song().clone()).unwrap();
        assert_eq!(*prev, *before);
        store.update_song(prev);
        store.flush().unwrap();
        drop(store);
        let store = ProjectStore::open(Db::open(&path).unwrap()).unwrap();
        assert_eq!(**store.song(), *before);
    }

    #[test]
    fn coalesced_edits_are_one_undo_step() {
        let store = ProjectStore::open(Db::open_in_memory().unwrap()).unwrap();
        let mut history = SongHistory::new();
        let mut cur = store.song().clone();
        let start = cur.tempo;
        for i in 1..=5 {
            cur = apply_edit(&cur, &mut history, Some(7), |s| s.tempo = start + i as f64).unwrap();
        }
        assert_eq!(history.undo_len(), 1);
        let back = history.undo(cur).unwrap();
        assert_eq!(back.tempo, start);
    }

    #[test]
    fn read_only_store_keeps_edits_in_memory_only() {
        let path = tmp_db("ro");
        drop(ProjectStore::open(Db::open(&path).unwrap()).unwrap());
        let mut store = ProjectStore::open(Db::open_read_only(&path).unwrap()).unwrap();
        let mut history = SongHistory::new();
        let next = apply_edit(store.song(), &mut history, None, |s| s.title = "outra".into()).unwrap();
        store.update_song(next);
        assert_eq!(store.song().title, "outra");
        store.flush().unwrap();
        drop(store);
        let store = ProjectStore::open(Db::open_read_only(&path).unwrap()).unwrap();
        assert_ne!(store.song().title, "outra");
    }
}

/// Seleção de seções depois de um clique em `id` (ver [`AppState::select_section`]).
pub fn next_selection(cur: &[String], id: &str, additive: bool) -> Vec<String> {
    let has = cur.iter().any(|x| x == id);
    if additive {
        if has { cur.iter().filter(|x| *x != id).cloned().collect() } else { cur.iter().cloned().chain([id.to_string()]).collect() }
    } else if has && cur.len() == 1 {
        Vec::new()
    } else {
        vec![id.to_string()]
    }
}

#[cfg(test)]
mod level_tests {
    use super::Level;

    #[test]
    fn level_rises_instantly_and_falls_36_db_per_second() {
        let mut l = Level::default();
        l.feed(1.0, 0.016, 0.0);
        assert_eq!(l.level, 1.0);
        assert_eq!(l.peak, 1.0);
        // 1 s de silêncio em passos de 16 ms: −36 dB ≈ ×0,0158
        let mut t = 0.0;
        for _ in 0..62 {
            t += 1.0 / 62.0;
            l.feed(0.0, 1.0 / 62.0, t);
        }
        assert!((l.level - 0.0158).abs() < 0.002, "{}", l.level);
        // pico retido por 1,5 s, depois cai a 12 dB/s
        assert_eq!(l.peak, 1.0);
        for _ in 0..62 {
            t += 1.0 / 62.0;
            l.feed(0.0, 1.0 / 62.0, t);
        }
        assert!(l.peak < 1.0 && l.peak > 0.3, "{}", l.peak);
        // um pico novo maior substitui na hora
        l.feed(0.8, 0.016, t);
        assert_eq!(l.peak, 0.8);
        assert!(!l.is_silent());
        for _ in 0..600 {
            t += 0.016;
            l.feed(0.0, 0.016, t);
        }
        assert!(l.is_silent());
    }
}

#[cfg(test)]
mod region_tests {
    use super::*;

    fn r(a: usize, b: usize) -> Region {
        Region { start_bar: a, end_bar: b }
    }

    #[test]
    fn range_wins_over_selected_sections() {
        assert_eq!(effective_region(None, None), None);
        assert_eq!(effective_region(None, Some(r(2, 4))), Some(r(2, 4)));
        assert_eq!(effective_region(Some(r(0, 1)), Some(r(2, 4))), Some(r(0, 1)));
        assert_eq!(effective_region(Some(r(0, 1)), None), Some(r(0, 1)));
    }

    #[test]
    fn clamp_shrinks_with_the_song_and_drops_empty() {
        assert_eq!(clamp_region(r(1, 3), 8), Some(r(1, 3)));
        assert_eq!(clamp_region(r(1, 9), 4), Some(r(1, 4)));
        assert_eq!(clamp_region(r(4, 9), 4), None);
        assert_eq!(clamp_region(r(2, 2), 8), None);
    }

    #[test]
    fn labels_are_one_based_and_inclusive() {
        assert_eq!(range_label(r(2, 3)), "c3");
        assert_eq!(range_label(r(2, 6)), "c3–c6");
    }
}

#[cfg(test)]
mod selection_tests {
    use super::next_selection;

    fn v(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plain_click_selects_only_that_section() {
        assert_eq!(next_selection(&v(&[]), "a", false), v(&["a"]));
        assert_eq!(next_selection(&v(&["a", "b"]), "c", false), v(&["c"]));
        // clicar numa já selecionada (entre várias) deixa só ela
        assert_eq!(next_selection(&v(&["a", "b"]), "b", false), v(&["b"]));
        // clicar na única selecionada limpa
        assert_eq!(next_selection(&v(&["a"]), "a", false), v(&[]));
    }

    #[test]
    fn ctrl_click_adds_or_removes_keeping_others() {
        assert_eq!(next_selection(&v(&["a"]), "b", true), v(&["a", "b"]));
        assert_eq!(next_selection(&v(&["a", "b"]), "a", true), v(&["b"]));
        assert_eq!(next_selection(&v(&[]), "a", true), v(&["a"]));
    }
}

#[cfg(test)]
mod rehearse_tests {
    use super::*;
    use wadawko_core::model::Person;

    #[test]
    fn sem_e_so_a_pessoa() {
        let mut s = wadawko_core::demo::demo_song();
        s.people = Some(vec![Person { id: "ana".into(), name: "Ana".into(), ..Default::default() }, Person { id: "bia".into(), name: "Bia".into(), ..Default::default() }]);
        let audible = |s: &Song, id: &str| wadawko_core::ops::people::person_audible(s, Some(id));
        let sem = rehearsal_song(&s, Some(&Rehearse { person: "ana".into(), listen: Listen::Without }));
        assert!(!audible(&sem, "ana") && audible(&sem, "bia"));
        let so = rehearsal_song(&s, Some(&Rehearse { person: "ana".into(), listen: Listen::Only }));
        assert!(audible(&so, "ana") && !audible(&so, "bia"));
        // a música original não muda
        assert!(audible(&s, "ana") && audible(&s, "bia"));
        assert_eq!(rehearsal_song(&s, None), s);
    }
}
