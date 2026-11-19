use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use wadawko_samples::*;

fn bank() -> &'static SampleBank {
    static BANK: std::sync::OnceLock<SampleBank> = std::sync::OnceLock::new();
    BANK.get_or_init(|| SampleBank::load_dir(repo_samples_dir()).expect("carregar assets/samples"))
}

fn manifest_names() -> BTreeSet<&'static str> {
    MANIFEST.iter().flat_map(|m| m.head.iter().chain(m.rim.iter())).flat_map(|l| l.iter().copied()).collect()
}

#[test]
fn instrument_ids_roundtrip() {
    let ids = ["shime", "chu", "hira", "nagado", "onagado", "okedo", "odaiko", "kane"];
    assert_eq!(BaseInstrument::ALL.map(|i| i.as_str()), ids);
    for (k, i) in BaseInstrument::ALL.into_iter().enumerate() {
        assert_eq!(i.index(), k);
        assert_eq!(BaseInstrument::from_str(i.as_str()), Ok(i));
        assert_eq!(i.manifest().instrument, i);
    }
    assert!(BaseInstrument::from_str("taiko").is_err());
    assert_eq!("rim".parse::<Zone>(), Ok(Zone::Rim));
    assert_eq!("head".parse::<Zone>(), Ok(Zone::Head));
}

#[test]
fn file_list_matches_manifest_and_disk() {
    let names = manifest_names();
    assert_eq!(names.len(), 72, "golpes únicos no manifesto");
    let mut all: BTreeSet<&str> = names.clone();
    all.extend(kiai::TAKES.iter().map(|t| t.name));
    assert_eq!(all.len(), 72 + kiai::TAKES.len());
    assert_eq!(SAMPLE_NAMES.len(), all.len(), "SAMPLE_NAMES sem duplicatas");
    assert_eq!(SAMPLE_NAMES.iter().copied().collect::<BTreeSet<_>>(), all);

    // o diretório não tem .ogg sobrando nem faltando
    let ogg = |dir: std::path::PathBuf, prefix: &str| -> Vec<String> {
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).filter_map(|f| f.strip_suffix(".ogg").map(|n| format!("{prefix}{n}"))).collect()
    };
    let mut on_disk: BTreeSet<String> = ogg(repo_samples_dir(), "").into_iter().collect();
    on_disk.extend(ogg(repo_samples_dir().join("kiai"), "kiai/"));
    assert_eq!(on_disk, all.iter().map(|s| s.to_string()).collect());

    // convenção de nomes: <instrumento>-<zona>-<camada>-<rr>
    for m in &MANIFEST {
        for zone in Zone::ALL {
            for (li, layer) in m.layers(zone).iter().enumerate() {
                assert!(!layer.is_empty());
                assert!(m.layers(zone).len() <= MAX_LAYERS);
                for (ri, name) in layer.iter().enumerate() {
                    let owner = name.split('-').next().unwrap();
                    assert!(name.ends_with(&format!("-{}-{li}-{ri}", zone.as_str())), "{name}");
                    // só a borda pode vir de outro instrumento (chu)
                    if owner != m.instrument.as_str() {
                        assert_eq!((owner, zone), ("chu", Zone::Rim), "{name} em {}", m.instrument);
                    }
                }
            }
        }
    }
}

#[test]
fn rim_reuse_matches_audio_ts() {
    use BaseInstrument::*;
    let chu_rim = Chu.manifest().rim;
    for i in [Hira, Nagado, Onagado, Odaiko] {
        assert_eq!(i.manifest().rim, chu_rim, "{i}");
    }
    for i in [Shime, Okedo, Kane] {
        assert_ne!(i.manifest().rim, chu_rim, "{i}");
    }
    let b = bank();
    assert!(Arc::ptr_eq(b.get(Onagado, Zone::Rim, 1, 2).unwrap(), b.get(Chu, Zone::Rim, 1, 2).unwrap()));
    assert!(Arc::ptr_eq(b.get(Onagado, Zone::Rim, 1, 2).unwrap(), b.by_name("chu-rim-1-2").unwrap()));
}

