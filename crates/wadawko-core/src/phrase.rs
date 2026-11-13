//! Quebra de palavras kuchi shōga em sílabas do vocabulário (`src/lib/phrase.ts`).
//!
//! `"dokodoko"` → doko+doko; alongamento falado é absorvido (`"doooko"` casa
//! doko); CAIXA ALTA sem variante maiúscula no vocabulário = acento genérico;
//! `!` no fim = sforzando.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use crate::vocab::{Hand, VOCABULARY, Zone, syllable};

/// Sílabas do vocabulário, da mais longa pra mais curta (empate: ordem de declaração).
static KEYS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut keys: Vec<&'static str> = VOCABULARY.iter().map(|s| s.name).collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.len())); // estável, como o sort do JS
    keys
});

/// Uma sílaba reconhecida dentro de uma palavra.
#[derive(Clone, Debug, PartialEq)]
pub struct WordSyllable {
    /// nome no vocabulário (ex.: `"don"`, `"DON"`, `"doko"`)
    pub syl: &'static str,
    /// peso de duração: 1 normal; letras alongadas (doooko, DOOON) somam +1 cada
    pub stretch: u32,
    /// escrito em CAIXA ALTA sem variante maiúscula no vocabulário = acento genérico
    pub accent: bool,
}

fn lower_eq(c: char, key_lower: char) -> bool {
    let mut it = c.to_lowercase();
    it.next() == Some(key_lower) && it.next().is_none()
}

fn is_upper_str(s: &str) -> bool {
    s.to_uppercase() == s
}

/// Tenta casar uma sílaba do vocabulário a partir de `pos`, absorvendo letras
/// repetidas (alongamento falado: "dooon" casa com "don", stretch 3).
/// Retorna (caracteres consumidos, stretch).
fn match_at(word: &[char], pos: usize, key: &str) -> Option<(usize, u32)> {
    let key: Vec<char> = key.chars().map(|c| c.to_ascii_lowercase()).collect();
    let mut wi = pos;
    let mut extra = 0;
    for (ki, &cur) in key.iter().enumerate() {
        if !word.get(wi).is_some_and(|&c| lower_eq(c, cur)) {
            return None;
        }
        wi += 1;
        let next = key.get(ki + 1).copied();
        while wi < word.len() && lower_eq(word[wi], cur) && Some(cur) != next {
            wi += 1;
            extra += 1;
        }
    }
    Some((wi - pos, 1 + extra))
}

struct Candidate {
    key: &'static str,
    key_lower: String,
    consumed: usize,
    stretch: u32,
    accent: bool,
}

/// Quebra uma palavra em sílabas do vocabulário. Tenta o maior consumo primeiro
/// mas com backtracking: "dokon" falha como doko+n e recua para do+kon.
/// `None` = a palavra não é formada só por sílabas conhecidas. Palavra vazia = `Some([])`.
pub fn split_word(word: &str) -> Option<Vec<WordSyllable>> {
    let chars: Vec<char> = word.chars().collect();
    let mut memo: HashMap<usize, Option<Vec<WordSyllable>>> = HashMap::new();
    go(&chars, 0, &mut memo)
}

fn go(word: &[char], pos: usize, memo: &mut HashMap<usize, Option<Vec<WordSyllable>>>) -> Option<Vec<WordSyllable>> {
    if pos >= word.len() {
        return Some(Vec::new());
    }
    if let Some(cached) = memo.get(&pos) {
        return cached.clone();
    }

    // candidatos dedupe por (consumo, sílaba minúscula), variante pela caixa escrita
    let mut by_shape: Vec<Candidate> = Vec::new();
    for &key in KEYS.iter() {
        let Some((consumed, stretch)) = match_at(word, pos, key) else { continue };
        let frag: String = word[pos..pos + consumed].iter().collect();
        let want_upper = is_upper_str(&frag);
        let key_lower = key.to_lowercase();
        let key_is_upper = is_upper_str(key);
        let matches_case = key_is_upper == want_upper;
        let cand = Candidate { key, key_lower, consumed, stretch, accent: want_upper && !key_is_upper };
        match by_shape.iter_mut().find(|c| c.consumed == consumed && c.key_lower == cand.key_lower) {
            // Map.set em chave existente mantém a posição original
            Some(existing) => {
                if matches_case {
                    *existing = cand;
                }
            }
            None => by_shape.push(cand),
        }
    }
    by_shape.sort_by_key(|c| std::cmp::Reverse(c.consumed));

    let mut result = None;
    for cand in &by_shape {
        if let Some(rest) = go(word, pos + cand.consumed, memo) {
            let mut out = Vec::with_capacity(rest.len() + 1);
            out.push(WordSyllable { syl: cand.key, stretch: cand.stretch, accent: cand.accent });
            out.extend(rest);
            result = Some(out);
            break;
        }
    }
    memo.insert(pos, result.clone());
    result
}

fn is_separator(c: char) -> bool {
    crate::js_space(c) || matches!(c, ',' | ';' | '|')
}

/// Separa o texto em palavras por espaços, `,`, `;` e `|` — sem validar.
pub fn split_text(text: &str) -> Vec<&str> {
    text.split(is_separator).filter(|w| !w.is_empty()).collect()
}

