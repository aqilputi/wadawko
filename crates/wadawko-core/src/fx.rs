//! Efeitos por célula estilo Renoise, para taiko (`docs/planejamento/tracker.md` §4).
//!
//! Texto curto e legível gravado em `Measure.fx` (paralelo a `cells`), até 3
//! comandos por célula, separados por espaço, em qualquer ordem:
//!
//! | comando | faz |
//! |---|---|
//! | `V0`–`V9` | velocity × n/10 (`V0` = ghost, 5 %) |
//! | `R2`–`R9` + `<`/`>` | rulo: repete o 1º golpe n vezes na célula, mãos alternando; `<`/`>` rampa de força |
//! | `F`, `F1`–`F3` | flam: golpe de apoio antes, mão oposta, 50 %, a n/16 da célula (`F` = `F2`) |
//! | `D-5`–`D+5` | atraso/antecipação de n/10 da célula |
//! | `X`, `X1`–`X9` | corte: abafa a linha a n/10 da célula (`X` = `X5`) |
//! | `H0`–`H9` | humanização × n/5 (`H5` = padrão, `H0` = exato) |
//!
//! [`apply`] é a função ÚNICA que transforma os golpes de uma célula — o áudio,
//! a esteira e o MIDI a chamam, então todos concordam.

use std::fmt;

use crate::phrase::{CellResolution, ResolvedHit};
use crate::vocab::Hand;

/// Rampa de força do rulo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ramp {
    Flat,
    /// crescendo (`<`): 0,6 → 1,0
    Up,
    /// decrescendo (`>`): 1,0 → 0,6
    Down,
}

/// Um comando de efeito.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fx {
    Vel(u8),
    Roll(u8, Ramp),
    Flam(u8),
    Delay(i8),
    Cut(u8),
    Human(u8),
}

impl Fx {
    /// letra do comando (ordem canônica: V R F D X H)
    pub fn letter(self) -> char {
        match self {
            Fx::Vel(_) => 'V',
            Fx::Roll(..) => 'R',
            Fx::Flam(_) => 'F',
            Fx::Delay(_) => 'D',
            Fx::Cut(_) => 'X',
            Fx::Human(_) => 'H',
        }
    }

    fn order(self) -> u8 {
        "VRFDXH".find(self.letter()).unwrap_or(0) as u8
    }

    /// nome curto para a UI ("rulo ×4 cresc.")
    pub fn label(self) -> String {
        match self {
            Fx::Vel(0) => "ghost".into(),
            Fx::Vel(n) => format!("força {}0 %", n),
            Fx::Roll(n, r) => format!(
                "rulo ×{n}{}",
                match r {
                    Ramp::Flat => "",
                    Ramp::Up => " cresc.",
                    Ramp::Down => " decresc.",
                }
            ),
            Fx::Flam(_) => "flam".into(),
            Fx::Delay(n) if n > 0 => format!("atrasa {}0 %", n),
            Fx::Delay(n) => format!("antecipa {}0 %", -n),
            Fx::Cut(_) => "corte".into(),
            Fx::Human(0) => "exato".into(),
            Fx::Human(n) => format!("humanização ×{:.1}", n as f64 / 5.0),
        }
    }
}

impl fmt::Display for Fx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Fx::Vel(n) => write!(f, "V{n}"),
            Fx::Roll(n, r) => write!(
                f,
                "R{n}{}",
                match r {
                    Ramp::Flat => "",
                    Ramp::Up => "<",
                    Ramp::Down => ">",
                }
            ),
            Fx::Flam(2) => write!(f, "F"),
            Fx::Flam(n) => write!(f, "F{n}"),
            Fx::Delay(n) => write!(f, "D{n:+}"),
            Fx::Cut(5) => write!(f, "X"),
            Fx::Cut(n) => write!(f, "X{n}"),
            Fx::Human(n) => write!(f, "H{n}"),
        }
    }
}

/// Erro de leitura de um texto de efeitos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FxError(pub String);

impl fmt::Display for FxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Máximo de comandos por célula.
pub const MAX_FX: usize = 3;

