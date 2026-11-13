//! Metadados SMuFL (Bravura): âncoras de glifos, caixas e padrões de engraving, em structs
//! tipadas, mais os codepoints do subconjunto usado pela partitura do WaDAWko.
//!
//! Unidades: tudo no JSON está em **staff spaces** (1 sp = distância entre duas linhas de um
//! pentagrama = em/4), com **y para cima**. Use [`Sp::to_px`] para converter em deslocamento
//! de tela (y para baixo) relativo à origem do glifo.

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};

use serde::Deserialize;

use crate::error::{DrawError, Result};
use crate::font::{FontBook, FontId};
use crate::geom::{Point, Rect, pt};

/// Nomes dos arquivos em `assets/fonts/`.
pub const BRAVURA_FONT_FILE: &str = "Bravura.otf";
pub const BRAVURA_METADATA_FILE: &str = "bravura_metadata.json";
pub const ZEN_KAKU_REGULAR_FILE: &str = "ZenKakuGothicNew-Regular.ttf";
pub const ZEN_KAKU_MEDIUM_FILE: &str = "ZenKakuGothicNew-Medium.ttf";
pub const ZEN_KAKU_BOLD_FILE: &str = "ZenKakuGothicNew-Bold.ttf";
pub const SHIPPORI_MINCHO_BOLD_FILE: &str = "ShipporiMincho-Bold.ttf";

/// Famílias (como lidas das fontes) para usar em [`crate::TextStyle`].
pub const FAMILY_ZEN_KAKU: &str = "Zen Kaku Gothic New";
pub const FAMILY_SHIPPORI_MINCHO: &str = "Shippori Mincho";
pub const FAMILY_BRAVURA: &str = "Bravura";

/// Coordenada SMuFL em staff spaces, y para cima.
#[derive(Clone, Copy, Debug, PartialEq, Default, Deserialize)]
#[serde(from = "[f32; 2]")]
pub struct Sp {
    pub x: f32,
    pub y: f32,
}

impl From<[f32; 2]> for Sp {
    fn from(v: [f32; 2]) -> Self {
        Sp { x: v[0], y: v[1] }
    }
}

impl Sp {
    /// Deslocamento em px (y para baixo) relativo à origem do glifo, dado o tamanho de um
    /// staff space em px.
    pub fn to_px(self, staff_space: f32) -> Point {
        pt(self.x * staff_space, -self.y * staff_space)
    }
}

/// Caixa de um glifo (`glyphBBoxes`), em staff spaces.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct GlyphBBox {
    #[serde(rename = "bBoxNE")]
    pub ne: Sp,
    #[serde(rename = "bBoxSW")]
    pub sw: Sp,
}

impl GlyphBBox {
    pub fn width(&self) -> f32 {
        self.ne.x - self.sw.x
    }
    pub fn height(&self) -> f32 {
        self.ne.y - self.sw.y
    }
    /// Retângulo em px (y para baixo) relativo à origem do glifo.
    pub fn to_px(&self, staff_space: f32) -> Rect {
        Rect::from_ltrb(
            self.sw.x * staff_space,
            -self.ne.y * staff_space,
            self.ne.x * staff_space,
            -self.sw.y * staff_space,
        )
    }
}

