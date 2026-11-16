#![allow(dead_code)]
use std::sync::OnceLock;

use wadawko_core::Song;
use wadawko_core::model::PROJECT_KEY_PREFIX;
use wadawko_samples::SampleBank;

pub fn bank() -> &'static SampleBank {
    static BANK: OnceLock<SampleBank> = OnceLock::new();
    BANK.get_or_init(|| SampleBank::load_embedded().expect("samples embutidos"))
}

const BACKUP: &str = include_str!("../../../wadawko-core/tests/fixtures/backup-localstorage-2026-10-02.json");

/// as 2 músicas do backup do localStorage (fixtures do wadawko-core), por (chave, música)
pub fn fixtures() -> Vec<(String, Song)> {
    let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(BACKUP).unwrap();
    map.iter()
        .filter(|(k, _)| k.starts_with(PROJECT_KEY_PREFIX))
        .map(|(k, v)| (k.clone(), Song::from_json(v.as_str().unwrap()).unwrap()))
        .collect()
}

/// a música grande do backup (131 compassos, 180 BPM, ritornellos, dinâmicas, hairpins, kit custom)
pub fn big_song() -> Song {
    fixtures().into_iter().map(|(_, s)| s).find(|s| s.measures.len() == 131).unwrap()
}

/// "yagyo" (123 BPM, swing 1, ritornello ×3, 3 instrumentos custom)
pub fn yagyo() -> Song {
    fixtures().into_iter().map(|(_, s)| s).find(|s| s.title == "yagyo").unwrap()
}

/// FNV-1a 64 sobre os bits dos samples
pub fn hash(samples: &[f32]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for s in samples {
        for b in s.to_bits().to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt() as f32
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |a, s| a.max(s.abs()))
}