/// Lê o texto (`"r4< d+1"`). Maiúsculas/minúsculas valem igual; repetir a
/// mesma letra fica com a última. Vazio = sem efeitos.
pub fn parse(s: &str) -> Result<Vec<Fx>, FxError> {
    let mut out: Vec<Fx> = Vec::new();
    for tok in s.split_whitespace() {
        let up = tok.to_ascii_uppercase();
        let mut ch = up.chars();
        let letter = ch.next().unwrap_or(' ');
        let rest: String = ch.collect();
        let digit = |r: &str, lo: u8, hi: u8, default: Option<u8>| -> Result<u8, FxError> {
            if r.is_empty() {
                return default.ok_or_else(|| FxError(format!("{tok}: falta o número")));
            }
            match r.parse::<u8>() {
                Ok(n) if (lo..=hi).contains(&n) => Ok(n),
                _ => Err(FxError(format!("{tok}: número de {lo} a {hi}"))),
            }
        };
        let fx = match letter {
            'V' => Fx::Vel(digit(&rest, 0, 9, None)?),
            'R' => {
                let (num, ramp) = match rest.chars().last() {
                    Some('<') => (&rest[..rest.len() - 1], Ramp::Up),
                    Some('>') => (&rest[..rest.len() - 1], Ramp::Down),
                    _ => (rest.as_str(), Ramp::Flat),
                };
                Fx::Roll(digit(num, 2, 9, None)?, ramp)
            }
            'F' => Fx::Flam(digit(&rest, 1, 3, Some(2))?),
            'D' => {
                let n: i8 = rest.parse().map_err(|_| FxError(format!("{tok}: use D-5 a D+5")))?;
                if !(-5..=5).contains(&n) {
                    return Err(FxError(format!("{tok}: use D-5 a D+5")));
                }
                Fx::Delay(n)
            }
            'X' => Fx::Cut(digit(&rest, 1, 9, Some(5))?),
            'H' => Fx::Human(digit(&rest, 0, 9, None)?),
            _ => return Err(FxError(format!("{tok}: comando desconhecido (V R F D X H)"))),
        };
        out.retain(|f| f.letter() != fx.letter());
        out.push(fx);
    }
    if out.len() > MAX_FX {
        return Err(FxError(format!("no máximo {MAX_FX} efeitos por célula")));
    }
    out.sort_by_key(|f| f.order());
    Ok(out)
}

/// Texto canônico (ordem V R F D X H, maiúsculas).
pub fn format(fx: &[Fx]) -> String {
    let mut v = fx.to_vec();
    v.sort_by_key(|f| f.order());
    v.iter().map(Fx::to_string).collect::<Vec<_>>().join(" ")
}

/// Normaliza o texto; `Ok(None)` = vazio (sem efeitos).
pub fn normalize(s: &str) -> Result<Option<String>, FxError> {
    let fx = parse(s)?;
    Ok((!fx.is_empty()).then(|| format(&fx)))
}

/// Golpe depois dos efeitos. `offset` pode ser negativo (antecipação/flam,
/// até −1 = início da célula anterior) ou ≥ 1 (atraso).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxHit {
    pub hit: ResolvedHit,
    /// golpe de apoio do flam (não é o principal)
    pub grace: bool,
}

/// Resultado de [`apply`].
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    pub hits: Vec<FxHit>,
    /// corte da linha nesta fração da célula (`X`)
    pub cut: Option<f64>,
    /// escala da humanização (1 = padrão; `H0` = 0)
    pub humanize: f64,
}

fn other(h: Hand) -> Hand {
    match h {
        Hand::R => Hand::L,
        Hand::L => Hand::R,
    }
}

