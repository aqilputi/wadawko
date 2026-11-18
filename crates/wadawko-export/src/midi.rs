//! Export MIDI (F10 de `docs/planejamento/daw.md`): Standard MIDI File tipo 1,
//! escrito à mão (o formato é pequeno; nada de crate novo).
//!
//! Sai do MESMO schedule do áudio (`wadawko_audio::Compiler`), então repetições
//! de seção, mapa de andamento (rall./accel.), fórmulas de compasso, swing,
//! dinâmicas, mudo/solo e "linhas principais" já vêm resolvidos.
//!
//! - divisão 960 ticks por semínima (o PPQ do schedule: sem arredondar nada);
//! - faixa 0: nome da música, fórmula de compasso a cada mudança, andamento a
//!   cada tempo em que o BPM muda (as rampas viram degraus por tempo) e um
//!   marcador com o nome de cada seção quando ela começa;
//! - uma faixa por linha, com o nome da linha, toda no canal 10 (bateria GM):
//!   a nota sai do instrumento base e da zona (ver [`gm_note`]); a força do golpe
//!   vira velocity (raiz do ganho relativo ao golpe mais forte da música).
//!   Kiai (vozes) não entra.

use wadawko_audio::{CompileOptions, Compiler, PPQ, Schedule};
use wadawko_core::ops::kit::resolve_instrument;
use wadawko_core::ops::sections::section_ranges;
use wadawko_core::{InstrumentId, Song};
use wadawko_samples::SampleBank;

/// Canal 10 (bateria General MIDI), base 0.
const DRUM_CH: u8 = 9;
/// Duração de cada nota (uma semicolcheia).
const NOTE_TICKS: u32 = (PPQ / 4.0) as u32;

/// Nota de bateria GM para o instrumento base e a zona (couro / borda).
pub fn gm_note(base: InstrumentId, rim: bool) -> u8 {
    use InstrumentId::*;
    match (base, rim) {
        (Kane, false) => 56, // cowbell
        (Kane, true) => 80,  // mute triangle
        (_, true) => 37,     // side stick (kara: borda)
        (Odaiko, _) => 35,   // acoustic bass drum
        (Onagado, _) => 41,  // low floor tom
        (Nagado, _) => 43,   // high floor tom
        (Chu, _) => 45,      // low tom
        (Okedo, _) => 47,    // low-mid tom
        (Hira, _) => 50,     // high tom
        (Shime, _) => 38,    // acoustic snare
    }
}

/// O arquivo `.mid` da música inteira.
pub fn song_to_midi(song: &Song, bank: &SampleBank) -> Vec<u8> {
    let schedule = Compiler::new(bank, 48_000).compile(song, &CompileOptions::default());
    schedule_to_midi(song, &schedule)
}

/// O `.mid` de um schedule já compilado (a música dá nomes e fórmulas).
pub fn schedule_to_midi(song: &Song, s: &Schedule) -> Vec<u8> {
    let mut tracks = vec![conductor_track(song, s)];
    let max_gain = s.hits().map(|(_, h)| h.gain).fold(0.0f32, f32::max).max(1e-6);
    for (ti, track) in song.tracks.iter().enumerate() {
        let base = resolve_instrument(song, track).base;
        let mut ev: Vec<(u32, Vec<u8>)> = vec![(0, meta(0x03, track.name.as_bytes()))];
        for (tick, h) in s.hits().filter(|(_, h)| h.track as usize == ti) {
            let t = tick.round().max(0.0) as u32;
            let note = gm_note(base, h.rim);
            let vel = (1.0 + 126.0 * (h.gain / max_gain).clamp(0.0, 1.0).sqrt()).round() as u8;
            ev.push((t, vec![0x90 | DRUM_CH, note, vel.clamp(1, 127)]));
            ev.push((t + NOTE_TICKS, vec![0x80 | DRUM_CH, note, 0]));
        }
        tracks.push(encode_track(ev));
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes()); // tipo 1
    out.extend_from_slice(&(tracks.len() as u16).to_be_bytes());
    out.extend_from_slice(&(PPQ as u16).to_be_bytes());
    for t in tracks {
        out.extend_from_slice(b"MTrk");
        out.extend_from_slice(&(t.len() as u32).to_be_bytes());
        out.extend_from_slice(&t);
    }
    out
}

