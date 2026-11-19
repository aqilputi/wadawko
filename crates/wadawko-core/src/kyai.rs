//! Kiai / kakegoe (掛け声) — parte de TEXTO de `src/lib/kyai.ts`: silabação
//! romaji, voz por pessoa (perfil determinístico a partir do id), entonação
//! (contorno) e a lista de eventos na ordem de execução. A síntese
//! (animalese + ruído) fica no crate de áudio.
//!
//! Gritos são escritos numa sub-linha de qualquer instrumento, UMA sílaba por
//! célula ("so" | "re!"). Entonação japonesa: sílaba inicial baixa subindo,
//! meio alto, e o "!" final é um ataque alto que despenca (o grito); sem "!" a
//! palavra termina neutra.

use crate::model::{KyaiVoice, Measure, Song, Track};

/// Kakegoe prontos (palavras separadas por espaço = gritos separados).
pub const KYAI_PRESETS: [&str; 11] = ["sa!", "sei!", "seiya!", "ha!", "hai!", "sore!", "soriya!", "hyia!", "soryia!", "soresoresoresore!", "soresoresoresore! soryia!"];

fn is_v(c: Option<u8>) -> bool {
    matches!(c, Some(b'a' | b'e' | b'i' | b'o' | b'u'))
}

/// Onset: consoante (dígrafos primeiro) + glide opcional ("hy", "ryi" antes de
/// vogal). Equivale a `/^(?:ch|sh|ts|[bcdfghjklmnpqrstvwxz])?(?:yi(?=[aeou])|y)?/`.
/// Retorna o comprimento casado.
fn onset_len(w: &[u8]) -> usize {
    let mut n = 0;
    if w.starts_with(b"ch") || w.starts_with(b"sh") || w.starts_with(b"ts") {
        n = 2;
    } else if w.first().is_some_and(|c| b"bcdfghjklmnpqrstvwxz".contains(c)) {
        n = 1;
    }
    let rest = &w[n..];
    if rest.starts_with(b"yi") && matches!(rest.get(2), Some(b'a' | b'e' | b'o' | b'u')) {
        n += 2;
    } else if rest.first() == Some(&b'y') {
        n += 1;
    }
    n
}

/// Texto → sílabas, uma por célula. "seiya!" → ["sei", "ya!"]; o "!" vai na
/// última sílaba de cada palavra. Ditongos (ai, ei, ou), vogal longa (aa, "a-")
/// e n final ficam na mesma sílaba; consoante dobrada (sokuon) fecha a anterior.
/// `None` = texto vazio ou inválido (letra sem vogal, símbolo estranho).
pub fn split_kyai(text: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let lower = text.to_lowercase();
    for raw in lower.split(|c: char| crate::js_space(c) || matches!(c, ',' | ';' | '|')).filter(|w| !w.is_empty()) {
        let trimmed = raw.trim_end_matches('!');
        let shout = trimmed.len() != raw.len();
        let w = trimmed.replace('ー', "-");
        if w.is_empty() || !w.bytes().all(|b| b.is_ascii_lowercase() || b == b'-') || !w.bytes().any(|b| is_v(Some(b))) {
            return None;
        }
        let w = w.as_bytes();
        let at = |i: usize| w.get(i).copied();
        let mut syls: Vec<Vec<u8>> = Vec::new();
        let mut i = 0;
        while i < w.len() {
            let onset = onset_len(&w[i..]);
            let v = at(i + onset);
            if !is_v(v) {
                // sem vogal: n moraico ou consoante dobrada → coda da sílaba anterior
                let c = w[i];
                match syls.last_mut() {
                    Some(last) if c == b'n' || c == b'-' || Some(c) == at(i + 1) => {
                        last.push(c);
                        i += 1;
                        continue;
                    }
                    _ => return None,
                }
            }
            let mut s: Vec<u8> = w[i..=i + onset].to_vec();
            i += s.len();
            // coda: vogal longa / "-", ditongo, n que não abre sílaba
            while i < w.len() {
                let c = w[i];
                let prev = *s.last().expect("sílaba tem ao menos a vogal");
                let next = at(i + 1);
                let next_is_vowel = is_v(next) || next == Some(b'y');
                let take =
                    c == b'-' || (c == prev && is_v(Some(c))) || (c == b'i' && b"aeou".contains(&prev) && !is_v(next)) || (c == b'u' && prev == b'o' && !is_v(next)) || (c == b'n' && !next_is_vowel);
                if !take {
                    break;
                }
                s.push(c);
                i += 1;
            }
            syls.push(s);
        }
        if shout && let Some(last) = syls.last_mut() {
            last.push(b'!');
        }
        out.extend(syls.into_iter().map(|s| String::from_utf8(s).expect("ascii")));
    }
    (!out.is_empty()).then_some(out)
}