/// Âncoras de um glifo (`glyphsWithAnchors`). Campos ausentes no JSON ficam `None`.
#[derive(Clone, Debug, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct GlyphAnchors {
    /// Canto inferior direito da haste para cima (onde a haste encosta na cabeça).
    #[serde(rename = "stemUpSE")]
    pub stem_up_se: Option<Sp>,
    /// Canto superior esquerdo da haste para baixo.
    #[serde(rename = "stemDownNW")]
    pub stem_down_nw: Option<Sp>,
    /// Em bandeirolas: onde a bandeirola para cima prende na ponta da haste.
    #[serde(rename = "stemUpNW")]
    pub stem_up_nw: Option<Sp>,
    /// Em bandeirolas: onde a bandeirola para baixo prende na ponta da haste.
    #[serde(rename = "stemDownSW")]
    pub stem_down_sw: Option<Sp>,
    #[serde(rename = "splitStemUpSE")]
    pub split_stem_up_se: Option<Sp>,
    #[serde(rename = "splitStemUpSW")]
    pub split_stem_up_sw: Option<Sp>,
    #[serde(rename = "splitStemDownNE")]
    pub split_stem_down_ne: Option<Sp>,
    #[serde(rename = "splitStemDownNW")]
    pub split_stem_down_nw: Option<Sp>,
    #[serde(rename = "cutOutNE")]
    pub cut_out_ne: Option<Sp>,
    #[serde(rename = "cutOutNW")]
    pub cut_out_nw: Option<Sp>,
    #[serde(rename = "cutOutSE")]
    pub cut_out_se: Option<Sp>,
    #[serde(rename = "cutOutSW")]
    pub cut_out_sw: Option<Sp>,
    #[serde(rename = "graceNoteSlashNE")]
    pub grace_note_slash_ne: Option<Sp>,
    #[serde(rename = "graceNoteSlashNW")]
    pub grace_note_slash_nw: Option<Sp>,
    #[serde(rename = "graceNoteSlashSE")]
    pub grace_note_slash_se: Option<Sp>,
    #[serde(rename = "graceNoteSlashSW")]
    pub grace_note_slash_sw: Option<Sp>,
    #[serde(rename = "noteheadOrigin")]
    pub notehead_origin: Option<Sp>,
    /// Centro óptico (dinâmicas: alinhar com o centro da cabeça).
    #[serde(rename = "opticalCenter")]
    pub optical_center: Option<Sp>,
    #[serde(rename = "repeatOffset")]
    pub repeat_offset: Option<Sp>,
    #[serde(rename = "numeralTop")]
    pub numeral_top: Option<Sp>,
    #[serde(rename = "numeralBottom")]
    pub numeral_bottom: Option<Sp>,
    /// Âncoras não mapeadas acima (ex.: `mark0`).
    #[serde(flatten)]
    pub other: HashMap<String, Sp>,
}

/// `engravingDefaults`: espessuras e distâncias recomendadas, em staff spaces.
/// Campos ausentes no JSON assumem os valores da Bravura.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EngravingDefaults {
    pub arrow_shaft_thickness: f32,
    pub barline_separation: f32,
    pub beam_spacing: f32,
    pub beam_thickness: f32,
    pub bracket_thickness: f32,
    pub dashed_barline_dash_length: f32,
    pub dashed_barline_gap_length: f32,
    pub dashed_barline_thickness: f32,
    pub h_bar_thickness: f32,
    pub hairpin_thickness: f32,
    pub leger_line_extension: f32,
    pub leger_line_thickness: f32,
    pub lyric_line_thickness: f32,
    pub octave_line_thickness: f32,
    pub pedal_line_thickness: f32,
    pub repeat_barline_dot_separation: f32,
    pub repeat_ending_line_thickness: f32,
    pub slur_endpoint_thickness: f32,
    pub slur_midpoint_thickness: f32,
    pub staff_line_thickness: f32,
    pub stem_thickness: f32,
    pub sub_bracket_thickness: f32,
    pub text_enclosure_thickness: f32,
    pub thick_barline_thickness: f32,
    pub thin_barline_thickness: f32,
    pub thin_thick_barline_separation: f32,
    pub tie_endpoint_thickness: f32,
    pub tie_midpoint_thickness: f32,
    pub tuplet_bracket_thickness: f32,
    pub text_font_family: Vec<String>,
}

