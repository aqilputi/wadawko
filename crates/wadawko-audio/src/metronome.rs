//! Metrônomo configurável (F7 de `docs/planejamento/daw.md`): padrão de
//! acentos por tempo no estilo do Reaper (`ABBB` = forte, fraco, fraco, fraco;
//! `.` = mudo), subdivisão (1 a 4 cliques por tempo) e volume.
//!
//! O schedule já traz um `EventKind::Click { beat, sub }` por tempo e por
//! posição de subdivisão (em doze avos do tempo); quem decide se soa, e com
//! que nível, é o engine, com o [`Metronome`] atual — trocar o padrão não
//! recompila a música.

/// Nível de um tempo no padrão.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickLevel {
    /// forte (agudo, volume cheio)
    Accent,
    /// fraco
    Normal,
    /// não soa
    Mute,
}

/// Tamanho máximo do padrão (tempos).
pub const MAX_PATTERN: usize = 16;

/// Posições de subdivisão dentro do tempo, em doze avos (o 0 é o tempo).
pub const SUB_TWELFTHS: [u8; 6] = [0, 3, 4, 6, 8, 9];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metronome {
    /// níveis por tempo do compasso (cíclico); `len == 0` = automático
    /// (acento no tempo 1, fraco nos demais)
    pub pattern: [ClickLevel; MAX_PATTERN],
    pub len: u8,
    /// cliques por tempo: 1 (só o tempo), 2 (colcheias), 3 (tercinas) ou 4 (semicolcheias)
    pub subdiv: u8,
    /// volume linear (0..=2)
    pub gain: f32,
}

impl Default for Metronome {
    fn default() -> Self {
        Metronome { pattern: [ClickLevel::Normal; MAX_PATTERN], len: 0, subdiv: 1, gain: 1.0 }
    }
}

/// Altura e ganho de um clique.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickTone {
    pub freq: f32,
    pub gain: f32,
}

const ACCENT: ClickTone = ClickTone { freq: 1_046.502_3, gain: 1.0 };
const NORMAL: ClickTone = ClickTone { freq: 783.990_9, gain: 0.6 };
const SUB: ClickTone = ClickTone { freq: 783.990_9, gain: 0.32 };

impl Metronome {
    /// Lê um padrão: `A`/`X`/`1` = forte, `B`/`x`/`2` = fraco, `.`/`-`/`C`/`0` =
    /// mudo; espaços e `|` são ignorados. Vazio ou `auto` = automático.
    pub fn parse_pattern(s: &str) -> Option<([ClickLevel; MAX_PATTERN], u8)> {
        let mut pat = [ClickLevel::Normal; MAX_PATTERN];
        let t = s.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("auto") {
            return Some((pat, 0));
        }
        let mut n = 0;
        for ch in t.chars() {
            let lv = match ch {
                ' ' | '|' | ',' => continue,
                'A' | 'a' | 'X' | '1' | '>' => ClickLevel::Accent,
                'B' | 'b' | 'x' | '2' | 'o' => ClickLevel::Normal,
                '.' | '-' | '_' | 'C' | 'c' | '0' => ClickLevel::Mute,
                _ => return None,
            };
            if n == MAX_PATTERN {
                return None;
            }
            pat[n] = lv;
            n += 1;
        }
        Some((pat, n as u8))
    }

    /// O padrão como texto (`ABBB`, `A.B.`); automático = `auto`.
    pub fn pattern_text(&self) -> String {
        if self.len == 0 {
            return "auto".into();
        }
        self.pattern[..self.len as usize]
            .iter()
            .map(|l| match l {
                ClickLevel::Accent => 'A',
                ClickLevel::Normal => 'B',
                ClickLevel::Mute => '.',
            })
            .collect()
    }

    /// Nível do tempo `beat` (0 = tempo 1) do compasso.
    pub fn level(&self, beat: usize) -> ClickLevel {
        if self.len == 0 {
            return if beat == 0 { ClickLevel::Accent } else { ClickLevel::Normal };
        }
        self.pattern[beat % self.len as usize]
    }

    /// O que soa no clique (`beat`, `sub` em doze avos do tempo); `None` = nada.
    pub fn tone(&self, beat: usize, sub: u8) -> Option<ClickTone> {
        let g = self.gain.clamp(0.0, 2.0);
        if g <= 0.0 {
            return None;
        }
        let t = if sub == 0 {
            match self.level(beat) {
                ClickLevel::Accent => ACCENT,
                ClickLevel::Normal => NORMAL,
                ClickLevel::Mute => return None,
            }
        } else {
            let d = self.subdiv.clamp(1, 4);
            if d == 1 || !sub.is_multiple_of(12 / d) {
                return None;
            }
            SUB
        };
        Some(ClickTone { freq: t.freq, gain: t.gain * g })
    }

    /// Clique do count-in no tempo `beat`: segue os acentos, mas nunca é mudo
    /// (a contagem tem que ser ouvida).
    pub fn count_in_tone(&self, beat: usize) -> ClickTone {
        let g = self.gain.clamp(0.05, 2.0);
        let t = if self.level(beat) == ClickLevel::Accent { ACCENT } else { NORMAL };
        ClickTone { freq: t.freq, gain: t.gain * g }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padrao_ida_e_volta() {
        let (p, n) = Metronome::parse_pattern("A B . B").unwrap();
        let m = Metronome { pattern: p, len: n, ..Metronome::default() };
        assert_eq!(m.pattern_text(), "AB.B");
        assert_eq!(m.level(2), ClickLevel::Mute);
        // cíclico
        assert_eq!(m.level(4), ClickLevel::Accent);
        assert_eq!(Metronome::parse_pattern("auto").unwrap().1, 0);
        assert!(Metronome::parse_pattern("ABZ").is_none());
        assert!(Metronome::parse_pattern(&"A".repeat(17)).is_none());
    }

    #[test]
    fn automatico_e_subdivisao() {
        let mut m = Metronome::default();
        assert_eq!(m.tone(0, 0).unwrap().freq, ACCENT.freq);
        assert_eq!(m.tone(3, 0).unwrap().freq, NORMAL.freq);
        // subdivisão 1: só o tempo
        assert!(SUB_TWELFTHS[1..].iter().all(|&s| m.tone(0, s).is_none()));
        m.subdiv = 2;
        let on: Vec<u8> = SUB_TWELFTHS.iter().copied().filter(|&s| m.tone(1, s).is_some()).collect();
        assert_eq!(on, vec![0, 6]);
        m.subdiv = 3;
        let on: Vec<u8> = SUB_TWELFTHS.iter().copied().filter(|&s| m.tone(1, s).is_some()).collect();
        assert_eq!(on, vec![0, 4, 8]);
        m.subdiv = 4;
        let on: Vec<u8> = SUB_TWELFTHS.iter().copied().filter(|&s| m.tone(1, s).is_some()).collect();
        assert_eq!(on, vec![0, 3, 6, 9]);
        m.gain = 0.0;
        assert!(m.tone(0, 0).is_none());
    }

    #[test]
    fn count_in_nunca_mudo() {
        let (p, n) = Metronome::parse_pattern("A...").unwrap();
        let m = Metronome { pattern: p, len: n, ..Metronome::default() };
        assert!(m.tone(1, 0).is_none());
        assert_eq!(m.count_in_tone(1).freq, NORMAL.freq);
        assert_eq!(m.count_in_tone(0).freq, ACCENT.freq);
    }
}
