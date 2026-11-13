//! Música de exemplo "Renshū" (`src/lib/demo.ts`): jiuchi no shime + tema
//! simples no chū + pontuação do ōdaiko. É a música de todo projeto novo.

use crate::model::{InstrumentId, Measure, Song, Track};

const X: &str = ""; // silêncio

fn measure(shime: [&str; 8], chu1: [&str; 8], odaiko: [&str; 8]) -> Measure {
    let row = |cells: [&str; 8]| cells.iter().map(|s| (!s.is_empty()).then(|| s.to_string())).collect::<Vec<_>>();
    Measure {
        beats: 4,
        subdivision: 2,
        cells: [("shime", row(shime)), ("chu1", row(chu1)), ("odaiko", row(odaiko))].into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        ..Default::default()
    }
}

/// Música de exemplo: 3 linhas, 4 compassos 4/4 em colcheias, 100 BPM.
pub fn demo_song() -> Song {
    Song {
        version: 1,
        title: "Renshū".into(),
        tempo: 100.0,
        tracks: vec![
            Track::new("shime", "Shime (jiuchi)", InstrumentId::Shime),
            Track::new("chu1", "Chū 1", InstrumentId::Chu),
            Track::new("odaiko", "Ōdaiko", InstrumentId::Odaiko),
        ],
        measures: vec![
            measure(
                ["ten", "ten", "ten", "ten", "ten", "ten", "ten", "ten"],
                ["DON", X, "don", X, "doko", "don", X, X],
                ["DON", X, X, X, X, X, X, X],
            ),
            measure(
                ["ten", "ten", "ten", "ten", "ten", "ten", "teke", "ten"],
                ["DON", X, "don", X, "ka", "ka", "doko", "don"],
                [X, X, X, X, "DON", X, X, X],
            ),
            measure(
                ["ten", "ten", "ten", "ten", "ten", "ten", "ten", "ten"],
                ["doko", "don", "doko", "don", "DON", X, "tsu", "tsu"],
                ["DON", X, X, X, X, X, X, X],
            ),
            measure(
                ["ten", "teke", "ten", "teke", "ten", "ten", "ten", "ten"],
                ["DON", X, "DON", X, "DON", "doko", "DON", "su"],
                ["DON", X, X, X, "DON", X, X, X],
            ),
        ],
        ..Default::default()
    }
}