/// A sílaba é um grito (termina em "!")?
pub fn is_shout(syl: &str) -> bool {
    syl.ends_with('!')
}

// ───────────────────────────── voz por pessoa ─────────────────────────────

/// Voz de um jogador.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KyaiProfile {
    /// transposição base (semitons)
    pub pitch: f64,
    /// multiplicador de duração das consoantes/sílabas
    pub speed: f64,
    /// 0..1 — quanto cada grito varia
    pub vary: f64,
    /// deslocamento (s) do recorte dentro da letra: muda o "timbre"
    pub grit: f64,
    /// volume do ruído das sibilantes
    pub breath: f64,
    /// extensão do contorno (gritos mais "cantados" ou mais secos)
    pub range: f64,
}

/// FNV-1a de 32 bits sobre as unidades UTF-16 do texto (igual ao `hash` do TS).
pub fn hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for c in s.encode_utf16() {
        h = (h ^ c as u32).wrapping_mul(16777619);
    }
    h
}

/// número 0..1 determinístico a partir de (seed, k)
fn rnd(seed: u32, k: f64) -> f64 {
    let x = (seed as f64 * 0.0001 + k * 12.9898).sin() * 43758.5453;
    x - x.floor()
}

/// Defaults da voz de kiai de uma linha (`KYAI_DEFAULTS`): pitch 0, vary 0.5, gain 0.
pub const KYAI_DEFAULT_PITCH: f64 = 0.0;
pub const KYAI_DEFAULT_VARY: f64 = 0.5;
pub const KYAI_DEFAULT_GAIN: f64 = 0.0;

/// `track.kyai` com os defaults aplicados: (pitch, vary, gain).
pub fn kyai_voice_or_default(voice: Option<&KyaiVoice>) -> (f64, f64, f64) {
    (voice.and_then(|v| v.pitch).unwrap_or(KYAI_DEFAULT_PITCH), voice.and_then(|v| v.vary).unwrap_or(KYAI_DEFAULT_VARY), voice.and_then(|v| v.gain).unwrap_or(KYAI_DEFAULT_GAIN))
}

/// Voz de um jogador (seed = id da pessoa; sem pessoa, id da linha — ver `kyai_seed`).
pub fn kyai_profile(seed_id: &str, track: &Track) -> KyaiProfile {
    let h = hash(seed_id);
    let (pitch, vary, _) = kyai_voice_or_default(track.kyai.as_ref());
    KyaiProfile { pitch: -4.0 + rnd(h, 1.0) * 9.0 + pitch, speed: 0.85 + rnd(h, 2.0) * 0.35, vary, grit: rnd(h, 3.0) * 0.012, breath: 0.5 + rnd(h, 4.0) * 0.6, range: 0.75 + rnd(h, 5.0) * 0.5 }
}

/// Voz dos kiai: o animalese sintetizado (padrão) ou as gravações reais.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KyaiTimbre {
    /// letras do animalese, com timbre de grito
    #[default]
    Animalese,
    /// gravação masculina
    Male,
    /// gravação feminina
    Female,
}

impl KyaiTimbre {
    pub fn parse(s: &str) -> Option<KyaiTimbre> {
        match s {
            "a" => Some(KyaiTimbre::Animalese),
            "m" => Some(KyaiTimbre::Male),
            "f" => Some(KyaiTimbre::Female),
            _ => None,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            KyaiTimbre::Animalese => "a",
            KyaiTimbre::Male => "m",
            KyaiTimbre::Female => "f",
        }
    }
}

