//! Síntese do kiai — porte de `playKyai` (`src/lib/kyai.ts`): cada sílaba vira
//! uma lista de grãos (letras do `animalese.ogg` transpostas por playbackRate,
//! com envelope curto) + ruído filtrado nas consoantes sibilantes. A lista é
//! montada FORA do callback (na compilação do schedule ou no preview); o thread
//! de áudio só dispara os grãos como vozes.
//!
//! Timbre de GRITO (não de fala): a voz sobe ~3 semitons (garganta tensa) e
//! mais 2 no "!", cada grão da vogal ganha um dobro levemente desafinado
//! (aspereza), o grito leva um sub-grave uma oitava abaixo (rouquidão), um
//! sopro rasgado na faixa de 2,4 kHz por baixo da vogal e um estouro de ar no
//! ataque; o volume abre forte e cai junto com o tom.

use wadawko_core::kyai::{KyaiNote, KyaiProfile};
use wadawko_samples::animalese::{LETTER_SEC, letter};

use crate::dsp::{Biquad, FilterType, Rng};
use crate::schedule::{EventKind, GrainEv, NoiseEv};

/// Semitons a mais em todo kiai (voz tensa de grito) e a mais no "!".
const SHOUT_LIFT: f64 = 3.0;
const SHOUT_EXTRA: f64 = 2.0;
/// Razão do dobro desafinado da vogal (+30 cents).
const DETUNE: f64 = 1.017_478;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cons {
    Letter { l: char, dur: f64, gain: f64 },
    Noise { kind: FilterType, freq: f64, q: f64, dur: f64, gain: f64 },
}

const fn le(l: char, dur: f64) -> Cons {
    Cons::Letter { l, dur, gain: 1.0 }
}
const fn hp(freq: f64, q: f64, dur: f64, gain: f64) -> Cons {
    Cons::Noise { kind: FilterType::Highpass, freq, q, dur, gain }
}
const fn bp(freq: f64, q: f64, dur: f64, gain: f64) -> Cons {
    Cons::Noise { kind: FilterType::Bandpass, freq, q, dur, gain }
}

