//! Teclado (PLANEJAMENTO-teclado.md, K0–K4): ações GPUI, registro único de
//! comandos, keymap em JSON, fuzzy, rótulos de salto e o registro de alvos dos
//! hints. A paleta de comandos (`palette`), o which-key (`whichkey`), o overlay de
//! hints (`hint_overlay`) e o vim além do básico (`modal`: `.`, registradores,
//! marcas, busca, `:`) são blocos `impl Shell` nestes arquivos.

mod actions;
mod fuzzy;
mod hint_overlay;
mod hint_targets;
mod keymap;
mod labels;
pub mod modal;
pub mod palette;
mod registry;
mod settings;
mod whichkey;

pub use actions::*;
pub use fuzzy::fuzzy;
pub use hint_targets::{HINTS, HintAct, dispatch, hintable, next_frame};
pub use keymap::{all_raw_bindings, binding_text, conflicts, load_keymap, user_keymap_path};
#[cfg(test)]
pub use keymap::{DEFAULT_KEYMAP, raw_bindings};
pub use labels::{HINT_ALPHABET, labels};
pub use registry::{SPECS, prefix_title, title_for};
pub use settings::{Settings, load_settings, save_settings, settings_path};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_prefix_free_and_unique() {
        let alpha: Vec<char> = HINT_ALPHABET.chars().collect();
        for n in [1, 5, 26, 27, 120, 480, 700] {
            let l = labels(n, &alpha);
            assert_eq!(l.len(), n);
            for (i, a) in l.iter().enumerate() {
                for (j, b) in l.iter().enumerate() {
                    if i != j {
                        assert!(!b.starts_with(a.as_str()), "{a} é prefixo de {b} (n={n})");
                    }
                }
            }
            assert!(l.windows(2).all(|w| w[0].len() <= w[1].len()));
            let max = l.iter().map(|x| x.len()).max().unwrap_or(0);
            assert!(max <= if n <= 26 { 1 } else if n <= 676 { 2 } else { 3 }, "rótulo longo demais para n={n}");
        }
    }

    #[test]
    fn fuzzy_folds_accents_and_ranks_word_starts() {
        assert!(fuzzy("metr", "Metrônomo: ligar/desligar").is_some());
        assert!(fuzzy("secao", "+ Seção").is_some());
        assert!(fuzzy("xyz", "Tocar").is_none());
        assert!(fuzzy("tp", "Tocar / parar").unwrap() > fuzzy("tp", "Metrônomo: ligar/desligar").unwrap_or(-999));
    }

    #[test]
    fn default_keymap_has_no_conflicts() {
        let c = conflicts(&raw_bindings(DEFAULT_KEYMAP, "padrão"));
        assert!(c.is_empty(), "{c:#?}");
    }

    #[test]
    fn conflicts_are_detected() {
        let src = r#"[{"context":"X","bindings":{"g":"a::A","g g":"a::B","h":"a::C"}},{"context":"X","bindings":{"h":"a::D"}}]"#;
        let c = conflicts(&raw_bindings(src, "t"));
        assert_eq!(c.len(), 2, "{c:#?}");
    }

    #[test]
    fn default_keymap_actions_have_known_names() {
        let v: serde_json::Value = serde_json::from_str(DEFAULT_KEYMAP).unwrap();
        for block in v.as_array().unwrap() {
            for (_, a) in block["bindings"].as_object().unwrap() {
                let name = a.as_str().unwrap();
                assert!(title_for(name).is_some() || false, "sem título: {name}");
            }
        }
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;

    #[test]
    fn specs_have_unique_ids_and_titles() {
        let mut seen = std::collections::HashSet::new();
        for s in SPECS {
            assert!(seen.insert(s.id), "id duplicado: {}", s.id);
            assert!(!s.title.is_empty(), "sem título: {}", s.id);
            assert!(s.id.contains("::"), "id sem namespace: {}", s.id);
        }
    }

    #[test]
    fn default_keymap_parses_and_has_all_blocks() {
        let raw = raw_bindings(DEFAULT_KEYMAP, "padrão");
        assert!(raw.len() > 150, "poucos bindings: {}", raw.len());
        for ctx in ["(global)", "Workspace", "Workspace && mode == simple", "Workspace && mode == normal", "Workspace && mode == insert", "Palette"] {
            assert!(raw.iter().any(|b| b.context == ctx), "falta o bloco {ctx}");
        }
    }

    #[test]
    fn prefix_titles_cover_leader_groups() {
        let raw = raw_bindings(DEFAULT_KEYMAP, "padrão");
        for b in &raw {
            let ks: Vec<&str> = b.keys.split(' ').collect();
            if ks.len() == 3 && ks[0] == "space" {
                let p = format!("space {}", ks[1]);
                assert!(prefix_title(&p).is_some(), "grupo sem título: {p}");
            }
        }
    }
}