#[test]
fn manifest_shape() {
    use BaseInstrument::*;
    let shape = |i: BaseInstrument, z: Zone| i.manifest().layers(z).iter().map(|l| l.len()).collect::<Vec<_>>();
    assert_eq!(shape(Shime, Zone::Head), [4, 4, 4]);
    assert_eq!(shape(Shime, Zone::Rim), [2, 1, 2]);
    assert_eq!(shape(Chu, Zone::Head), [2, 2]);
    assert_eq!(shape(Chu, Zone::Rim), [2, 3, 2]);
    assert_eq!(shape(Hira, Zone::Head), [3, 3]);
    assert_eq!(shape(Nagado, Zone::Head), [2, 3]);
    assert_eq!(shape(Onagado, Zone::Head), [2, 2, 1]);
    assert_eq!(shape(Okedo, Zone::Head), [4, 4, 3]);
    assert_eq!(shape(Okedo, Zone::Rim), [4]);
    assert_eq!(shape(Odaiko, Zone::Head), [2, 1, 1, 1]);
    assert_eq!(shape(Kane, Zone::Head), [4]);
    assert_eq!(shape(Kane, Zone::Rim), [4]);
}

#[test]
fn tuning_tail_eq_spot_checks() {
    use BaseInstrument::*;
    let t = Hira.manifest().tuning;
    assert_eq!((t.head_gain, t.rim_gain, t.rim_rate, t.head_rate), (1.1, 0.95, 1.12, 1.0));
    assert_eq!(Okedo.manifest().tuning.gain(Zone::Head), 1.8);
    assert_eq!(Odaiko.manifest().tuning.rate(Zone::Rim), 0.9);
    assert_eq!(Onagado.manifest().tuning.rate(Zone::Rim), 0.94);
    let tl = Odaiko.manifest().tail;
    assert_eq!((tl.head, tl.rim, tl.choke), (2.8, 0.8, 0.14));
    assert_eq!(Chu.manifest().tail, Tail { head: 1.3, rim: 0.7, choke: 0.09 });
    assert_eq!(Shime.manifest().tail.max(Zone::Rim), 0.35);

    let n = |i: BaseInstrument, z: Zone| i.manifest().eq(z).len();
    let counts: Vec<(usize, usize)> = BaseInstrument::ALL.iter().map(|&i| (n(i, Zone::Head), n(i, Zone::Rim))).collect();
    //                 shime   chu     hira    nagado  onagado okedo   odaiko  kane
    assert_eq!(counts, [(2, 1), (4, 2), (3, 2), (4, 2), (4, 1), (3, 1), (3, 1), (1, 1)]);
    assert_eq!(Chu.manifest().eq_head[1], EqBand { kind: FilterKind::Lowshelf, freq: 130.0, gain_db: -3.0, q: 0.7 });
    assert_eq!(Onagado.manifest().eq_head[3], EqBand { kind: FilterKind::Peaking, freq: 2200.0, gain_db: 2.5, q: 1.0 });
    assert_eq!(Kane.manifest().eq_rim[0], EqBand { kind: FilterKind::Highpass, freq: 700.0, gain_db: 0.0, q: 0.7 });
    for m in &MANIFEST {
        for z in Zone::ALL {
            // toda cadeia começa por um highpass com Q 0.7
            let first = m.eq(z)[0];
            assert_eq!((first.kind, first.gain_db, first.q), (FilterKind::Highpass, 0.0, 0.7));
        }
    }
}

#[test]
fn all_files_decode() {
    let b = bank();
    assert_eq!(b.len(), 72 + kiai::TAKES.len());
    assert_eq!(b.samples().len(), b.len());
    for (name, s) in b.iter() {
        assert_eq!(s.sr, 48_000, "{name}");
        assert_eq!(s.frames.len() % 2, 0, "{name}");
        assert!(s.len_frames() > 0 && s.duration_secs() > 0.02, "{name}: {}", s.duration_secs());
        assert!(s.duration_secs() < 6.0, "{name}: {}", s.duration_secs());
        assert!(s.frames.iter().all(|x| x.is_finite()), "{name}");
        let peak = s.frames.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.01 && peak <= 1.5, "{name}: pico {peak}");
        assert_eq!(s.source_channels, 2, "{name} (CREDITS: todos estéreo)");
    }
    // kiai: gravações curtas (até 1,2 s)
    for t in kiai::TAKES {
        assert!(b.by_name(t.name).unwrap().duration_secs() <= 1.25, "{}", t.name);
    }
    assert!(b.memory_bytes() > 1_000_000);
    eprintln!("banco: {:.1} MB de PCM", b.memory_bytes() as f64 / 1e6);
    // nº de frames = duration_ts do ffprobe (granule position final do Ogg)
    for (n, frames) in [("chu-head-0-0", 90489), ("odaiko-head-3-0", 240000), ("kane-rim-0-0", 16426), ("shime-head-0-0", 23040)] {
        assert_eq!(b.by_name(n).unwrap().len_frames(), frames, "{n}");
    }
}

