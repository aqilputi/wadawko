//! `SampleBank`: todos os samples decodificados + lookup por
//! (instrumento, zona, camada, round robin).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};

use crate::decode::{SampleData, decode_ogg};
use crate::embedded::SAMPLE_NAMES;
use crate::manifest::{ANIMALESE, BaseInstrument, MANIFEST, Zone, layer_velocity, select_layer};

/// Índice de um sample no banco (posição em [`SAMPLE_NAMES`]). Barato de
/// copiar para dentro de eventos da thread de áudio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SampleId(pub u16);

impl SampleId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
    pub fn name(self) -> &'static str {
        SAMPLE_NAMES[self.index()]
    }
}

/// De onde carregar os `.ogg`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleSource {
    /// bytes embutidos no binário (feature `embed`, padrão).
    Embedded,
    /// diretório com `<nome>.ogg` (mesma convenção de `assets/samples/`).
    /// Todos os arquivos precisam existir.
    Dir(PathBuf),
    /// diretório; arquivos ausentes caem nos embutidos (kit parcial do grupo).
    DirWithFallback(PathBuf),
}

/// Banco de samples decodificado. Imutável depois de carregado; clone barato
/// dos `Arc<SampleData>` para a thread de áudio.
#[derive(Clone, Debug)]
pub struct SampleBank {
    samples: Vec<Arc<SampleData>>,
    /// [instrumento][zona][camada][rr] → SampleId
    table: Vec<[Vec<Vec<SampleId>>; 2]>,
    animalese: SampleId,
}

fn id_of(name: &str) -> Option<SampleId> {
    SAMPLE_NAMES.iter().position(|n| *n == name).map(|i| SampleId(i as u16))
}

fn read_file(dir: &Path, name: &str) -> Result<Vec<u8>> {
    let path = dir.join(format!("{name}.ogg"));
    std::fs::read(&path).with_context(|| format!("abrir {}", path.display()))
}

#[cfg(feature = "embed")]
fn decode_embedded(i: usize) -> Result<SampleData> {
    decode_ogg(crate::embedded::EMBEDDED[i])
}
#[cfg(not(feature = "embed"))]
fn decode_embedded(_i: usize) -> Result<SampleData> {
    bail!("wadawko-samples compilado sem a feature `embed`: informe um diretório de samples (--samples-dir)")
}

fn decode_one(source: &SampleSource, i: usize) -> Result<SampleData> {
    let name = SAMPLE_NAMES[i];
    match source {
        SampleSource::Embedded => decode_embedded(i),
        SampleSource::Dir(dir) => decode_ogg(read_file(dir, name)?),
        SampleSource::DirWithFallback(dir) => {
            if dir.join(format!("{name}.ogg")).is_file() {
                decode_ogg(read_file(dir, name)?)
            } else {
                decode_embedded(i)
            }
        }
    }
    .with_context(|| format!("sample {name}.ogg"))
}

impl SampleBank {
    /// `true` se os samples estão embutidos neste binário (feature `embed`).
    pub const HAS_EMBEDDED: bool = cfg!(feature = "embed");

    /// Carrega dos assets embutidos (erro se compilado sem `embed`).
    pub fn load_embedded() -> Result<Self> {
        Self::load(&SampleSource::Embedded)
    }

