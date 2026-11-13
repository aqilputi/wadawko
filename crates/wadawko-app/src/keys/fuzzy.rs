//! Busca fuzzy da paleta (sem dependências, com folding de acentos).

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'ā' => 'a',
        'é' | 'ê' | 'è' | 'ē' => 'e',
        'í' | 'î' | 'ī' => 'i',
        'ó' | 'ô' | 'õ' | 'ö' | 'ō' => 'o',
        'ú' | 'ü' | 'ū' => 'u',
        'ç' => 'c',
        c => c.to_ascii_lowercase(),
    }
}

/// Subsequência com bônus para início de palavra e caracteres consecutivos.
/// `None` = não casa. Maior = melhor.
pub fn fuzzy(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(fold).collect();
    if q.is_empty() {
        return Some(0);
    }
    let t: Vec<char> = text.chars().map(|c| fold(c.to_lowercase().next().unwrap_or(c))).collect();
    let mut score = 0;
    let mut qi = 0;
    let mut last: Option<usize> = None;
    for (i, &c) in t.iter().enumerate() {
        if qi < q.len() && c == q[qi] {
            let word_start = i == 0 || !t[i - 1].is_alphanumeric();
            score += 10 + if word_start { 15 } else { 0 } + if last == Some(i.wrapping_sub(1)) { 12 } else { 0 };
            if let Some(l) = last {
                score -= (i - l - 1).min(10) as i32;
            }
            last = Some(i);
            qi += 1;
        }
    }
    (qi == q.len()).then_some(score - t.len() as i32 / 8)
}