#[test]
fn lookup_covers_every_manifest_entry() {
    let b = bank();
    let mut seen = BTreeSet::new();
    for m in &MANIFEST {
        for zone in Zone::ALL {
            assert_eq!(b.layer_count(m.instrument, zone), m.layers(zone).len());
            for (li, layer) in m.layers(zone).iter().enumerate() {
                assert_eq!(b.rr_count(m.instrument, zone, li), layer.len());
                for (ri, name) in layer.iter().enumerate() {
                    let id = b.id(m.instrument, zone, li, ri).unwrap();
                    assert_eq!(id.name(), *name);
                    assert_eq!(b.id_by_name(name), Some(id));
                    assert!(Arc::ptr_eq(b.get(m.instrument, zone, li, ri).unwrap(), b.sample(id)));
                    seen.insert(id);
                }
                // rr dá a volta
                assert_eq!(b.id(m.instrument, zone, li, layer.len()), b.id(m.instrument, zone, li, 0));
            }
            assert!(b.get(m.instrument, zone, m.layers(zone).len(), 0).is_none());
            assert_eq!(b.rr_count(m.instrument, zone, 9), 0);
        }
    }
    assert_eq!(seen.len(), 72);
    assert!(kiai::TAKES.iter().all(|t| !seen.contains(&b.id_by_name(t.name).unwrap())));
    assert!(b.by_name("nao-existe").is_none());
}

#[test]
fn layer_selection_boundaries() {
    // maxVel = (i+1)/n, última 1.01
    assert_eq!(layer_max_vel(0, 1), 1.01);
    assert_eq!(layer_max_vel(0, 2), 0.5);
    assert_eq!(layer_max_vel(1, 2), 1.01);
    assert_eq!(layer_max_vel(2, 4), 0.75);

    assert_eq!(select_layer(1, 0.0), 0);
    assert_eq!(select_layer(1, 1.0), 0);
    // 2 camadas: <= 0.5 → 0
    assert_eq!(select_layer(2, 0.0), 0);
    assert_eq!(select_layer(2, 0.05), 0);
    assert_eq!(select_layer(2, 0.5), 0);
    assert_eq!(select_layer(2, 0.5001), 1);
    assert_eq!(select_layer(2, 1.0), 1);
    // 3 camadas
    assert_eq!(select_layer(3, 1.0 / 3.0), 0);
    assert_eq!(select_layer(3, 0.34), 1);
    assert_eq!(select_layer(3, 2.0 / 3.0), 1);
    assert_eq!(select_layer(3, 0.7), 2);
    assert_eq!(select_layer(3, 1.0), 2);
    // 4 camadas (odaiko)
    assert_eq!(select_layer(4, 0.25), 0);
    assert_eq!(select_layer(4, 0.26), 1);
    assert_eq!(select_layer(4, 0.5), 1);
    assert_eq!(select_layer(4, 0.75), 2);
    assert_eq!(select_layer(4, 0.76), 3);
    // fora do range / NaN → última (fallback `?? layers[n-1]`)
    assert_eq!(select_layer(3, 5.0), 2);
    assert_eq!(select_layer(3, f32::NAN), 2);
    assert_eq!(select_layer(3, -1.0), 0);
    assert_eq!(select_layer(0, 0.5), 0);

    // layerVel = min(1, vel·√dyn)
    assert_eq!(layer_velocity(0.7, 1.0), 0.7);
    assert_eq!(layer_velocity(0.7, 3.2), 1.0);
    assert!((layer_velocity(0.7, 0.25) - 0.35).abs() < 1e-6);

    let b = bank();
    use BaseInstrument::*;
    // don (0.7) em mf: chu → camada forte; em pp (0.25) → camada suave
    assert_eq!(b.select_layer(Chu, Zone::Head, layer_velocity(0.7, 1.0)), 1);
    assert_eq!(b.select_layer(Chu, Zone::Head, layer_velocity(0.7, 0.25)), 0);
    assert_eq!(b.select_layer(Odaiko, Zone::Head, 1.0), 3);
    assert_eq!(b.select_layer(Kane, Zone::Rim, 1.0), 0);
    assert_eq!(b.select_layer(Shime, Zone::Head, 0.7), 2);
}