/// Voz de quem grita: a da pessoa (`Person.voice`), senão a da linha
/// (`track.kyai.voice`), senão o animalese.
pub fn kyai_timbre(song: &Song, seed_id: &str, track: &Track) -> KyaiTimbre {
    song.people().iter().find(|p| p.id == seed_id).and_then(|p| p.voice.as_deref()).or_else(|| track.kyai.as_ref().and_then(|v| v.voice.as_deref())).and_then(KyaiTimbre::parse).unwrap_or_default()
}

// ───────────────────────────── entonação ─────────────────────────────

/// Contorno (início, fim) em semitons da sílaba `idx` de uma palavra de `n`
/// sílabas. 1 sílaba com "!" = ataque alto que cai (sa!, hai!); palavra longa
/// sobe da 1ª pra 2ª (heiban), alterna alto/médio no meio (so-re so-re…) e o
/// "!" final é o pico, caindo no fim (so-ri-YA!).
pub fn contour(idx: usize, n: usize, shout: bool) -> (f64, f64) {
    let lift = (idx as f64 * 0.15).min(2.0); // cantos longos vão esquentando
    if n == 1 {
        return if shout { (5.0, -4.0) } else { (3.0, 0.5) };
    }
    if idx + 1 == n {
        return if shout { (6.0 + lift, -5.0 + lift) } else { (2.0 + lift, 0.5 + lift) };
    }
    if idx == 0 {
        return (0.0, 1.5);
    }
    if idx % 2 == 1 { (3.0 + lift, 3.5 + lift) } else { (1.5 + lift, 2.0 + lift) }
}

// ───────────────────────────── eventos da música ─────────────────────────────

/// Uma sílaba de kiai na ordem de execução.
#[derive(Clone, Debug, PartialEq)]
pub struct KyaiEvent {
    /// índice da linha em `song.tracks`
    pub track_index: usize,
    pub track_id: String,
    /// compasso tocado (posição na ordem de execução)
    pub u: usize,
    /// compasso-fonte
    pub mi: usize,
    /// célula dentro do compasso
    pub ci: usize,
    pub beat: usize,
    /// fração do tempo (já com swing)
    pub frac: f64,
    /// posição absoluta em tempos desde o início da execução (início do compasso tocado `u`
    /// — soma dos tempos dos anteriores, ver `ops::measures::bar_beat_starts` — + `beat + frac`)
    pub beats: f64,
    pub syl: String,
    /// posição da sílaba dentro da palavra e tamanho da palavra
    pub idx: usize,
    pub n: usize,
    pub shout: bool,
    /// tempos até a próxima sílaba da mesma linha (`f64::INFINITY` = nada depois)
    pub gap_beats: f64,
}

