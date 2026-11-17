//! Página Configurações: teclado (vim, keymap do usuário, testar tecla,
//! conflitos, mapa completo de atalhos), sincronia som × imagem, banco e créditos.

use gpui_kit::{ClickEvent, Context, FontWeight, SharedString, div, prelude::*, px};

use crate::keys;
use crate::shell::Shell;
use crate::theme::Theme;
use crate::ui;

impl Shell {
    pub(crate) fn config_page(&self, th: &Theme, cx: &mut Context<Self>) -> gpui_kit::Div {
        let user = keys::user_keymap_path();
        let exists = user.exists();
        let raw = keys::all_raw_bindings();
        let conflicts = keys::conflicts(&raw);
        let card = || ui::card(th);
        let h = |t: &'static str| div().text_size(px(15.0)).font_weight(FontWeight::BOLD).child(t);
        let probe = match &self.last_key {
            Some((k, a, at)) => format!("{k}  →  {}   (há {:.0} s)", a.clone().unwrap_or_else(|| "nenhuma ação (texto/sem atalho)".into()), at.elapsed().as_secs_f32()),
            None => "aperte qualquer tecla…".into(),
        };

        let mut table = div().flex().flex_col();
        let mut last_ctx = String::new();
        for b in &raw {
            if b.context != last_ctx {
                last_ctx = b.context.clone();
                table = table.child(
                    div().mt(px(10.0)).mb(px(2.0)).text_size(px(11.0)).font_weight(FontWeight::BOLD).text_color(th.accent2).child(SharedString::from(format!("[{}] · {}", b.context, b.origin))),
                );
            }
            let title = keys::title_for(&b.action).unwrap_or("");
            table = table.child(
                div()
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .text_size(px(12.0))
                    .child(div().w(px(150.0)).child(ui::kbd(&b.keys, th)))
                    .child(div().w(px(260.0)).text_color(th.ink).child(SharedString::from(title.to_string())))
                    .child(div().text_color(th.ink_faint).child(SharedString::from(b.action.clone()))),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(div().text_size(px(22.0)).font_family("Shippori Mincho").font_weight(FontWeight::BOLD).child("Configurações"))
            .child(
                card()
                    .child(h("Teclado"))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .items_center()
                            .child(ui::atoggle("cfg-vim", th, self.vim_on, keys::keys::ToggleVim).child(if self.vim_on { "Vim: ligado" } else { "Vim: desligado (modo simples)" }))
                            .child(ui::abtn("cfg-reload", th, keys::keys::ReloadKeymap).child("Recarregar atalhos"))
                            .when(!exists, |d| d.child(ui::abtn("cfg-create", th, keys::keys::CreateKeymap).child("Criar keymap.json"))),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(th.ink_soft)
                            .child(SharedString::from(format!(
                                "Seus atalhos: {} {} — mesmo formato do Zed; recarrega sozinho ao salvar. Preferência do vim: {}",
                                user.display(),
                                if exists { "(existe)" } else { "(não existe)" },
                                keys::settings_path().display()
                            ))),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(th.ink_soft)
                            .child("Ctrl+/ mostra o que dá para apertar aqui · Ctrl+K paleta · Ctrl+Space ou Alt+F saltar · : comandos · / busca"),
                    ),
            )
            .child({
                let st = self.state.read(cx);
                let offset = st.av_offset_ms;
                let info = match st.audio.as_ref() {
                    Some(a) => {
                        let c = a.clock();
                        format!(
                            "latência informada pelo áudio: {:.1} ms · buffer {} quadros @ {} Hz · PIPEWIRE_LATENCY={}",
                            c.latency_ns.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1e6,
                            c.buffer_frames.load(std::sync::atomic::Ordering::Relaxed),
                            c.sample_rate,
                            std::env::var("PIPEWIRE_LATENCY").unwrap_or_else(|_| "—".into())
                        )
                    }
                    None => st.audio_error.clone().map(|e| format!("sem áudio: {e}")).unwrap_or_else(|| "carregando samples…".into()),
                };
                let step = |id: &'static str, label: &'static str, d: f32, cx: &mut Context<Self>| {
                    ui::btn(id, th).h(px(28.0)).text_size(px(12.0)).child(label).on_click(cx.listener(move |this: &mut Shell, _: &ClickEvent, _, cx| {
                        let v = if d == 0.0 { 0.0 } else { this.state.read(cx).av_offset_ms + d };
                        this.set_offset(v, cx);
                    }))
                };
                card()
                    .child(h("Sincronia som × imagem"))
                    .child(div().text_size(px(12.0)).text_color(th.ink_soft).child(SharedString::from(info)))
                    .child(div().text_size(px(12.0)).text_color(th.ink_soft).child(
                        "As células piscam quando a nota soa. Ligue o metrônomo, toque e ajuste até o destaque bater com o som: se a luz vem antes do som, aumente; se vem depois, diminua. (:offset 40 também funciona.)",
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(step("off-10", "−10 ms", -10.0, cx))
                            .child(step("off-1", "−1", -1.0, cx))
                            .child(div().w(px(90.0)).flex().justify_center().text_size(px(15.0)).font_weight(FontWeight::BOLD).child(SharedString::from(format!("{offset:+.0} ms"))))
                            .child(step("off+1", "+1", 1.0, cx))
                            .child(step("off+10", "+10 ms", 10.0, cx))
                            .child(step("off0", "zerar", 0.0, cx)),
                    )
            })
            .child({
                let st = self.state.read(cx);
                card()
                    .child(h("Banco e créditos"))
                    .child(div().text_size(px(12.0)).text_color(th.ink_soft).child(SharedString::from(format!(
                        "banco: {}{} · {} músicas · tema e música atual ficam no banco; vim e sincronia em {}",
                        st.db_label,
                        if st.read_only() { " (somente leitura)" } else { "" },
                        st.store.projects().len(),
                        keys::settings_path().display()
                    ))))
                    .child(div().text_size(px(11.0)).text_color(th.ink_faint).child(SharedString::from(wadawko_samples::CREDITS.lines().filter(|l| !l.trim().is_empty()).take(6).collect::<Vec<_>>().join(" · "))))
            })
            .child(
                card()
                    .child(h("Testar tecla"))
                    .child(div().text_size(px(12.0)).text_color(th.ink_soft).child("Se Ctrl+Space não aparecer aqui, o fcitx5/ibus está capturando a tecla — use Alt+F (ou mude o atalho do IME)."))
                    .child(div().px(px(10.0)).py(px(6.0)).rounded(px(6.0)).bg(th.paper_raise).text_size(px(14.0)).font_weight(FontWeight::MEDIUM).child(SharedString::from(probe))),
            )
            .child(
                card()
                    .child(h("Conflitos"))
                    .child(if conflicts.is_empty() {
                        div().text_size(px(12.0)).text_color(th.ink_soft).child("Nenhum conflito no keymap atual.")
                    } else {
                        let mut d = div().flex().flex_col().gap(px(4.0));
                        for c in conflicts {
                            d = d.child(div().text_size(px(12.0)).text_color(th.accent).child(SharedString::from(c)));
                        }
                        d
                    }),
            )
            .child(card().child(h("Mapa de atalhos")).child(table))
    }
}
