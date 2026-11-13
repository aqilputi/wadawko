//! Decodificação Ogg Vorbis → PCM f32 estéreo intercalado (symphonia).

use std::io::Cursor;

use anyhow::{Context, Result, anyhow, bail};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Um sample decodificado. `frames` é SEMPRE estéreo intercalado (L, R, L, R…);
/// arquivos mono são duplicados nos dois canais.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleData {
    /// PCM f32 estéreo intercalado: `frames.len() == 2 * n_frames`.
    pub frames: Vec<f32>,
    /// taxa de amostragem do arquivo (Hz).
    pub sr: u32,
    /// canais do arquivo original (1 = mono duplicado, 2 = estéreo).
    pub source_channels: u16,
}

impl SampleData {
    /// número de frames estéreo.
    pub fn len_frames(&self) -> usize {
        self.frames.len() / 2
    }
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    /// duração natural em segundos (`buffer.duration` do WebAudio).
    pub fn duration_secs(&self) -> f64 {
        self.len_frames() as f64 / self.sr as f64
    }
    /// frame `i` como `[L, R]`.
    pub fn frame(&self, i: usize) -> [f32; 2] {
        [self.frames[2 * i], self.frames[2 * i + 1]]
    }
}

/// Decodifica um `.ogg` (Vorbis) inteiro que já está em memória.
pub fn decode_ogg<B>(bytes: B) -> Result<SampleData>
where
    B: AsRef<[u8]> + Send + Sync + 'static,
{
    let total_frames = ogg_last_granule(bytes.as_ref());
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    // gapless: respeita o granule position final (mesma duração que o decodeAudioData do browser)
    let fmt_opts = FormatOptions { enable_gapless: true, ..Default::default() };
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &MetadataOptions::default()).context("formato não reconhecido")?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| anyhow!("sem trilha de áudio"))?;
    let track_id = track.id;
    let mut sr = track.codec_params.sample_rate;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).context("codec não suportado")?;

    let mut frames: Vec<f32> = Vec::new();
    let mut source_channels = 0usize;
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(e).context("leitura do pacote"),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue, // pacote corrompido: pula
            Err(e) => return Err(e).context("decodificação"),
        };
        let spec = *decoded.spec();
        let ch = spec.channels.count();
        if ch == 0 {
            bail!("pacote sem canais");
        }
        if source_channels == 0 {
            source_channels = ch;
            sr.get_or_insert(spec.rate);
        } else if source_channels != ch {
            bail!("número de canais muda no meio do arquivo ({source_channels} → {ch})");
        }
        if buf.as_ref().is_none_or(|b| b.capacity() < decoded.capacity() * ch) {
            buf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
        }
        let b = buf.as_mut().unwrap();
        b.copy_interleaved_ref(decoded);
        let s = b.samples();
        match ch {
            2 => frames.extend_from_slice(s),
            1 => frames.extend(s.iter().flat_map(|&x| [x, x])),
            _ => frames.extend(s.chunks_exact(ch).flat_map(|f| [f[0], f[1]])),
        }
    }
    let sr = sr.ok_or_else(|| anyhow!("taxa de amostragem desconhecida"))?;
    // O symphonia nem sempre corta o enchimento do último pacote; o granule
    // position da última página Ogg é o nº real de frames (o que o browser e o
    // ffmpeg reportam como duração).
    if let Some(total) = total_frames
        && total > 0
        && (total as usize) < frames.len() / 2
    {
        frames.truncate(total as usize * 2);
    }
    if frames.is_empty() {
        bail!("arquivo sem áudio");
    }
    Ok(SampleData { frames, sr, source_channels: source_channels as u16 })
}

/// Granule position da última página Ogg (= total de frames PCM num Vorbis
/// de um único stream lógico começando em 0).
fn ogg_last_granule(bytes: &[u8]) -> Option<u64> {
    let pos = bytes.windows(4).rposition(|w| w == b"OggS")?;
    let g = bytes.get(pos + 6..pos + 14)?;
    let g = u64::from_le_bytes(g.try_into().ok()?);
    (g != u64::MAX).then_some(g)
}
