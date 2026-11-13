import type { Song, Measure } from './types'

function measure(cells: Record<string, (string | null)[]>): Measure {
  return { beats: 4, subdivision: 2, cells }
}

const _ = null

/** Música de exemplo: jiuchi no shime + tema simples no chū + pontuação do ōdaiko */
export function demoSong(): Song {
  return {
    version: 1,
    title: 'Renshū',
    tempo: 100,
    tracks: [
      { id: 'shime', name: 'Shime (jiuchi)', instrument: 'shime', muted: false, solo: false },
      { id: 'chu1', name: 'Chū 1', instrument: 'chu', muted: false, solo: false },
      { id: 'odaiko', name: 'Ōdaiko', instrument: 'odaiko', muted: false, solo: false },
    ],
    measures: [
      measure({
        shime: ['ten', 'ten', 'ten', 'ten', 'ten', 'ten', 'ten', 'ten'],
        chu1: ['DON', _, 'don', _, 'doko', 'don', _, _],
        odaiko: ['DON', _, _, _, _, _, _, _],
      }),
      measure({
        shime: ['ten', 'ten', 'ten', 'ten', 'ten', 'ten', 'teke', 'ten'],
        chu1: ['DON', _, 'don', _, 'ka', 'ka', 'doko', 'don'],
        odaiko: [_, _, _, _, 'DON', _, _, _],
      }),
      measure({
        shime: ['ten', 'ten', 'ten', 'ten', 'ten', 'ten', 'ten', 'ten'],
        chu1: ['doko', 'don', 'doko', 'don', 'DON', _, 'tsu', 'tsu'],
        odaiko: ['DON', _, _, _, _, _, _, _],
      }),
      measure({
        shime: ['ten', 'teke', 'ten', 'teke', 'ten', 'ten', 'ten', 'ten'],
        chu1: ['DON', _, 'DON', _, 'DON', 'doko', 'DON', 'su'],
        odaiko: ['DON', _, _, _, 'DON', _, _, _],
      }),
    ],
  }
}