/// Faixa 0: título, fórmulas, andamento e marcadores de seção.
fn conductor_track(song: &Song, s: &Schedule) -> Vec<u8> {
    let mut ev: Vec<(u32, Vec<u8>)> = vec![(0, meta(0x03, song.title.as_bytes()))];
    let time = &s.time;
    let secs = section_ranges(song);
    let mut last_sig = None;
    let mut last_bpm: Option<f64> = None;
    for (u, &mi) in time.order.iter().enumerate() {
        let start = time.bar_tick(u);
        let t = start.round() as u32;
        let beats = time.shape.get(mi).map_or(4, |x| x.0).clamp(1, 255) as u8;
        if last_sig != Some(beats) {
            // numerador, denominador como potência de 2 (4 = semínima), clocks por clique, 32 avos por semínima
            ev.push((t, meta(0x58, &[beats, 2, 24, 8])));
            last_sig = Some(beats);
        }
        // marcador ao entrar numa seção (inclusive em cada repetição)
        if let Some(sec) = secs.iter().find(|x| x.start == mi) {
            ev.push((t, meta(0x06, sec.name.as_bytes())));
        }
        for b in 0..beats as usize {
            let tick = start + b as f64 * PPQ;
            let bpm = s.curve.bpm_at(tick);
            if last_bpm.is_none_or(|l| (bpm - l).abs() > 0.01) {
                let us = (60_000_000.0 / bpm.max(1.0)).round() as u32;
                ev.push((tick.round() as u32, meta(0x51, &us.to_be_bytes()[1..])));
                last_bpm = Some(bpm);
            }
        }
    }
    encode_track(ev)
}

fn meta(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, kind];
    v.extend(vlq(data.len() as u32));
    v.extend_from_slice(data);
    v
}

/// Eventos absolutos → bytes da faixa (tempos delta + fim de faixa). Ordena de
/// forma estável, com note-off antes de note-on no mesmo tick.
fn encode_track(mut ev: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    ev.sort_by_key(|(t, e)| (*t, (e[0] & 0xF0) == 0x90));
    let mut out = Vec::new();
    let mut now = 0;
    for (t, e) in ev {
        out.extend(vlq(t - now));
        out.extend(e);
        now = t;
    }
    out.extend(vlq(0));
    out.extend([0xFF, 0x2F, 0x00]);
    out
}

/// Quantidade de tamanho variável do MIDI (7 bits por byte, mais significativo primeiro).
fn vlq(mut n: u32) -> Vec<u8> {
    let mut v = vec![(n & 0x7F) as u8];
    n >>= 7;
    while n > 0 {
        v.push(0x80 | (n & 0x7F) as u8);
        n >>= 7;
    }
    v.reverse();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use wadawko_core::demo::demo_song;

    #[test]
    fn vlq_conforme_a_especificacao() {
        assert_eq!(vlq(0), vec![0x00]);
        assert_eq!(vlq(0x7F), vec![0x7F]);
        assert_eq!(vlq(0x80), vec![0x81, 0x00]);
        assert_eq!(vlq(0x3FFF), vec![0xFF, 0x7F]);
        assert_eq!(vlq(0x0FFF_FFFF), vec![0xFF, 0xFF, 0xFF, 0x7F]);
    }

    /// Lê as faixas de volta (parser mínimo) e confere cabeçalho, notas e andamento.
    #[test]
    fn exporta_demo() {
        let song = demo_song();
        let bank = SampleBank::load_embedded().unwrap();
        let mid = song_to_midi(&song, &bank);
        assert_eq!(&mid[..4], b"MThd");
        let ntracks = u16::from_be_bytes([mid[10], mid[11]]) as usize;
        assert_eq!(ntracks, song.tracks.len() + 1);
        assert_eq!(u16::from_be_bytes([mid[12], mid[13]]), 960);
        // percorre as faixas contando note-ons e achando o andamento
        let mut pos = 14;
        let mut note_ons = 0;
        let mut tempo_us = None;
        for _ in 0..ntracks {
            assert_eq!(&mid[pos..pos + 4], b"MTrk");
            let len = u32::from_be_bytes(mid[pos + 4..pos + 8].try_into().unwrap()) as usize;
            let body = &mid[pos + 8..pos + 8 + len];
            let mut i = 0;
            while i < body.len() {
                while body[i] & 0x80 != 0 {
                    i += 1;
                }
                i += 1; // delta
                match body[i] {
                    0xFF => {
                        let kind = body[i + 1];
                        let n = body[i + 2] as usize; // tamanhos pequenos aqui
                        if kind == 0x51 && tempo_us.is_none() {
                            tempo_us = Some(u32::from_be_bytes([0, body[i + 3], body[i + 4], body[i + 5]]));
                        }
                        i += 3 + n;
                    }
                    st => {
                        if st & 0xF0 == 0x90 {
                            note_ons += 1;
                            assert_eq!(st & 0x0F, DRUM_CH);
                            assert!(body[i + 2] >= 1 && body[i + 2] <= 127);
                        }
                        i += 3;
                    }
                }
            }
            assert_eq!(&body[body.len() - 3..], &[0xFF, 0x2F, 0x00]);
            pos += 8 + len;
        }
        assert_eq!(pos, mid.len());
        let schedule = Compiler::new(&bank, 48_000).compile(&song, &CompileOptions::default());
        assert_eq!(note_ons, schedule.hits().count());
        assert_eq!(tempo_us, Some((60_000_000.0 / song.tempo).round() as u32));
    }

    #[test]
    fn notas_por_zona() {
        assert_eq!(gm_note(InstrumentId::Odaiko, false), 35);
        assert_eq!(gm_note(InstrumentId::Chu, true), 37);
        assert_eq!(gm_note(InstrumentId::Kane, true), 80);
    }
}