#[test]
fn round_robin_cycles_per_zone_and_layer() {
    let b = bank();
    use BaseInstrument::*;
    let mut rr = RoundRobin::new();
    // chu/rim camada 1 tem 3 variações: 0,1,2,0,1…
    let seq: Vec<&str> = (0..5).map(|_| b.pick(Chu, Zone::Rim, 0.5, 1.0, &mut rr).id.name()).collect();
    assert_eq!(seq, ["chu-rim-1-0", "chu-rim-1-1", "chu-rim-1-2", "chu-rim-1-0", "chu-rim-1-1"]);
    // outra camada e outra zona têm contadores independentes
    let p = b.pick(Chu, Zone::Rim, 0.1, 1.0, &mut rr);
    assert_eq!((p.id.name(), p.layer, p.rr), ("chu-rim-0-0", 0, 0));
    let p = b.pick(Chu, Zone::Head, 0.7, 1.0, &mut rr);
    assert_eq!((p.id.name(), p.layer, p.rr), ("chu-head-1-0", 1, 0));
    assert!(Arc::ptr_eq(p.data, b.by_name("chu-head-1-0").unwrap()));
    let p = b.pick(Chu, Zone::Head, 0.7, 1.0, &mut rr);
    assert_eq!(p.id.name(), "chu-head-1-1");
    // a camada 1 da borda continua de onde parou
    assert_eq!(b.pick(Chu, Zone::Rim, 0.5, 1.0, &mut rr).id.name(), "chu-rim-1-2");
    // camada de 1 variação repete sempre
    let mut rr = RoundRobin::new();
    for _ in 0..3 {
        assert_eq!(b.pick(Odaiko, Zone::Head, 1.0, 1.0, &mut rr).id.name(), "odaiko-head-3-0");
    }
    // API manual: contador + get com módulo
    let mut rr = RoundRobin::new();
    let ids: Vec<usize> = (0..4).map(|_| rr.next(Zone::Head, 2)).collect();
    assert_eq!(ids, [0, 1, 2, 3]);
    assert_eq!(rr.next(Zone::Rim, 2), 0);
    rr.reset();
    assert_eq!(rr, RoundRobin::default());
}

#[test]
fn kiai_takes_and_counts() {
    let b = bank();
    for v in [kiai::Voice::Male, kiai::Voice::Female] {
        // as duas vozes têm "ha" e "hai" (fallback por vogal) e a contagem de 1 a 8
        assert!(b.kiai_takes(v, "ha", false).len() >= 2);
        assert!(!b.kiai_takes(v, "hai", false).is_empty());
        for n in 1..=8 {
            assert_eq!(b.kiai_takes(v, kiai::count_word(n, v).unwrap(), true).len(), 1, "{v:?} {n}");
        }
        assert!(b.kiai_words(v).len() >= 6);
    }
    assert!(b.kiai_takes(kiai::Voice::Male, "nada", false).is_empty());
    assert_eq!(kiai::count_word(9, kiai::Voice::Male), None);
}

#[test]
fn credits_text() {
    assert!(CREDITS.starts_with("# Créditos dos samples"));
    for needle in ["CC BY-SA 4.0", "CC BY 3.0", "CC0", "CC BY 4.0", "S. Christian Collins", "whatsanickname4u", "RutgerMuller", "Vilaa2404"] {
        assert!(CREDITS.contains(needle), "{needle}");
    }
}

#[test]
fn missing_dir_is_an_error_naming_the_file() {
    let err = SampleBank::load_dir("/nonexistent/wadawko-samples").unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains(".ogg"), "{msg}");
}

#[test]
fn garbage_bytes_fail_to_decode() {
    assert!(decode_ogg(vec![0u8; 1024]).is_err());
    assert!(decode_ogg(Vec::new()).is_err());
}

#[cfg(feature = "embed")]
#[test]
fn embedded_matches_directory() {
    let e = SampleBank::load_embedded().expect("embutidos");
    let d = bank();
    assert_eq!(e.len(), d.len());
    for ((name, a), (_, b)) in e.iter().zip(d.iter()) {
        assert_eq!(a.sr, b.sr, "{name}");
        assert!(a.frames == b.frames, "{name}: PCM difere");
    }
    let l = SampleBank::load_from(None).unwrap();
    assert_eq!(l.len(), d.len());
    // fallback: diretório vazio → tudo dos embutidos
    let tmp = std::env::temp_dir().join(format!("wadawko-samples-empty-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let f = SampleBank::load(&SampleSource::DirWithFallback(tmp.clone())).unwrap();
    assert!(f.by_name("kiai/m-ha-0").unwrap().frames == e.by_name("kiai/m-ha-0").unwrap().frames);
    let _ = std::fs::remove_dir(&tmp);
}

#[cfg(not(feature = "embed"))]
#[test]
fn without_embed_feature_embedded_load_errors() {
    assert!(!SampleBank::HAS_EMBEDDED);
    let msg = format!("{:#}", SampleBank::load_embedded().unwrap_err());
    assert!(msg.contains("embed"), "{msg}");
    assert!(SampleBank::load_from(Some(&repo_samples_dir())).is_ok());
}
