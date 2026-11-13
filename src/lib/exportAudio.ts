import { Mp3Encoder } from '@breezystack/lamejs'
import type { Song } from './types'
import { engine } from './audio'

/** float [-1,1] → PCM 16-bit, com ganho de normalização */
function toInt16(data: Float32Array, gain: number): Int16Array {
  const out = new Int16Array(data.length)
  for (let i = 0; i < data.length; i++) {
    const v = Math.max(-1, Math.min(1, data[i] * gain))
    out[i] = v < 0 ? v * 0x8000 : v * 0x7fff
  }
  return out
}

/** codifica um AudioBuffer em MP3 (cede o thread entre blocos p/ não travar a UI) */
async function encodeMp3(buf: AudioBuffer, kbps = 192, onProgress?: (f: number) => void): Promise<Blob> {
  const left = buf.getChannelData(0)
  const right = buf.numberOfChannels > 1 ? buf.getChannelData(1) : left
  // só abaixa se clipar — não infla músicas baixas
  let peak = 0
  for (let i = 0; i < left.length; i++) peak = Math.max(peak, Math.abs(left[i]), Math.abs(right[i]))
  const gain = peak > 0.98 ? 0.98 / peak : 1
  const l16 = toInt16(left, gain)
  const r16 = toInt16(right, gain)

  const enc = new Mp3Encoder(2, buf.sampleRate, kbps)
  const chunks: Uint8Array[] = []
  const block = 1152
  for (let i = 0; i < l16.length; i += block) {
    const out = enc.encodeBuffer(l16.subarray(i, i + block), r16.subarray(i, i + block))
    if (out.length) chunks.push(new Uint8Array(out))
    if ((i / block) % 400 === 0) {
      onProgress?.(i / l16.length)
      await new Promise((r) => setTimeout(r))
    }
  }
  const tail = enc.flush()
  if (tail.length) chunks.push(new Uint8Array(tail))
  return new Blob(chunks as BlobPart[], { type: 'audio/mpeg' })
}

export async function exportMp3(song: Song, onStatus?: (s: string) => void): Promise<void> {
  onStatus?.('renderizando…')
  const buf = await engine.renderOffline(song)
  const blob = await encodeMp3(buf, 192, (f) => onStatus?.(`mp3 ${Math.round(f * 100)}%`))
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${song.title.toLowerCase().replace(/\s+/g, '-') || 'musica'}.mp3`
  a.click()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}