    /// Carrega de um diretório (`--samples-dir`); todos os arquivos devem existir.
    pub fn load_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        Self::load(&SampleSource::Dir(dir.into()))
    }

    /// Conveniência para o app: `Some(dir)` → diretório, `None` → embutidos.
    pub fn load_from(samples_dir: Option<&Path>) -> Result<Self> {
        match samples_dir {
            Some(d) => Self::load_dir(d),
            None => Self::load_embedded(),
        }
    }

    /// Decodifica todos os samples em paralelo (bloqueia até terminar; chame
    /// de uma thread de fundo se não quiser travar a UI).
    pub fn load(source: &SampleSource) -> Result<Self> {
        let n = SAMPLE_NAMES.len();
        let workers = std::thread::available_parallelism().map(|p| p.get()).unwrap_or(1).clamp(1, n);
        let next = std::sync::atomic::AtomicUsize::new(0);
        let mut slots: Vec<Option<Result<SampleData>>> = (0..n).map(|_| None).collect();
        let parts: Vec<Vec<(usize, Result<SampleData>)>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    s.spawn(|| {
                        let mut out = Vec::new();
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if i >= n {
                                break out;
                            }
                            out.push((i, decode_one(source, i)));
                        }
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("thread de decodificação entrou em pânico")).collect()
        });
        for (i, r) in parts.into_iter().flatten() {
            slots[i] = Some(r);
        }
        let mut samples = Vec::with_capacity(n);
        for (i, slot) in slots.into_iter().enumerate() {
            let data = slot.ok_or_else(|| anyhow!("sample {} não foi decodificado", SAMPLE_NAMES[i]))??;
            samples.push(Arc::new(data));
        }

        let mut table = Vec::with_capacity(MANIFEST.len());
        for m in &MANIFEST {
            let mut zones: [Vec<Vec<SampleId>>; 2] = [Vec::new(), Vec::new()];
            for zone in Zone::ALL {
                for layer in m.layers(zone) {
                    let mut ids = Vec::with_capacity(layer.len());
                    for name in layer.iter() {
                        ids.push(id_of(name).ok_or_else(|| anyhow!("{name} está no manifesto mas não em SAMPLE_NAMES"))?);
                    }
                    if ids.is_empty() {
                        bail!("camada vazia em {}/{}", m.instrument, zone.as_str());
                    }
                    zones[zone.index()].push(ids);
                }
            }
            table.push(zones);
        }
        let animalese = id_of(ANIMALESE).ok_or_else(|| anyhow!("animalese fora de SAMPLE_NAMES"))?;
        Ok(SampleBank { samples, table, animalese })
    }

    // ───────────── lookup ─────────────

    /// nº de camadas de dinâmica da zona.
    pub fn layer_count(&self, inst: BaseInstrument, zone: Zone) -> usize {
        self.table[inst.index()][zone.index()].len()
    }

    /// nº de round robins da camada (0 se a camada não existe).
    pub fn rr_count(&self, inst: BaseInstrument, zone: Zone, layer: usize) -> usize {
        self.table[inst.index()][zone.index()].get(layer).map_or(0, Vec::len)
    }

    /// Id do sample de `(instrumento, zona, camada, rr)`. `rr` é tomado módulo
    /// o nº de round robins (como `layer.rr % buffers.length` no TS), então
    /// um contador crescente pode ser passado direto. `None` só se a camada
    /// não existe.
    pub fn id(&self, inst: BaseInstrument, zone: Zone, layer: usize, rr: usize) -> Option<SampleId> {
        let l = self.table[inst.index()][zone.index()].get(layer)?;
        Some(l[rr % l.len()])
    }

    /// Sample de `(instrumento, zona, camada, rr)`; mesmas regras de [`Self::id`].
    pub fn get(&self, inst: BaseInstrument, zone: Zone, layer: usize, rr: usize) -> Option<&Arc<SampleData>> {
        self.id(inst, zone, layer, rr).map(|id| &self.samples[id.index()])
    }

    /// Sample por id.
    pub fn sample(&self, id: SampleId) -> &Arc<SampleData> {
        &self.samples[id.index()]
    }

    /// Sample por nome de arquivo sem extensão (ex.: `"chu-head-0-1"`).
    pub fn by_name(&self, name: &str) -> Option<&Arc<SampleData>> {
        id_of(name).map(|id| &self.samples[id.index()])
    }

    /// Id por nome de arquivo sem extensão.
    pub fn id_by_name(&self, name: &str) -> Option<SampleId> {
        id_of(name)
    }

    /// Letras do animalese (vozes dos kiai); ver [`crate::animalese`].
    pub fn animalese(&self) -> &Arc<SampleData> {
        &self.samples[self.animalese.index()]
    }
    pub fn animalese_id(&self) -> SampleId {
        self.animalese
    }

    /// Todos os samples, indexáveis por [`SampleId`] (ordem de [`SAMPLE_NAMES`]).
    pub fn samples(&self) -> &[Arc<SampleData>] {
        &self.samples
    }

    /// nº total de samples (72 golpes + animalese).
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// (nome, sample) de todos os arquivos.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Arc<SampleData>)> {
        SAMPLE_NAMES.iter().copied().zip(self.samples.iter())
    }

    /// PCM total em bytes (diagnóstico).
    pub fn memory_bytes(&self) -> usize {
        self.samples.iter().map(|s| s.frames.len() * size_of::<f32>()).sum()
    }

    // ───────────── seleção de camada ─────────────

    /// Camada da zona para uma velocity EFETIVA (já `min(1, vel·√dyn)`; ver
    /// [`layer_velocity`]): primeira camada com `layer_vel <= maxVel`, senão a última.
    pub fn select_layer(&self, inst: BaseInstrument, zone: Zone, layer_vel: f32) -> usize {
        select_layer(self.layer_count(inst, zone), layer_vel)
    }

    /// Golpe completo como em `playLayered`: escolhe a camada por
    /// `min(1, vel·√dyn)`, pega o round robin atual de `rr` e avança o contador.
    pub fn pick(&self, inst: BaseInstrument, zone: Zone, vel: f32, dynamic: f32, rr: &mut RoundRobin) -> Pick<'_> {
        let layer = self.select_layer(inst, zone, layer_velocity(vel, dynamic));
        let l = &self.table[inst.index()][zone.index()][layer];
        let rr_index = rr.next(zone, layer) % l.len();
        let id = l[rr_index];
        Pick { id, layer, rr: rr_index, data: &self.samples[id.index()] }
    }
}

/// Resultado de [`SampleBank::pick`].
#[derive(Clone, Copy, Debug)]
pub struct Pick<'a> {
    pub id: SampleId,
    pub layer: usize,
    pub rr: usize,
    pub data: &'a Arc<SampleData>,
}

/// Máximo de camadas por zona no manifesto (odaiko/head tem 4).
pub const MAX_LAYERS: usize = 4;

/// Contadores de round robin de UMA voz (no TS: `Layer.rr`, um por camada e
/// zona, por canal — cada linha/dono tem os seus; o preview tem outros).
/// `Copy`, sem alocação: pode viver na thread de áudio ou no compilador de schedule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoundRobin {
    counters: [[u32; MAX_LAYERS]; 2],
}

impl RoundRobin {
    pub const fn new() -> Self {
        RoundRobin { counters: [[0; MAX_LAYERS]; 2] }
    }

    /// Devolve o contador atual de (zona, camada) e o incrementa. Passe o
    /// valor a [`SampleBank::get`], que aplica o módulo.
    pub fn next(&mut self, zone: Zone, layer: usize) -> usize {
        let c = &mut self.counters[zone.index()][layer.min(MAX_LAYERS - 1)];
        let v = *c;
        *c = c.wrapping_add(1);
        v as usize
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
