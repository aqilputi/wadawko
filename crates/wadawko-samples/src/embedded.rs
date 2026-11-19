//! Lista única dos arquivos do banco e (feature `embed`) os bytes embutidos,
//! lidos de `assets/samples/` na raiz do repositório — sem copiar os arquivos.

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
            "kiai/m-ha-0", "kiai/m-ha-1", "kiai/m-ha-2", "kiai/m-ha-3", "kiai/m-hai-0", "kiai/m-hai-1",
            "kiai/m-yo-0", "kiai/m-yo-1", "kiai/m-yo-2", "kiai/m-iyo-0", "kiai/m-iyo-1", "kiai/m-iya-0",
            "kiai/m-iya-1", "kiai/m-haiya-0", "kiai/m-ya-0", "kiai/m-ei-0", "kiai/m-orya-0", "kiai/f-hai-0",
            "kiai/f-hai-1", "kiai/f-hai-2", "kiai/f-hai-3", "kiai/f-ha-0", "kiai/f-ha-1", "kiai/f-ha-2",
            "kiai/f-ha-3", "kiai/f-ha-4", "kiai/f-sa-0", "kiai/f-iya-0", "kiai/f-iya-1", "kiai/f-hya-0",
            "kiai/f-hyo-0", "kiai/f-ya-0", "kiai/f-ho-0", "kiai/m-count-ichi-0", "kiai/m-count-ni-0", "kiai/m-count-san-0",
            "kiai/m-count-yon-0", "kiai/m-count-go-0", "kiai/m-count-roku-0", "kiai/m-count-nana-0", "kiai/m-count-hachi-0", "kiai/f-count-ichi-0",
            "kiai/f-count-ni-0", "kiai/f-count-san-0", "kiai/f-count-shi-0", "kiai/f-count-go-0", "kiai/f-count-roku-0", "kiai/f-count-shichi-0",
            "kiai/f-count-hachi-0",
        }
    };
}

macro_rules! names_array {
    ($($n:literal),* $(,)?) => { &[$($n),*] };
}

/// Nomes (sem `.ogg`) de todos os samples do banco: os 72 golpes do
/// manifesto (únicos) + as gravações de kiai/contagem (`kiai/…`, ver [`crate::kiai`]). O índice aqui é o [`crate::SampleId`].
pub static SAMPLE_NAMES: &[&str] = with_sample_names!(names_array);

#[cfg(feature = "embed")]
macro_rules! bytes_array {
    ($($n:literal),* $(,)?) => {
        &[$(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/samples/", $n, ".ogg")) as &[u8]),*]
    };
}

/// Bytes dos `.ogg`, na mesma ordem de [`SAMPLE_NAMES`].
#[cfg(feature = "embed")]
pub static EMBEDDED: &[&[u8]] = with_sample_names!(bytes_array);

/// Texto de `assets/samples/CREDITS.md` (obrigação das licenças CC BY / CC BY-SA:
/// a UI precisa mostrar). Embutido sempre, com ou sem a feature `embed`.
pub static CREDITS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/samples/CREDITS.md"));
