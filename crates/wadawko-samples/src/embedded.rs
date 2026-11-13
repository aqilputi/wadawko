//! Lista única dos arquivos do banco e (feature `embed`) os bytes embutidos,
//! lidos de `public/samples/` na raiz do repositório — sem copiar os arquivos.

/// Chama `$m!` com os nomes (sem extensão) de todos os `.ogg` do banco.
macro_rules! with_sample_names {
    ($m:ident) => {
        $m! {
            "shime-head-0-0", "shime-head-0-1", "shime-head-0-2", "shime-head-0-3",
            "shime-head-1-0", "shime-head-1-1", "shime-head-1-2", "shime-head-1-3",
            "shime-head-2-0", "shime-head-2-1", "shime-head-2-2", "shime-head-2-3",
            "shime-rim-0-0", "shime-rim-0-1", "shime-rim-1-0", "shime-rim-2-0", "shime-rim-2-1",
            "chu-head-0-0", "chu-head-0-1", "chu-head-1-0", "chu-head-1-1",
            "chu-rim-0-0", "chu-rim-0-1", "chu-rim-1-0", "chu-rim-1-1", "chu-rim-1-2", "chu-rim-2-0", "chu-rim-2-1",
            "hira-head-0-0", "hira-head-0-1", "hira-head-0-2", "hira-head-1-0", "hira-head-1-1", "hira-head-1-2",
            "nagado-head-0-0", "nagado-head-0-1", "nagado-head-1-0", "nagado-head-1-1", "nagado-head-1-2",
            "onagado-head-0-0", "onagado-head-0-1", "onagado-head-1-0", "onagado-head-1-1", "onagado-head-2-0",
            "okedo-head-0-0", "okedo-head-0-1", "okedo-head-0-2", "okedo-head-0-3",
            "okedo-head-1-0", "okedo-head-1-1", "okedo-head-1-2", "okedo-head-1-3",
            "okedo-head-2-0", "okedo-head-2-1", "okedo-head-2-2",
            "okedo-rim-0-0", "okedo-rim-0-1", "okedo-rim-0-2", "okedo-rim-0-3",
            "odaiko-head-0-0", "odaiko-head-0-1", "odaiko-head-1-0", "odaiko-head-2-0", "odaiko-head-3-0",
            "kane-head-0-0", "kane-head-0-1", "kane-head-0-2", "kane-head-0-3",
            "kane-rim-0-0", "kane-rim-0-1", "kane-rim-0-2", "kane-rim-0-3",
            "animalese",
        }
    };
}

macro_rules! names_array {
    ($($n:literal),* $(,)?) => { &[$($n),*] };
}

/// Nomes (sem `.ogg`) de todos os samples do banco: os 72 golpes do
/// manifesto (únicos) + `animalese`. O índice aqui é o [`crate::SampleId`].
pub static SAMPLE_NAMES: &[&str] = with_sample_names!(names_array);

#[cfg(feature = "embed")]
macro_rules! bytes_array {
    ($($n:literal),* $(,)?) => {
        &[$(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../public/samples/", $n, ".ogg")) as &[u8]),*]
    };
}

/// Bytes dos `.ogg`, na mesma ordem de [`SAMPLE_NAMES`].
#[cfg(feature = "embed")]
pub static EMBEDDED: &[&[u8]] = with_sample_names!(bytes_array);

/// Texto de `public/samples/CREDITS.md` (obrigação das licenças CC BY / CC BY-SA:
/// a UI precisa mostrar). Embutido sempre, com ou sem a feature `embed`.
pub static CREDITS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../public/samples/CREDITS.md"));
