//! Views do app. PONTO DE EXTENSÃO (ver `docs/arquitetura/app.md`):
//!
//! - [`Page`] = páginas do NavRail; [`Tab`] = abas do Editor.
//! - Cada página/aba vive no SEU arquivo (`mixer.rs`, `people.rs`, `kit.rs`,
//!   `lane.rs`, `score.rs`) como uma entidade GPUI que recebe o
//!   `Entity<AppState>`; quem implementa uma delas só edita o próprio arquivo.
//! - [`Views::page`]/[`Views::tab`] são os únicos `match` que delegam para elas, e
//!   [`bind_actions`] encadeia o `bind` de cada módulo (handlers de ações na raiz).
//!
//! A grade (`grid/`), a topbar, o NavRail, o painel de músicas, a status bar, a
//! barra de contexto e as Configurações são `impl Shell` nos arquivos irmãos.

pub mod context_toolbar;
pub mod director;
pub mod drag_ghost;
pub mod grid;
pub mod inspector;
pub mod kit;
pub mod lane;
pub mod minimap;
pub mod mixer;
pub mod nav_rail;
pub mod people;
pub mod right_panel;
pub mod score;
pub mod sections_panel;
pub mod settings;
pub mod status_bar;
pub mod topbar;
pub mod widgets;

use gpui_kit::{AnyElement, Context, Entity, prelude::*};

use crate::shell::Shell;
use crate::state::AppState;

/// Páginas do NavRail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Editor,
    Pessoas,
    Mixer,
    Kit,
    Config,
}

/// Abas do Editor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Grade,
    Partitura,
    Esteira,
}

/// As views "plugáveis" (uma entidade por arquivo).
pub struct Views {
    pub mixer: Entity<mixer::MixerView>,
    pub people: Entity<people::PeopleView>,
    pub kit: Entity<kit::KitView>,
    pub lane: Entity<lane::LaneView>,
    pub score: Entity<score::ScoreView>,
}

impl Views {
    pub fn new(state: &Entity<AppState>, cx: &mut Context<Shell>) -> Self {
        Views {
            mixer: cx.new(|cx| mixer::MixerView::new(state.clone(), cx)),
            people: cx.new(|cx| people::PeopleView::new(state.clone(), cx)),
            kit: cx.new(|cx| kit::KitView::new(state.clone(), cx)),
            lane: cx.new(|cx| lane::LaneView::new(state.clone(), cx)),
            score: cx.new(|cx| score::ScoreView::new(state.clone(), cx)),
        }
    }

    /// Página que não é o Editor nem Configurações (essas o Shell monta).
    pub fn page(&self, page: Page) -> Option<AnyElement> {
        Some(match page {
            Page::Mixer => self.mixer.clone().into_any_element(),
            Page::Pessoas => self.people.clone().into_any_element(),
            Page::Kit => self.kit.clone().into_any_element(),
            Page::Editor | Page::Config => return None,
        })
    }

    /// Aba do Editor que não é a grade.
    pub fn tab(&self, tab: Tab) -> Option<AnyElement> {
        Some(match tab {
            Tab::Esteira => self.lane.clone().into_any_element(),
            Tab::Partitura => self.score.clone().into_any_element(),
            Tab::Grade => return None,
        })
    }
}

/// Handlers de ação das views plugáveis, registrados na raiz do Shell (assim
/// valem com o foco em qualquer lugar). Cada módulo cuida do seu `bind`.
pub fn bind_actions(d: gpui_kit::Stateful<gpui_kit::Div>, cx: &mut Context<Shell>) -> gpui_kit::Stateful<gpui_kit::Div> {
    let d = mixer::bind(d, cx);
    let d = people::bind(d, cx);
    let d = kit::bind(d, cx);
    let d = lane::bind(d, cx);
    score::bind(d, cx)
}