impl Default for EngravingDefaults {
    fn default() -> Self {
        EngravingDefaults {
            arrow_shaft_thickness: 0.16,
            barline_separation: 0.4,
            beam_spacing: 0.25,
            beam_thickness: 0.5,
            bracket_thickness: 0.5,
            dashed_barline_dash_length: 0.5,
            dashed_barline_gap_length: 0.25,
            dashed_barline_thickness: 0.16,
            h_bar_thickness: 1.0,
            hairpin_thickness: 0.16,
            leger_line_extension: 0.4,
            leger_line_thickness: 0.16,
            lyric_line_thickness: 0.16,
            octave_line_thickness: 0.16,
            pedal_line_thickness: 0.16,
            repeat_barline_dot_separation: 0.16,
            repeat_ending_line_thickness: 0.16,
            slur_endpoint_thickness: 0.1,
            slur_midpoint_thickness: 0.22,
            staff_line_thickness: 0.13,
            stem_thickness: 0.12,
            sub_bracket_thickness: 0.16,
            text_enclosure_thickness: 0.16,
            thick_barline_thickness: 0.5,
            thin_barline_thickness: 0.16,
            thin_thick_barline_separation: 0.4,
            tie_endpoint_thickness: 0.1,
            tie_midpoint_thickness: 0.22,
            tuplet_bracket_thickness: 0.16,
            text_font_family: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMetadata {
    #[serde(default)]
    font_name: String,
    #[serde(default)]
    font_version: serde_json::Value,
    #[serde(default)]
    engraving_defaults: EngravingDefaults,
    #[serde(default)]
    glyph_advance_widths: HashMap<String, f32>,
    #[serde(default, rename = "glyphBBoxes")]
    glyph_bboxes: HashMap<String, GlyphBBox>,
    #[serde(default)]
    glyphs_with_anchors: HashMap<String, GlyphAnchors>,
}

/// Metadados de uma fonte SMuFL (o `bravura_metadata.json`), indexados pelo **nome canônico**
/// do glifo (ex.: `"noteheadBlack"`).
#[derive(Clone, Debug, PartialEq)]
pub struct SmuflMetadata {
    pub font_name: String,
    /// Versão como texto (a Bravura grava um número, ex.: `"1.392"`).
    pub font_version: String,
    pub engraving_defaults: EngravingDefaults,
    /// Avanço de cada glifo, em staff spaces.
    pub glyph_advance_widths: HashMap<String, f32>,
    pub glyph_bboxes: HashMap<String, GlyphBBox>,
    pub glyphs_with_anchors: HashMap<String, GlyphAnchors>,
}

impl SmuflMetadata {
    pub fn from_json_str(json: &str) -> Result<Self> {
        let raw: RawMetadata =
            serde_json::from_str(json).map_err(|e| DrawError::Metadata(e.to_string()))?;
        let font_version = match raw.font_version {
            serde_json::Value::String(s) => s,
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        Ok(SmuflMetadata {
            font_name: raw.font_name,
            font_version,
            engraving_defaults: raw.engraving_defaults,
            glyph_advance_widths: raw.glyph_advance_widths,
            glyph_bboxes: raw.glyph_bboxes,
            glyphs_with_anchors: raw.glyphs_with_anchors,
        })
    }

    pub fn from_json_slice(json: &[u8]) -> Result<Self> {
        let s = std::str::from_utf8(json).map_err(|e| DrawError::Metadata(e.to_string()))?;
        Self::from_json_str(s)
    }

    pub fn load(path: impl AsRef<FsPath>) -> Result<Self> {
        let path = path.as_ref();
        let s = std::fs::read_to_string(path)
            .map_err(|source| DrawError::Io { path: path.to_path_buf(), source })?;
        Self::from_json_str(&s)
    }

    /// Âncoras do glifo (por nome SMuFL), se houver.
    pub fn anchors(&self, glyph_name: &str) -> Option<&GlyphAnchors> {
        self.glyphs_with_anchors.get(glyph_name)
    }
    /// Caixa do glifo em staff spaces.
    pub fn bbox(&self, glyph_name: &str) -> Option<GlyphBBox> {
        self.glyph_bboxes.get(glyph_name).copied()
    }
    /// Avanço do glifo em staff spaces.
    pub fn advance(&self, glyph_name: &str) -> Option<f32> {
        self.glyph_advance_widths.get(glyph_name).copied()
    }
}

/// Uma fonte SMuFL registrada num [`FontBook`] junto de seus metadados.
#[derive(Clone, Debug)]
pub struct SmuflFont {
    pub font: FontId,
    pub metadata: SmuflMetadata,
}

impl SmuflFont {
    /// Registra a fonte (bytes) e interpreta o JSON de metadados — para `include_bytes!`.
    pub fn from_bytes(
        book: &mut FontBook,
        font_data: impl Into<std::borrow::Cow<'static, [u8]>>,
        metadata_json: &[u8],
    ) -> Result<Self> {
        let metadata = SmuflMetadata::from_json_slice(metadata_json)?;
        let font = book.add(font_data)?;
        Ok(SmuflFont { font, metadata })
    }

    /// Carrega fonte + metadados de arquivos.
    pub fn load(
        book: &mut FontBook,
        font_path: impl AsRef<FsPath>,
        metadata_path: impl AsRef<FsPath>,
    ) -> Result<Self> {
        let metadata = SmuflMetadata::load(metadata_path)?;
        let font = book.load_file(font_path)?;
        Ok(SmuflFont { font, metadata })
    }

    /// Carrega `Bravura.otf` + `bravura_metadata.json` de um diretório (ex.: `assets/fonts`).
    pub fn load_bravura(book: &mut FontBook, fonts_dir: impl AsRef<FsPath>) -> Result<Self> {
        let dir = fonts_dir.as_ref();
        Self::load(book, dir.join(BRAVURA_FONT_FILE), dir.join(BRAVURA_METADATA_FILE))
    }

    /// Tamanho de fonte (px) para que 1 staff space meça `staff_space` px. (SMuFL: em = 4 sp.)
    pub fn font_size(staff_space: f32) -> f32 {
        staff_space * 4.0
    }
    /// Staff space (px) correspondente a um tamanho de fonte.
    pub fn staff_space(font_size: f32) -> f32 {
        font_size / 4.0
    }

    pub fn anchors(&self, glyph_name: &str) -> Option<&GlyphAnchors> {
        self.metadata.anchors(glyph_name)
    }
    /// Caixa do glifo em px (y para baixo, relativa à origem do glifo).
    pub fn bbox_px(&self, glyph_name: &str, staff_space: f32) -> Option<Rect> {
        self.metadata.bbox(glyph_name).map(|b| b.to_px(staff_space))
    }
    /// Avanço do glifo em px.
    pub fn advance_px(&self, glyph_name: &str, staff_space: f32) -> Option<f32> {
        self.metadata.advance(glyph_name).map(|a| a * staff_space)
    }
    pub fn defaults(&self) -> &EngravingDefaults {
        &self.metadata.engraving_defaults
    }
}

/// Um glifo SMuFL: nome canônico (chave nos metadados) + codepoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SmuflGlyph {
    pub name: &'static str,
    pub codepoint: char,
}

/// Subconjunto de glifos SMuFL usado pela partitura (PLANEJAMENTO-gpui.md §5).
/// Os nomes/codepoints são conferidos contra a Bravura nos testes do crate.
pub mod glyphs {
    use super::SmuflGlyph;

    macro_rules! smufl_glyphs {
        ($($konst:ident = $name:literal, $cp:literal;)*) => {
            $(pub const $konst: SmuflGlyph = SmuflGlyph { name: $name, codepoint: $cp };)*
            /// Todos os glifos declarados neste módulo.
            pub const ALL: &[SmuflGlyph] = &[$($konst),*];
        };
    }

    smufl_glyphs! {
        // barras de compasso e repetição
        BARLINE_SINGLE = "barlineSingle", '\u{E030}';
        BARLINE_DOUBLE = "barlineDouble", '\u{E031}';
        BARLINE_FINAL = "barlineFinal", '\u{E032}';
        REPEAT_LEFT = "repeatLeft", '\u{E040}';
        REPEAT_RIGHT = "repeatRight", '\u{E041}';
        REPEAT_DOTS = "repeatDots", '\u{E043}';
        REPEAT_DOT = "repeatDot", '\u{E044}';
        // clave de percussão e fórmula de compasso
        PERCUSSION_CLEF_1 = "unpitchedPercussionClef1", '\u{E069}';
        TIME_SIG_0 = "timeSig0", '\u{E080}';
        TIME_SIG_1 = "timeSig1", '\u{E081}';
        TIME_SIG_2 = "timeSig2", '\u{E082}';
        TIME_SIG_3 = "timeSig3", '\u{E083}';
        TIME_SIG_4 = "timeSig4", '\u{E084}';
        TIME_SIG_5 = "timeSig5", '\u{E085}';
        TIME_SIG_6 = "timeSig6", '\u{E086}';
        TIME_SIG_7 = "timeSig7", '\u{E087}';
        TIME_SIG_8 = "timeSig8", '\u{E088}';
        TIME_SIG_9 = "timeSig9", '\u{E089}';
        // cabeças de nota
        NOTEHEAD_WHOLE = "noteheadWhole", '\u{E0A2}';
        NOTEHEAD_HALF = "noteheadHalf", '\u{E0A3}';
        NOTEHEAD_BLACK = "noteheadBlack", '\u{E0A4}';
        NOTEHEAD_X_BLACK = "noteheadXBlack", '\u{E0A9}';
        AUGMENTATION_DOT = "augmentationDot", '\u{E1E7}';
        // bandeirolas
        FLAG_8TH_UP = "flag8thUp", '\u{E240}';
        FLAG_8TH_DOWN = "flag8thDown", '\u{E241}';
        FLAG_16TH_UP = "flag16thUp", '\u{E242}';
        FLAG_16TH_DOWN = "flag16thDown", '\u{E243}';
        FLAG_32ND_UP = "flag32ndUp", '\u{E244}';
        FLAG_32ND_DOWN = "flag32ndDown", '\u{E245}';
        // articulações
        ACCENT_ABOVE = "articAccentAbove", '\u{E4A0}';
        ACCENT_BELOW = "articAccentBelow", '\u{E4A1}';
        STACCATO_ABOVE = "articStaccatoAbove", '\u{E4A2}';
        MARCATO_ABOVE = "articMarcatoAbove", '\u{E4AC}';
        // pausas
        REST_WHOLE = "restWhole", '\u{E4E3}';
        REST_HALF = "restHalf", '\u{E4E4}';
        REST_QUARTER = "restQuarter", '\u{E4E5}';
        REST_8TH = "rest8th", '\u{E4E6}';
        REST_16TH = "rest16th", '\u{E4E7}';
        REST_32ND = "rest32nd", '\u{E4E8}';
        // dinâmicas
        DYNAMIC_PIANO = "dynamicPiano", '\u{E520}';
        DYNAMIC_MEZZO = "dynamicMezzo", '\u{E521}';
        DYNAMIC_FORTE = "dynamicForte", '\u{E522}';
        DYNAMIC_SFORZANDO = "dynamicSforzando", '\u{E524}';
        DYNAMIC_Z = "dynamicZ", '\u{E525}';
        DYNAMIC_PPP = "dynamicPPP", '\u{E52A}';
        DYNAMIC_PP = "dynamicPP", '\u{E52B}';
        DYNAMIC_MP = "dynamicMP", '\u{E52C}';
        DYNAMIC_MF = "dynamicMF", '\u{E52D}';
        DYNAMIC_FF = "dynamicFF", '\u{E52F}';
        DYNAMIC_FFF = "dynamicFFF", '\u{E530}';
        DYNAMIC_SF = "dynamicSforzando1", '\u{E536}';
        DYNAMIC_SFZ = "dynamicSforzato", '\u{E539}';
        // quiálteras
        TUPLET_0 = "tuplet0", '\u{E880}';
        TUPLET_1 = "tuplet1", '\u{E881}';
        TUPLET_2 = "tuplet2", '\u{E882}';
        TUPLET_3 = "tuplet3", '\u{E883}';
        TUPLET_4 = "tuplet4", '\u{E884}';
        TUPLET_5 = "tuplet5", '\u{E885}';
        TUPLET_6 = "tuplet6", '\u{E886}';
        TUPLET_7 = "tuplet7", '\u{E887}';
        TUPLET_8 = "tuplet8", '\u{E888}';
        TUPLET_9 = "tuplet9", '\u{E889}';
    }

    /// Procura no subconjunto pelo nome canônico.
    pub fn by_name(name: &str) -> Option<SmuflGlyph> {
        ALL.iter().copied().find(|g| g.name == name)
    }
}

/// As fontes padrão do WaDAWko carregadas de um diretório (ver [`StandardFonts::load`]).
#[derive(Debug)]
pub struct StandardFonts {
    pub book: FontBook,
    pub zen_regular: FontId,
    pub zen_medium: FontId,
    pub zen_bold: FontId,
    pub mincho_bold: FontId,
    pub bravura: SmuflFont,
}

impl StandardFonts {
    /// Carrega Zen Kaku Gothic New (Regular/Medium/Bold), Shippori Mincho Bold e Bravura
    /// (+ metadados) de `fonts_dir` e fixa os nomes de família nas constantes `FAMILY_*`.
    pub fn load(fonts_dir: impl AsRef<FsPath>) -> Result<Self> {
        let dir = fonts_dir.as_ref();
        let mut book = FontBook::new();
        let zen_regular = book.load_file(dir.join(ZEN_KAKU_REGULAR_FILE))?;
        let zen_medium = book.load_file(dir.join(ZEN_KAKU_MEDIUM_FILE))?;
        let zen_bold = book.load_file(dir.join(ZEN_KAKU_BOLD_FILE))?;
        let mincho_bold = book.load_file(dir.join(SHIPPORI_MINCHO_BOLD_FILE))?;
        let bravura = SmuflFont::load_bravura(&mut book, dir)?;
        for id in [zen_regular, zen_medium, zen_bold] {
            book.set_family(id, FAMILY_ZEN_KAKU);
        }
        book.set_family(mincho_bold, FAMILY_SHIPPORI_MINCHO);
        book.set_family(bravura.font, FAMILY_BRAVURA);
        Ok(StandardFonts { book, zen_regular, zen_medium, zen_bold, mincho_bold, bravura })
    }
}

/// `assets/fonts/` do workspace, resolvido a partir do diretório deste crate em tempo de
/// compilação — só para desenvolvimento/testes/CLI rodando dentro do repositório. O app
/// distribuído deve embutir os bytes (`include_bytes!`) e usar [`FontBook::add`] /
/// [`SmuflFont::from_bytes`].
pub fn dev_fonts_dir() -> PathBuf {
    FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts")
}
