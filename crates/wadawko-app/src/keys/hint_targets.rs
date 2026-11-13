//! Alvos de salto (Ctrl+Space): qualquer elemento embrulhado em [`hintable`]
//! registra seus bounds a cada frame; o overlay (em `shell.rs`) lê daqui.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::{Action, App, Bounds, Pixels, SharedString, Window};

pub type HintAct = Rc<dyn Fn(&mut Window, &mut App)>;

pub struct HintTarget {
    pub bounds: Bounds<Pixels>,
    pub act: HintAct,
}

#[derive(Default)]
pub struct HintRegistry {
    pub frame: u64,
    pub targets: HashMap<SharedString, HintTarget>,
}

thread_local! {
    pub static HINTS: RefCell<HintRegistry> = RefCell::new(HintRegistry::default());
}

pub fn next_frame() -> u64 {
    HINTS.with(|h| {
        let mut h = h.borrow_mut();
        h.frame += 1;
        h.frame
    })
}

/// Envolve um elemento interativo para virar alvo de salto: um canvas absoluto
/// filho registra os bounds do pai no prepaint de cada frame em que ele é renderizado.
pub fn hintable<E: gpui_kit::ParentElement + gpui_kit::Styled>(el: E, id: impl Into<SharedString>, act: HintAct) -> E {
    use gpui_kit::{Styled as _, canvas};
    let id = id.into();
    el.child(
        canvas(
            move |b, _, _| {
                HINTS.with(|h| {
                    let mut h = h.borrow_mut();
                    h.targets.insert(id.clone(), HintTarget { bounds: b, act: act.clone() });
                })
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

/// Ação → HintAct que a despacha.
pub fn dispatch<A: Action + Clone>(a: A) -> HintAct {
    Rc::new(move |w: &mut Window, cx: &mut App| w.dispatch_action(a.boxed_clone(), cx))
}
