//! Moldura da janela em cache: barra de cima, barra lateral e painel de músicas.
//!
//! Enquanto algo anima (tocar, arrastar notas, esteira, partitura) o Shell
//! redesenha a cada frame; sem cache, a moldura inteira era refeita (layout +
//! paint de centenas de elementos) 60×/s. Cada peça aqui é uma view
//! `cached(...)`: o GPUI reaproveita o desenho do frame anterior até a view ser
//! notificada. O Shell calcula uma CHAVE por peça a cada frame (tudo que a peça
//! lê) e só notifica quando a chave muda — ver [`Shell::refresh_chrome`].
//!
//! Ao acrescentar algo que uma destas peças mostre, inclua na chave dela; senão
//! a peça fica velha até outra coisa mudar.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gpui_kit::{AnyElement, Context, Render, WeakEntity, Window, div, prelude::*};

use super::Shell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Top,
    Nav,
    Panel,
    /// barra de contexto do editor (altura variável: medida quando a chave muda)
    Context,
    /// status bar (desenhada de `Shell::status` — um modelo só de texto)
    Status,
    /// painel de seções à esquerda do editor (`views/sections_panel.rs`)
    Sections,
}

pub struct Chrome {
    shell: WeakEntity<Shell>,
    part: Part,
    /// chave do último desenho (o Shell compara e notifica quando muda)
    pub key: u64,
}

impl Chrome {
    pub fn new(shell: WeakEntity<Shell>, part: Part) -> Self {
        Chrome { shell, part, key: 0 }
    }
}

impl Render for Chrome {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let part = self.part;
        let Some(shell) = self.shell.upgrade() else { return div().into_any_element() };
        shell.update(cx, |sh, cx| -> AnyElement {
            let th = sh.theme(cx);
            match part {
                Part::Top => {
                    let playing = sh.playing(cx);
                    sh.topbar(&th, playing, cx).into_any_element()
                }
                Part::Nav => sh.nav_rail(&th).into_any_element(),
                Part::Panel => sh.right_panel(&th, cx).into_any_element(),
                Part::Context => sh.context_toolbar(&th, cx).into_any_element(),
                Part::Status => sh.status_bar(&th, &sh.status.clone()).into_any_element(),
                Part::Sections => sh.sections_panel(&th, cx).into_any_element(),
            }
        })
    }
}

pub fn key(f: impl FnOnce(&mut DefaultHasher)) -> u64 {
    let mut h = DefaultHasher::new();
    f(&mut h);
    h.finish()
}

/// Hash de qualquer valor `Hash` (atalho para as chaves).
pub fn hash_into<T: Hash + ?Sized>(h: &mut DefaultHasher, v: &T) {
    v.hash(h);
}