/// Sílabas de kiai na ordem de execução `order`, agrupadas em palavras por
/// linha (a palavra termina num "!" ou numa pausa de mais de 1 tempo).
/// `swing_warp` = `|f| timeline::swing_warp(f, song.swing())`.
/// Saída agrupada por linha (na ordem em que cada linha aparece pela 1ª vez).
pub fn kyai_events(song: &Song, order: &[usize], swing_warp: impl Fn(f64) -> f64) -> Vec<KyaiEvent> {
    let mut by_track: Vec<(usize, Vec<KyaiEvent>)> = Vec::new();
    let starts = crate::ops::measures::bar_beat_starts(song, order);
    for (u, &mi) in order.iter().enumerate() {
        let Some(meas) = song.measures.get(mi) else { continue };
        let Some(kyai) = meas.kyai.as_ref() else { continue };
        for (track_index, track) in song.tracks.iter().enumerate() {
            let Some(row) = kyai.get(&track.id) else { continue };
            for ci in 0..meas.cell_count() {
                let Some(syl) = row.get(ci).and_then(|s| s.as_deref()).filter(|s| !s.is_empty()) else { continue };
                let beat = ci / meas.subdivision;
                let frac = swing_warp((ci % meas.subdivision) as f64 / meas.subdivision as f64);
                let ev = KyaiEvent {
                    track_index,
                    track_id: track.id.clone(),
                    u,
                    mi,
                    ci,
                    beat,
                    frac,
                    beats: starts[u] + beat as f64 + frac,
                    syl: syl.to_string(),
                    idx: 0,
                    n: 1,
                    shout: is_shout(syl),
                    gap_beats: f64::INFINITY,
                };
                match by_track.iter_mut().find(|(t, _)| *t == track_index) {
                    Some((_, list)) => list.push(ev),
                    None => by_track.push((track_index, vec![ev])),
                }
            }
        }
    }
    let mut all = Vec::new();
    for (_, mut list) in by_track {
        // palavra termina num "!" ou numa pausa de mais de 1 tempo
        let mut word_start = 0;
        for i in 0..list.len() {
            let next_beats = list.get(i + 1).map(|e| e.beats);
            list[i].gap_beats = next_beats.map_or(f64::INFINITY, |b| b - list[i].beats);
            if list[i].shout || next_beats.is_none() || list[i].gap_beats > 1.01 {
                let n = i + 1 - word_start;
                for (k, e) in list[word_start..=i].iter_mut().enumerate() {
                    e.idx = k;
                    e.n = n;
                }
                word_start = i + 1;
            }
        }
        all.extend(list);
    }
    all
}

/// Dono da célula de kiai (espelha o dono dos golpes) → semente da voz:
/// sem pessoa = id da linha; 1 pessoa = ela; compartilhada = dono da célula.
pub fn kyai_seed<'a>(track: &'a Track, meas: &'a Measure, ci: usize) -> &'a str {
    crate::ops::people::owner_in_measure(track, Some(meas), ci).unwrap_or(&track.id)
}

// ───────────────────────────── nota (parâmetros pra síntese) ─────────────────────────────

/// Parâmetros de uma sílaba pronta pra síntese.
#[derive(Clone, Debug, PartialEq)]
pub struct KyaiNote {
    pub syl: String,
    /// semitons no início/fim da sílaba (contorno + voz + variação)
    pub st0: f64,
    pub st1: f64,
    /// duração da sílaba (s)
    pub dur: f64,
    pub shout: bool,
    /// ganho linear
    pub gain: f64,
}

/// Monta a nota de uma sílaba: contorno japonês + voz do jogador + variação
/// aleatória por grito. `gap_sec` = tempo até a próxima sílaba da mesma linha
/// (pode ser ∞). `rand01` = fonte de aleatoriedade em [0, 1) (o `Math.random`
/// do TS); é chamada 4 vezes, na ordem: escala, deslocamento, duração, ganho.
pub fn kyai_note(syl: &str, idx: usize, n: usize, shout: bool, gap_sec: f64, prof: &KyaiProfile, mut rand01: impl FnMut() -> f64) -> KyaiNote {
    let v = prof.vary;
    let mut r = || rand01() * 2.0 - 1.0;
    let (c0, c1) = contour(idx, n, shout);
    let scale = prof.range * (1.0 + r() * 0.35 * v);
    let shift = r() * 1.2 * v;
    let base = if shout { 0.36 + if n == 1 { 0.0 } else { 0.08 } } else { 0.2 };
    let mut dur = base * prof.speed * (1.0 + r() * 0.15 * v);
    if !shout || idx + 1 < n {
        dur = dur.min(gap_sec * 0.95);
    }
    dur = dur.min(gap_sec * 0.98).max(0.06);
    KyaiNote {
        syl: syl.to_string(),
        st0: c0 * scale + shift,
        st1: c1 * scale + shift,
        dur,
        shout,
        // kiai fica BEM abaixo dos tambores (~16 dB): é voz de fundo, não solista
        gain: (if shout { 0.14 } else { 0.11 }) * (1.0 + r() * 0.12 * v),
    }
}

/// Micro-atraso humano (s): gritos raramente caem exatamente no tempo.
pub fn kyai_lag(vary: f64, rand01: f64) -> f64 {
    rand01 * 0.018 * vary
}
