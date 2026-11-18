use freya::{
    clipboard::Clipboard,
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    engine::OnchainOption,
    fmt,
    screens::use_selected_mint,
    ui::*,
};

const PRESETS: &[u64] = &[10_000, 50_000, 100_000, 500_000];

#[derive(Clone, PartialEq)]
enum Stage {
    Details,
    Options(Vec<OnchainOption>),
    Done(Option<u64>),
    Failed(String),
}

/// Bitcoin out: ecash melted to pay an address.
#[derive(PartialEq)]
pub struct OnchainWithdrawScreen;

impl Component for OnchainWithdrawScreen {
    fn render(&self) -> impl IntoElement {
        let Some(url) = use_selected_mint() else {
            return page_header("Onchain withdraw", true).into_element();
        };
        let engine = use_engine();
        let refresh = use_refresh();
        let mut address = use_state(String::new);
        let amount = use_state(|| 0u64);
        let mut chosen = use_state(|| 0usize);
        let mut stage = use_state(|| Stage::Details);
        let mut busy = use_state(|| false);

        let quote = {
            let (engine, url) = (engine.clone(), url.clone());
            move |_| {
                let text = address.read().trim().trim_start_matches("bitcoin:").to_owned();
                if text.is_empty() || amount() == 0 || busy() {
                    return;
                }
                busy.set(true);
                let (engine, url, sats) = (engine.clone(), url.clone(), amount());
                spawn(async move {
                    match engine.quote_onchain_withdrawal(&url, &text, sats).await {
                        Ok(options) => {
                            chosen.set(0);
                            stage.set(Stage::Options(options));
                        }
                        Err(e) => stage.set(Stage::Failed(e)),
                    }
                    busy.set(false);
                });
            }
        };

        let colors = palette();
        let body: Element = match stage.read().clone() {
            Stage::Details => rect()
                .width(Size::fill())
                .spacing(12.)
                .child(
                    Input::new(address)
                        .width(Size::fill())
                        .placeholder("Bitcoin address"),
                )
                .child(
                    Button::new()
                        .width(Size::fill())
                        .outline()
                        .on_press(move |_| {
                            if let Ok(text) = Clipboard::get() {
                                address.set(text.trim().to_owned());
                            }
                        })
                        .child("Paste"),
                )
                .child(AmountKeypad {
                    amount,
                    presets: PRESETS,
                })
                .child(
                    wide_button(if busy() { "Asking the mint…" } else { "Continue" })
                        .filled()
                        .enabled(!address.read().trim().is_empty() && amount() > 0 && !busy())
                        .on_press(quote),
                )
                .into_element(),
            Stage::Options(options) => {
                let selected = options.get(chosen()).cloned();
                let (engine, url) = (engine.clone(), url.clone());
                rect()
                    .width(Size::fill())
                    .spacing(14.)
                    .child(
                        label()
                            .font_size(13.)
                            .font_weight(FontWeight::BOLD)
                            .color(colors.text_secondary)
                            .text("FEE"),
                    )
                    .children(options.iter().enumerate().map(|(i, option)| {
                        let active = i == chosen();
                        let eta = option
                            .estimated_blocks
                            .map(|b| format!("~{b} block{}", if b == 1 { "" } else { "s" }))
                            .unwrap_or_else(|| "Network fee".to_owned());
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .cross_align(Alignment::center())
                            .padding((12., 14.))
                            .corner_radius(14.)
                            .background(colors.surface_secondary)
                            .border(
                                Border::new()
                                    .fill(if active { ACCENT } else { colors.border })
                                    .width(if active { 2. } else { 1. })
                                    .alignment(BorderAlignment::Inner),
                            )
                            .on_press(move |_| chosen.set(i))
                            .child(label().width(Size::flex(1.)).text(eta))
                            .child(
                                label()
                                    .font_weight(FontWeight::BOLD)
                                    .text(format!("up to {}", fmt::sats(option.fee_reserve_sat))),
                            )
                            .into_element()
                    }))
                    .maybe_child(selected.clone().map(|option| {
                        rect()
                            .width(Size::fill())
                            .padding((4., 14.))
                            .corner_radius(16.)
                            .background(colors.surface_secondary)
                            .child(kv_row("Pays", fmt::sats(option.amount_sat)))
                            .child(divider())
                            .child(kv_row("Network fee, at most", fmt::sats(option.fee_reserve_sat)))
                            .child(divider())
                            .child(kv_row(
                                "Total, at most",
                                fmt::sats(option.amount_sat + option.fee_reserve_sat),
                            ))
                    }))
                    .child(
                        wide_button(if busy() { "Paying…" } else { "Send bitcoin" })
                            .filled()
                            .enabled(selected.is_some() && !busy())
                            .on_press(move |_| {
                                let Some(option) = selected.clone() else {
                                    return;
                                };
                                busy.set(true);
                                let (engine, url) = (engine.clone(), url.clone());
                                spawn(async move {
                                    match engine.confirm_onchain_withdrawal(&url, &option).await {
                                        Ok(fee) => {
                                            bump(refresh);
                                            stage.set(Stage::Done(fee));
                                        }
                                        Err(e) => stage.set(Stage::Failed(e)),
                                    }
                                    busy.set(false);
                                });
                            }),
                    )
                    .child(
                        wide_button("Cancel")
                            .flat()
                            .on_press(move |_| stage.set(Stage::Details)),
                    )
                    .into_element()
            }
            Stage::Done(fee) => rect()
                .width(Size::fill())
                .padding((24., 0.))
                .spacing(14.)
                .center()
                .child(icon_badge(lucide::check(), 72., Color::WHITE, POSITIVE))
                .child(
                    label()
                        .font_size(24.)
                        .font_weight(FontWeight::BOLD)
                        .text(if fee.is_some() { "Sent" } else { "Sending" }),
                )
                .child(
                    label()
                        .color(colors.text_secondary)
                        .text(match fee {
                            Some(fee) => format!("Network fee: {}", fmt::sats(fee)),
                            None => "The mint is broadcasting the transaction.".to_owned(),
                        }),
                )
                .child(wide_button("Done").filled().on_press(|_| {
                    RouterContext::get().go_back();
                }))
                .into_element(),
            Stage::Failed(error) => rect()
                .width(Size::fill())
                .spacing(12.)
                .child(notice(error, NEGATIVE))
                .child(
                    wide_button("Try again")
                        .outline()
                        .on_press(move |_| stage.set(Stage::Details)),
                )
                .into_element(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Onchain withdraw", true))
            .child(rect().width(Size::fill()).padding((4., 16., 24., 16.)).child(body))
            .into_element()
    }
}