/// Texto livre → lista de palavras válidas (cada palavra = UMA célula; palavra
/// composta como "dokodoko" mantém todos os golpes dentro da própria célula).
/// `None` se alguma palavra não for kuchi shōga válido. As palavras voltam como
/// escritas (com caixa, alongamento e `!`).
pub fn parse_words(text: &str) -> Option<Vec<String>> {
    let words = split_text(text);
    for word in &words {
        split_word(strip_sfz(word))?;
    }
    Some(words.into_iter().map(str::to_string).collect())
}

/// Sufixo de sforzando: "don!" = don com sfz (acento súbito no 1º golpe).
pub const SFZ_MARK: char = '!';

/// O valor tem a marca de sfz? (um `!` sozinho não conta)
pub fn is_sfz(value: &str) -> bool {
    value.chars().count() > 1 && value.ends_with(SFZ_MARK)
}

/// Valor sem a marca de sfz.
pub fn strip_sfz(value: &str) -> &str {
    if is_sfz(value) { &value[..value.len() - SFZ_MARK.len_utf8()] } else { value }
}

/// Liga/desliga o sfz de um valor de célula (`None`/vazio fica como está).
pub fn toggle_sfz(value: Option<&str>) -> Option<String> {
    let v = value?;
    if v.is_empty() {
        return Some(String::new());
    }
    Some(if is_sfz(v) { strip_sfz(v).to_string() } else { format!("{v}{SFZ_MARK}") })
}

/// Golpe já posicionado dentro da célula.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedHit {
    /// posição do golpe como fração da célula [0, 1)
    pub offset: f64,
    pub zone: Zone,
    pub vel: f64,
    pub hand: Hand,
    pub accent: bool,
    /// sforzando: golpe forçado acima da dinâmica do trecho
    pub sfz: bool,
}

/// Uma sílaba da célula com os golpes dela.
///
/// Atenção (igual ao TS): com sfz, o 1º golpe em `CellResolution::hits` sai com
/// `vel = 1`, mas a cópia dele aqui em `parts` continua com a força original.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPart {
    pub syl: &'static str,
    pub hits: Vec<ResolvedHit>,
}

/// Resultado de `resolve_cell`.
#[derive(Clone, Debug, PartialEq)]
pub struct CellResolution {
    /// todos os golpes da célula, em ordem (o índice é o `hitIndex` da hitKey)
    pub hits: Vec<ResolvedHit>,
    pub parts: Vec<ResolvedPart>,
    /// a célula tem sfz (marca "!" no fim)
    pub sfz: bool,
}

/// Resolve o valor de uma célula (sílaba simples ou palavra composta) nos golpes
/// que soam dentro dela, sem cache. Sílabas dividem a célula por peso de
/// alongamento; os offsets 0/0.5 de cada sílaba caem dentro da fatia dela.
/// `None` = valor não reconhecido (a célula não soa).
pub fn resolve_cell_uncached(value: &str) -> Option<CellResolution> {
    let sfz = is_sfz(value);
    let parts = split_word(strip_sfz(value))?;
    let total: u32 = parts.iter().map(|p| p.stretch).sum();
    let total = total as f64;
    let mut cum = 0u32;
    let mut hits = Vec::new();
    let mut out_parts = Vec::with_capacity(parts.len());
    for part in &parts {
        let entry = syllable(part.syl).expect("split_word só devolve sílabas do vocabulário");
        let start = cum as f64 / total;
        let width = part.stretch as f64 / total;
        let part_hits: Vec<ResolvedHit> = entry
            .hits
            .iter()
            .map(|h| ResolvedHit {
                offset: start + h.offset * width,
                zone: h.zone,
                vel: if part.accent { (h.vel * 1.5).min(1.0) } else { h.vel },
                hand: h.hand,
                accent: part.accent || h.vel >= 1.0,
                sfz: false,
            })
            .collect();
        hits.extend(part_hits.iter().copied());
        out_parts.push(ResolvedPart { syl: part.syl, hits: part_hits });
        cum += part.stretch;
    }
    // sfz: o 1º golpe da nota sai na força máxima (o resto da nota segue normal)
    if sfz && let Some(first) = hits.first_mut() {
        first.vel = 1.0;
        first.accent = true;
        first.sfz = true;
    }
    Some(CellResolution { hits, parts: out_parts, sfz })
}

static RESOLVE_MEMO: LazyLock<RwLock<HashMap<String, Option<Arc<CellResolution>>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

/// `resolve_cell_uncached` com memo global (thread-safe). O cache cresce com os
/// valores distintos já vistos (na prática, dezenas).
pub fn resolve_cell(value: &str) -> Option<Arc<CellResolution>> {
    if let Some(cached) = RESOLVE_MEMO.read().unwrap_or_else(|e| e.into_inner()).get(value) {
        return cached.clone();
    }
    let result = resolve_cell_uncached(value).map(Arc::new);
    RESOLVE_MEMO.write().unwrap_or_else(|e| e.into_inner()).insert(value.to_string(), result.clone());
    result
}
