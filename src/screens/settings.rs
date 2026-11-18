use freya::prelude::*;

use crate::{
    engine::{DEFAULT_RELAYS, normalize_relay},
    ui::*,
};

#[derive(PartialEq)]
pub struct SettingsScreen;

impl Component for SettingsScreen {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let mut settings = use_settings();
        let mut show_phrase = use_state(|| false);
        let colors = palette();
        let relays = use_state({
            let engine = engine.clone();
            move || engine.relays().join(", ")
        });
        let mut relay_message = use_state(|| None::<Result<String, String>>);
        let hide = settings.read().hide_balance;

        let phrase = if show_phrase() {
            engine.recovery_phrase()
        } else {
            "•••• •••• •••• •••• •••• •••• •••• •••• •••• •••• •••• ••••".to_owned()
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Settings", false))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((4., 16., 24., 16.))
                    .spacing(20.)
                    .child(section(
                        "Display",
                        rect().width(Size::fill()).child(
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .content(Content::flex())
                                .padding((12., 0.))
                                .cross_align(Alignment::center())
                                .child(
                                    rect()
                                        .width(Size::flex(1.))
                                        .child(label().text("Hide balances"))
                                        .child(
                                            label()
                                                .font_size(12.)
                                                .color(colors.text_secondary)
                                                .text("Also by tapping the balance"),
                                        ),
                                )
                                .child(Switch::new(WritableSetter::new(hide, move |hidden| {
                                    settings.write().hide_balance = hidden
                                }))),
                        ),
                    ))
                    .child(section(
                        "Nostr relays",
                        rect()
                            .width(Size::fill())
                            .padding((12., 0.))
                            .spacing(10.)
                            .child(
                                label()
                                    .font_size(12.)
                                    .color(colors.text_secondary)
                                    .text("Where Cassis routes are looked up when a payment needs one. Separate them with commas."),
                            )
                            .child(Input::new(relays).width(Size::fill()).placeholder("nos.lol, relay.primal.net"))
                            .maybe_child(relay_message.read().clone().map(|m| match m {
                                Ok(text) => notice(text, POSITIVE),
                                Err(text) => notice(text, NEGATIVE),
                            }))
                            .child(
                                Button::new()
                                    .outline()
                                    .on_press({
                                        let engine = engine.clone();
                                        move |_| {
                                            let list: Vec<String> = relays
                                                .read()
                                                .split(|c: char| c == '\n' || c == ',' || c.is_whitespace())
                                                .filter_map(normalize_relay)
                                                .collect();
                                            relay_message.set(Some(
                                                engine.set_relays(list).map(|_| "Saved.".to_owned()),
                                            ));
                                        }
                                    })
                                    .child("Save relays"),
                            )
                            .child(
                                Button::new().outline().on_press({
                                    let mut relays = relays;
                                    move |_| {
                                        relays.set(DEFAULT_RELAYS.join(", "));
                                        relay_message.set(None);
                                    }
                                })
                                .child("Use the defaults"),
                            ),
                    ))
                    .child(section(
                        "Recovery phrase",
                        rect()
                            .width(Size::fill())
                            .padding((12., 0.))
                            .spacing(10.)
                            .child(if show_phrase() {
                                SelectableText::new().span(phrase).into_element()
                            } else {
                                label().text(phrase).into_element()
                            })
                            .child(
                                label()
                                    .font_size(12.)
                                    .color(colors.text_secondary)
                                    .text("These twelve words are the keys to your funds, at every mint and on Cassis. Write them down and keep them private."),
                            )
                            .child(
                                Button::new()
                                    .outline()
                                    .on_press(move |_| show_phrase.set(!show_phrase()))
                                    .child(if show_phrase() { "Hide" } else { "Show" }),
                            ),
                    ))
                    .child(section(
                        "About this wallet",
                        rect()
                            .width(Size::fill())
                            .child(kv_row("Cassis node", engine.node_id()))
                            .child(divider())
                            .child(kv_row("Data", engine.data_dir().display().to_string())),
                    )),
            )
    }
}