/// `CONS` do TS: letra do animalese, ou ruído filtrado nas que lá são mudas.
fn cons(key: &str) -> Option<Cons> {
    Some(match key {
        "k" => le('k', 0.035),
        "g" => le('g', 0.035),
        "p" => le('p', 0.03),
        "b" => le('b', 0.03),
        "d" => le('d', 0.03),
        "r" => Cons::Letter { l: 'd', dur: 0.018, gain: 0.6 }, // r japonês = batida de língua
        "m" => le('m', 0.045),
        "n" => le('n', 0.04),
        "w" => le('w', 0.03),
        "j" | "z" => le('j', 0.035),
        "y" => Cons::Letter { l: 'i', dur: 0.03, gain: 0.7 },
        "s" => hp(5200.0, 0.7, 0.075, 0.5),
        "sh" => bp(3000.0, 1.2, 0.07, 0.7),
        "ts" => hp(4500.0, 0.7, 0.05, 0.55),
        "ch" => bp(3600.0, 1.0, 0.05, 0.6),
        "t" => hp(3500.0, 0.7, 0.014, 0.7),
        "h" => bp(1700.0, 0.8, 0.05, 0.45),
        "f" => bp(1500.0, 0.8, 0.05, 0.4),
        _ => return None,
    })
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

/// `ONSET` do TS: consoante (dígrafos primeiro) + glide opcional. Devolve o nº de bytes.
fn onset_len(w: &str) -> usize {
    let b = w.as_bytes();
    let mut n = 0;
    if w.starts_with("ch") || w.starts_with("sh") || w.starts_with("ts") {
        n = 2;
    } else if b.first().is_some_and(|c| b"bcdfghjklmnpqrstvwxz".contains(c)) {
        n = 1;
    }
    let rest = &b[n..];
    if rest.starts_with(b"yi") && matches!(rest.get(2), Some(b'a' | b'e' | b'o' | b'u')) {
        n += 2;
    } else if rest.first() == Some(&b'y') {
        n += 1;
    }
    n
}

struct Phonemes {
    cons: Vec<Cons>,
    vowels: Vec<char>,
    coda: Vec<Cons>,
}

/// quebra a sílaba em consoantes + vogais ("hyia" → h, y, a; "sein" → s, e, i, n)
fn phonemes(syl: &str) -> Phonemes {
    let s = syl.trim_end_matches('!');
    let on = onset_len(s);
    let mut out = Phonemes { cons: Vec::new(), vowels: Vec::new(), coda: Vec::new() };
    let mut o = &s[..on];
    for dig in ["ch", "sh", "ts"] {
        if o.starts_with(dig) {
            out.cons.extend(cons(dig));
            o = &o[2..];
        }
    }
    let o = o.strip_suffix("yi").map(|p| format!("{p}y")).unwrap_or_else(|| o.to_string());
    for c in o.chars() {
        out.cons.extend(cons(c.encode_utf8(&mut [0; 4])));
    }
    let mut last = None;
    for c in s[on..].chars() {
        if c == '-' {
            out.vowels.extend(last);
        } else if is_vowel(c) {
            out.vowels.push(c);
            last = Some(c);
        } else if c == 'n' {
            out.coda.extend(cons("n"));
        }
        // consoante dobrada (sokuon) = só fecha a sílaba
    }
    out
}

/// Monta os grãos de uma sílaba. `sample` = índice do `animalese` no banco,
/// `strip` = canal de kiai da linha, `t0` = atraso (s) do início da sílaba em
/// relação ao evento (micro-atraso humano / posição no preview), `sr` = taxa
/// do engine (os coeficientes do ruído dependem dela).
#[allow(clippy::too_many_arguments)]
pub fn kyai_grains(note: &KyaiNote, prof: &KyaiProfile, sample: u16, strip: u16, t0: f64, sr: f64, rng: &mut Rng, out: &mut Vec<EventKind>) {
    let ph = phonemes(&note.syl);
    let lift = SHOUT_LIFT + if note.shout { SHOUT_EXTRA } else { 0.0 };
    let rate_at = |frac: f64| {
        let st = note.st0 + (note.st1 - note.st0) * frac.clamp(0.0, 1.0).powf(1.3) + prof.pitch + lift;
        2f64.powf(st / 12.0)
    };
    let grain = |l: char, t: f64, dur: f64, rate: f64, gain: f64, rng: &mut Rng, out: &mut Vec<EventKind>| {
        let Some(info) = letter(l) else { return };
        if dur <= 0.0 {
            return;
        }
        let idx = (l as u8 - b'a') as f64;
        let start = idx * LETTER_SEC as f64 + info.start as f64 + prof.grit + rng.unit64() * 0.004;
        // duração em tempo de buffer (o playbackRate encolhe/estica)
        let src_len = (LETTER_SEC as f64 - info.start as f64 - prof.grit).min(dur * rate);
        if src_len <= 0.0 {
            return;
        }
        out.push(EventKind::Grain(GrainEv {
            sample,
            start: start as f32,
            src_len: src_len as f32,
            rate: rate as f32,
            // volume da sílaba (nó `pre`) já multiplicado
            gain: (gain * info.gain as f64 * note.gain) as f32,
            att: (dur / 3.0).min(0.008) as f32,
            rel: (dur / 3.0).min(0.014) as f32,
            dur: dur as f32,
            delay: (t0 + t) as f32,
            strip,
        }));
    };
    let hiss = |c: Cons, t: f64, dur: f64, rng: &mut Rng, out: &mut Vec<EventKind>| {
        let Cons::Noise { kind, freq, q, gain, .. } = c else { return };
        if dur <= 0.0 {
            return;
        }
        let f = freq * (0.9 + rng.unit64() * 0.2);
        out.push(EventKind::Noise(NoiseEv {
            filter: Biquad::new(kind, f, 0.0, q, sr),
            gain: (gain * prof.breath * note.gain) as f32,
            att: (dur / 2.0).min(0.012) as f32,
            dur: dur as f32,
            delay: (t0 + t) as f32,
            strip,
            seed: rng.next_u32() | 1,
        }));
    };

    // consoantes do ataque
    let mut t = 0.0;
    let cons_end = note.dur * 0.45;
    for c in &ph.cons {
        match *c {
            Cons::Noise { dur, .. } => {
                let d = (dur * prof.speed).min((cons_end - t).max(0.01));
                hiss(*c, t, d, rng, out);
                t += d * 0.85;
            }
            Cons::Letter { l, dur, gain } => {
                let d = (dur * prof.speed).min((cons_end - t).max(0.01));
                grain(l, t, d, rate_at(0.0), gain, rng, out);
                t += d * 0.85;
            }
        }
    }
    // vogais: grãos de 60 ms a cada 40 ms — a sobreposição (20 ms) cobre as
    // rampas de entrada/saída, então a vogal sai contínua (sem o "picotado")
    let coda_dur = if ph.coda.is_empty() { 0.0 } else { (note.dur * 0.25).min(0.05) };
    let v_end = note.dur - coda_dur;
    let v_dur = (v_end - t).max(0.03);
    const G: f64 = 0.06;
    const HOP: f64 = 0.04;
    let n_grains = (((v_dur - (G - HOP)) / HOP).ceil() as i64).max(1);
    if note.shout {
        // estouro de ar no ataque + sopro rasgado por baixo da vogal
        let burst = (0.035 * prof.speed).min(v_dur * 0.5);
        hiss(Cons::Noise { kind: FilterType::Highpass, freq: 3200.0, q: 0.7, dur: burst, gain: 0.55 }, t, burst, rng, out);
        hiss(Cons::Noise { kind: FilterType::Bandpass, freq: 2400.0, q: 0.8, dur: v_dur, gain: 0.32 }, t, v_dur, rng, out);
    }
    for k in 0..n_grains {
        let gt = t + k as f64 * HOP;
        let frac = gt / note.dur;
        let pos = (gt - t) / v_dur;
        // ditongo: 2ª vogal ocupa o último terço
        let vi = if ph.vowels.len() > 1 && pos > 0.62 { ph.vowels.len() - 1 } else { 0 };
        let v = ph.vowels.get(vi).copied().unwrap_or('a');
        // grito: abre forte (+35 %) e cai até ~60 %; sílaba de passagem também empurrada
        let fade = if note.shout { 1.35 - 0.75 * pos } else { 1.1 };
        let d = G.min(v_end - gt + 0.005);
        // tremor da voz forçada: cada grão sai um pouco fora do tom
        let rate = rate_at(frac) * (1.0 + (rng.unit64() - 0.5) * 0.02);
        grain(v, gt, d, rate, fade, rng, out);
        // dobro desafinado (+30 cents, 6 ms depois): aspereza
        grain(v, gt + 0.006, (d - 0.006).max(0.01), rate * DETUNE, fade * 0.55, rng, out);
        if note.shout {
            // sub-grave uma oitava abaixo: rouquidão do grito
            grain(v, gt, d, rate * 0.5, fade * 0.22, rng, out);
        }
    }
    for c in &ph.coda {
        if let Cons::Letter { l, .. } = *c {
            grain(l, v_end, coda_dur, rate_at(1.0), 0.8, rng, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn letters(c: &[Cons]) -> String {
        c.iter().map(|c| if let Cons::Letter { l, .. } = c { *l } else { '~' }).collect()
    }

    #[test]
    fn phonemes_like_ts() {
        let p = phonemes("hyia!");
        assert_eq!(letters(&p.cons), "~i"); // h (ruído) + y (letra i)
        assert_eq!(p.vowels, vec!['a']);
        let p = phonemes("sein");
        assert_eq!(letters(&p.cons), "~");
        assert_eq!(p.vowels, vec!['e', 'i']);
        assert_eq!(letters(&p.coda), "n");
        let p = phonemes("tsu");
        assert_eq!(p.cons.len(), 1);
        assert!(matches!(p.cons[0], Cons::Noise { freq, .. } if freq == 4500.0));
        let p = phonemes("ya-");
        assert_eq!(letters(&p.cons), "i");
        assert_eq!(p.vowels, vec!['a', 'a']);
        let p = phonemes("re!");
        assert!(matches!(p.cons[0], Cons::Letter { l: 'd', gain, .. } if gain == 0.6));
    }

    #[test]
    fn grito_tem_timbre_de_grito() {
        let prof = KyaiProfile { pitch: 0.0, speed: 1.0, vary: 0.0, grit: 0.0, breath: 1.0, range: 1.0 };
        let note = KyaiNote { syl: "sei!".into(), st0: 5.0, st1: -4.0, dur: 0.36, shout: true, gain: 0.14 };
        let mut out = Vec::new();
        kyai_grains(&note, &prof, 72, 9, 0.0, 48_000.0, &mut Rng::new(1), &mut out);
        // s + estouro de ar + sopro rasgado
        let noise = out.iter().filter(|e| matches!(e, EventKind::Noise(_))).count();
        assert_eq!(noise, 3);
        let grains: Vec<&GrainEv> = out.iter().filter_map(|e| if let EventKind::Grain(g) = e { Some(g) } else { None }).collect();
        // vogal: grão + dobro + sub-grave por passo
        assert_eq!(grains.len() % 3, 0);
        let main: Vec<_> = grains.iter().step_by(3).collect();
        // mais agudo que a fala (sobe 5 st no grito) e cai de tom e de volume
        assert!(main[0].rate > 2f32.powf(9.0 / 12.0) * 0.98, "{}", main[0].rate);
        assert!(main.last().unwrap().rate < main[0].rate * 0.7);
        assert!(main.last().unwrap().gain < main[0].gain * 0.75, "{} {}", main.last().unwrap().gain, main[0].gain);
        // o sub-grave fica uma oitava abaixo
        assert!((grains[2].rate / grains[0].rate - 0.5).abs() < 1e-3);
        for g in &grains {
            assert_eq!((g.sample, g.strip), (72, 9));
            assert!(g.att > 0.0 && g.rel > 0.0 && g.att + g.rel <= g.dur + 1e-6);
            assert!(g.delay + g.dur <= 0.36 + 0.012);
        }
        // sílaba sem "!": sem sub-grave nem sopro
        let note = KyaiNote { syl: "so".into(), st0: 0.0, st1: 1.5, dur: 0.2, shout: false, gain: 0.11 };
        let mut out = Vec::new();
        kyai_grains(&note, &prof, 72, 9, 0.0, 48_000.0, &mut Rng::new(1), &mut out);
        assert_eq!(out.iter().filter(|e| matches!(e, EventKind::Noise(_))).count(), 1);
        assert_eq!(out.iter().filter(|e| matches!(e, EventKind::Grain(_))).count() % 2, 0);
    }
}
