//! Voz do kiai com gravações reais (`assets/samples/kiai/`, vozes masculina e
//! feminina). Cada palavra escrita na grade vira tomadas gravadas:
//!
//! 1. a palavra inteira, se existe gravação dela (`i`+`ya!` → "iya");
//! 2. senão, sílaba a sílaba: a gravação da sílaba ("ha", "yo", "sa"…) ou, na
//!    falta, a da mesma vogal ("so" → "yo"/"ho", "re" → "ei"/"hai").
//!
//! A tomada é sorteada entre as gravadas, levemente transposta pela voz da
//! pessoa e pelo contorno da palavra, e cortada antes da próxima sílaba. A
//! lista de grãos é montada FORA do callback (na compilação do schedule ou no
//! preview); o thread de áudio só toca.

use wadawko_core::kyai::{KyaiNote, KyaiProfile, KyaiSex};
use wadawko_samples::SampleBank;
use wadawko_samples::kiai::Voice;

use crate::dsp::Rng;
use crate::schedule::{EventKind, GrainEv};

/// Ganho das gravações em relação ao `gain` da nota (as tomadas estão
/// normalizadas em −1 dBFS; o kiai fica bem abaixo dos tambores).
const SAMPLE_GAIN: f64 = 1.2;
/// Transposição máxima (semitons): mais que isso a voz gravada soa artificial.
const MAX_SHIFT: f64 = 3.0;

pub fn voice_of(sex: KyaiSex) -> Voice {
    match sex {
        KyaiSex::Male => Voice::Male,
        KyaiSex::Female => Voice::Female,
    }
}

/// Sílaba/palavra sem "!", "-" e espaços, em minúsculas.
fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphabetic()).map(|c| c.to_ascii_lowercase()).collect()
}

/// Grafias diferentes da mesma palavra gravada.
fn alias(w: &str) -> &str {
    match w {
        "hiya" | "hyia" | "hyaa" => "hya",
        "orya" | "oriya" | "sorya" | "soriya" | "soryia" | "soorya" => "orya",
        "iyaa" => "iya",
        "hei" | "hey" | "sei" | "ey" => "ei",
        "yoi" | "yoo" | "yoho" => "yo",
        "hoo" => "ho",
        "haa" | "hah" => "ha",
        "haai" | "hhai" => "hai",
        "seiya" | "heiya" | "haiyaa" => "haiya",
        "saa" => "sa",
        "yaa" => "ya",
        _ => w,
    }
}

/// Palavras gravadas da mesma vogal, em ordem de preferência.
fn by_vowel(syl: &str) -> &'static [&'static str] {
    match syl.chars().rev().find(|c| "aeiou".contains(*c)) {
        Some('a') => &["ha", "ya", "sa"],
        Some('i') => &["hai", "ei"],
        Some('e') => &["ei", "hai"],
        Some('o') => &["yo", "ho", "hyo"],
        Some('u') => &["ho", "yo", "ha"],
        _ => &["ha"],
    }
}

/// O que toca em cada sílaba de uma palavra: `Some(palavra gravada)` ou
/// `None` (a sílaba já está dentro da gravação da palavra inteira).
pub fn word_plan(has: impl Fn(&str) -> bool, syls: &[&str]) -> Vec<Option<&'static str>> {
    let st = |w: &str| -> Option<&'static str> { wadawko_samples::kiai::TAKES.iter().find(|t| t.word == w && !t.count).map(|t| t.word).filter(|w| has(w)) };
    let joined = norm(&syls.concat());
    if syls.len() > 1
        && let Some(w) = st(alias(&joined))
    {
        let mut out = vec![None; syls.len()];
        out[0] = Some(w);
        return out;
    }
    syls.iter()
        .map(|s| {
            let n = norm(s);
            st(alias(&n)).or_else(|| by_vowel(&n).iter().find_map(|w| st(w))).or_else(|| st("ha"))
        })
        .collect()
}

/// Plano da palavra para a voz `voice` do banco.
pub fn plan_for(bank: &SampleBank, voice: Voice, syls: &[&str]) -> Vec<Option<&'static str>> {
    word_plan(|w| !bank.kiai_takes(voice, w, false).is_empty(), syls)
}

/// Grão que toca a gravação de `word` para a nota `note`. `gap_sec` = tempo
/// até a próxima sílaba da linha (corta antes dela), `delay` = atraso (s)
/// em relação ao evento.
#[allow(clippy::too_many_arguments)]
pub fn kiai_grain(bank: &SampleBank, voice: Voice, word: &str, note: &KyaiNote, prof: &KyaiProfile, gap_sec: f64, strip: u16, delay: f64, rng: &mut Rng) -> Option<EventKind> {
    let takes = bank.kiai_takes(voice, word, false);
    if takes.is_empty() {
        return None;
    }
    let id = takes[((rng.unit64() * takes.len() as f64) as usize).min(takes.len() - 1)];
    let len = bank.sample(id).duration_secs();
    // voz da pessoa (um terço do tom sintetizado) + um toque do contorno da palavra
    let st = (prof.pitch * 0.35 + (note.st0 + note.st1) * 0.5 * 0.12).clamp(-MAX_SHIFT, MAX_SHIFT);
    let rate = 2f64.powf(st / 12.0);
    let full = len / rate;
    // grito: a tomada inteira (até a próxima sílaba); sílaba de passagem: curta
    let dur = if note.shout { full.min(gap_sec * 0.98) } else { full.min((note.dur * 1.4).min(gap_sec * 0.98)) }.max(0.06);
    Some(EventKind::Grain(GrainEv {
        sample: id.0,
        start: 0.0,
        src_len: len as f32,
        rate: rate as f32,
        gain: (note.gain * SAMPLE_GAIN) as f32,
        att: 0.003,
        rel: (dur / 4.0).min(0.04) as f32,
        dur: dur as f32,
        delay: delay as f32,
        strip,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: &str) -> bool {
        true
    }

    #[test]
    fn palavra_inteira_ou_por_silaba() {
        assert_eq!(word_plan(all, &["i", "ya!"]), vec![Some("iya"), None]);
        assert_eq!(word_plan(all, &["so", "ri", "ya!"]), vec![Some("orya"), None, None]);
        assert_eq!(word_plan(all, &["hai!"]), vec![Some("hai")]);
        // sem gravação de "sore": sílaba a sílaba pela vogal
        assert_eq!(word_plan(all, &["so", "re!"]), vec![Some("yo"), Some("ei")]);
        // voz sem "yo"/"ei" (feminina): cai na próxima da mesma vogal
        let f = |w: &str| !matches!(w, "yo" | "ei" | "orya");
        assert_eq!(word_plan(f, &["so", "re!"]), vec![Some("ho"), Some("hai")]);
        assert_eq!(word_plan(f, &["so", "ri", "ya!"]), vec![Some("ho"), Some("hai"), Some("ya")]);
    }
}