/// Aplica os efeitos aos golpes resolvidos da célula. Sem efeitos, devolve os
/// mesmos golpes (mesma ordem, mesmos valores).
pub fn apply(res: &CellResolution, fx: &[Fx]) -> Applied {
    let mut hits: Vec<FxHit> = res.hits.iter().map(|&hit| FxHit { hit, grace: false }).collect();
    let mut cut = None;
    let mut humanize = 1.0;
    // ordem fixa: rulo (troca os golpes) → velocity → flam → atraso → corte
    if let Some((n, ramp)) = fx.iter().find_map(|f| if let Fx::Roll(n, r) = *f { Some((n, r)) } else { None })
        && let Some(first) = hits.first().copied()
    {
        let n = n.max(1) as usize;
        hits = (0..n)
            .map(|i| {
                let t = if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 };
                let k = match ramp {
                    Ramp::Flat => 1.0,
                    Ramp::Up => 0.6 + 0.4 * t,
                    Ramp::Down => 1.0 - 0.4 * t,
                };
                let hand = if i % 2 == 0 { first.hit.hand } else { other(first.hit.hand) };
                // o sfz/acento fica só no 1º golpe do rulo
                let mut h = ResolvedHit { offset: first.hit.offset + (1.0 - first.hit.offset) * i as f64 / n as f64, vel: first.hit.vel * k, hand, ..first.hit };
                if i > 0 {
                    h.sfz = false;
                }
                FxHit { hit: h, grace: false }
            })
            .collect();
    }
    for f in fx {
        match *f {
            Fx::Vel(n) => {
                let k = if n == 0 { 0.05 } else { n as f64 / 10.0 };
                for h in &mut hits {
                    h.hit.vel *= k;
                }
            }
            Fx::Human(n) => humanize = n as f64 / 5.0,
            Fx::Cut(n) => cut = Some(n as f64 / 10.0),
            _ => {}
        }
    }
    if let Some(Fx::Flam(n)) = fx.iter().copied().find(|f| matches!(f, Fx::Flam(_)))
        && let Some(main) = hits.first().copied()
    {
        let grace = ResolvedHit { offset: (main.hit.offset - n as f64 / 16.0).max(-1.0), vel: main.hit.vel * 0.5, hand: other(main.hit.hand), accent: false, sfz: false, ..main.hit };
        hits.insert(0, FxHit { hit: grace, grace: true });
    }
    if let Some(Fx::Delay(n)) = fx.iter().copied().find(|f| matches!(f, Fx::Delay(_))) {
        let d = n as f64 / 10.0;
        for h in &mut hits {
            h.hit.offset = (h.hit.offset + d).max(-1.0);
        }
        cut = cut.map(|c| c + d);
    }
    Applied { hits, cut, humanize }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phrase::resolve_cell_uncached;

    fn res(s: &str) -> CellResolution {
        resolve_cell_uncached(s).unwrap()
    }

    #[test]
    fn parse_e_format() {
        assert_eq!(parse("r4< d+1").unwrap(), vec![Fx::Roll(4, Ramp::Up), Fx::Delay(1)]);
        assert_eq!(normalize(" d-2  v3 ").unwrap().as_deref(), Some("V3 D-2"));
        assert_eq!(normalize("f x").unwrap().as_deref(), Some("F X"));
        assert_eq!(normalize("F3 X7 H0").unwrap().as_deref(), Some("F3 X7 H0"));
        assert_eq!(normalize("").unwrap(), None);
        // a mesma letra fica com a última
        assert_eq!(normalize("R4 R8>").unwrap().as_deref(), Some("R8>"));
        for bad in ["Z", "R1", "R10", "V", "D+6", "D", "F4", "V1 R2 F D+1"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn sem_fx_e_identidade() {
        let r = res("doko");
        let a = apply(&r, &[]);
        assert_eq!(a.hits.iter().map(|h| h.hit).collect::<Vec<_>>(), r.hits);
        assert_eq!((a.cut, a.humanize), (None, 1.0));
    }

    #[test]
    fn rulo_flam_atraso_corte() {
        let r = res("don");
        let a = apply(&r, &parse("R4<").unwrap());
        let offs: Vec<f64> = a.hits.iter().map(|h| h.hit.offset).collect();
        assert_eq!(offs, vec![0.0, 0.25, 0.5, 0.75]);
        let hands: Vec<Hand> = a.hits.iter().map(|h| h.hit.hand).collect();
        assert_eq!(hands, vec![r.hits[0].hand, other(r.hits[0].hand), r.hits[0].hand, other(r.hits[0].hand)]);
        assert!(a.hits[0].hit.vel < a.hits[3].hit.vel, "crescendo");
        // flam: apoio antes, mão oposta, metade da força
        let f = apply(&r, &parse("F").unwrap());
        assert_eq!(f.hits.len(), 2);
        assert!(f.hits[0].grace && !f.hits[1].grace);
        assert!((f.hits[0].hit.offset + 2.0 / 16.0).abs() < 1e-12);
        assert_eq!(f.hits[0].hit.hand, other(r.hits[0].hand));
        assert!((f.hits[0].hit.vel - r.hits[0].vel * 0.5).abs() < 1e-12);
        // atraso desloca golpes e corte
        let d = apply(&r, &parse("D+2 X").unwrap());
        assert!((d.hits[0].hit.offset - 0.2).abs() < 1e-12);
        assert!((d.cut.unwrap() - 0.7).abs() < 1e-12);
        // antecipação não passa da célula anterior
        let e = apply(&r, &parse("F3 D-5").unwrap());
        assert!(e.hits.iter().all(|h| h.hit.offset >= -1.0));
        // velocity e humanização
        let v = apply(&r, &parse("V0 H0").unwrap());
        assert!((v.hits[0].hit.vel - r.hits[0].vel * 0.05).abs() < 1e-12);
        assert_eq!(v.humanize, 0.0);
        // rulo numa sílaba composta usa só o 1º golpe
        assert_eq!(apply(&res("doko"), &parse("R3").unwrap()).hits.len(), 3);
    }
}
