//! Rótulos de salto (código livre de prefixo, home row primeiro).

pub const HINT_ALPHABET: &str = "asdfghjklqwertyuiopzxcvbnm";

/// Código livre de prefixo com `n` rótulos, mais curtos primeiro.
pub fn labels(n: usize, alphabet: &[char]) -> Vec<String> {
    let mut leaves: std::collections::VecDeque<String> = alphabet.iter().map(|c| c.to_string()).collect();
    if alphabet.len() < 2 {
        return Vec::new();
    }
    // expande o ÚLTIMO dos rótulos MAIS CURTOS (o menos preferido do nível mais raso):
    // árvore balanceada — ex.: 90 alvos = 23 de 1 letra + 67 de 2, nunca "mmmf"
    while leaves.len() < n {
        let min = leaves.iter().map(|l| l.len()).min().unwrap_or(1);
        let idx = leaves.iter().rposition(|l| l.len() == min).expect("alfabeto vazio");
        let p = leaves.remove(idx).expect("índice válido");
        for c in alphabet {
            leaves.push_back(format!("{p}{c}"));
        }
    }
    let mut v: Vec<String> = leaves.into_iter().collect();
    v.sort_by_key(|l| l.len());
    v.truncate(n);
    v
}
