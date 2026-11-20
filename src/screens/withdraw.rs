use freya::{
    clipboard::Clipboard,
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    engine::Withdrawal,
    fmt,
    screens::use_selected_mint,
    state::PendingInvoice,
    ui::*,
};

#[derive(Clone, PartialEq)]
enum Stage {
    Invoice,
    Quote(Withdrawal),
    Done(u64),
    Failed(String),
}

/// Lightning out: ecash melted to pay an invoice.
#[derive(PartialEq)]
pub struct WithdrawScreen;

impl Component for WithdrawScreen {
    fn render(&self) -> impl IntoElement {
        let Some(url) = use_selected_mint() else {
            return page_header("Withdraw", true).into_element();
        };
        let engine = use_engine();
        let refresh = use_refresh();
        let mut invoice = use_state(String::new);
        let mut stage = use_state(|| Stage::Invoice);
        let mut busy = use_state(|| false);

        let mut pending = use_consume::<PendingInvoice>().0;
        use_hook(move || {
            if let Some(waiting) = pending.write().take() {
                invoice.set(waiting);
            }
        });

        #[cfg(target_os = "android")]
        {
            let mut scan = crate::scanner::use_scan_result();
            use_side_effect(move || {
                let scanned = scan.read().clone();
                if let Some(contents) = scanned {
                    scan.set(None);
                    invoice.set(contents.trim().trim_start_matches("lightning:").to_owned());
                }
            });
        }

        let quote = {
            let (engine, url) = (engine.clone(), url.clone());
            move |_| {
                let text = invoice.read().trim().trim_start_matches("lightning:").to_owned();
                if text.is_empty() || busy() {
                    return;
                }
                busy.set(true);
                let (engine, url) = (engine.clone(), url.clone());
                spawn(async move {
                    match engine.quote_withdrawal(&url, &text).await {
                        Ok(w) => stage.set(Stage::Quote(w)),
                        Err(e) => stage.set(Stage::Failed(e)),
                    }
                    busy.set(false);
                });
            }
        };

        let colors = palette();
        let body: Element = match stage.read().clone() {
            Stage::Invoice => rect()
                .width(Size::fill())
                .spacing(12.)
                .child(
                    Input::new(invoice)
                        .width(Size::fill())
                        .placeholder("Lightning invoice (lnbc…)"),
                )
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::flex())
                        .spacing(8.)
                        .child(
                            button()
                                .width(Size::flex(1.))
                                .outline()
                                .on_press(move |_| {
                                    if let Ok(text) = Clipboard::get() {
                                        invoice.set(text.trim().to_owned());
                                    }
                                })
                                .child("Paste"),
                        )
                        .maybe(cfg!(target_os = "android"), |el| {
                            el.child(
                                button()
                                    .width(Size::flex(1.))
                                    .outline()
                                    .on_press(|_| {
                                        #[cfg(target_os = "android")]
                                        crate::scanner::start();
                                    })
                                    .child("Scan"),
                            )
                        }),
                )
                .child(
                    wide_button(if busy() { "Asking the mint…" } else { "Continue" })
                        .filled()
                        .enabled(!invoice.read().trim().is_empty() && !busy())
                        .on_press(quote),
                )
                .into_element(),
            Stage::Quote(w) => {
                let (engine, url, quote_id) = (engine.clone(), url.clone(), w.quote_id.clone());
                rect()
                    .width(Size::fill())
                    .spacing(14.)
                    .child(
                        rect()
                            .width(Size::fill())
                            .padding((4., 14.))
                            .corner_radius(16.)
                            .background(colors.surface_secondary)
                            .child(kv_row("Pays", fmt::sats(w.amount_sat)))
                            .child(divider())
                            .child(kv_row("Lightning fee, at most", fmt::sats(w.fee_reserve_sat)))
                            .child(divider())
                            .child(kv_row(
                                "Total, at most",
                                fmt::sats(w.amount_sat + w.fee_reserve_sat),
                            )),
                    )
                    .child(
                        wide_button(if busy() { "Paying…" } else { "Pay invoice" })
                            .filled()
                            .enabled(!busy())
                            .on_press(move |_| {
                                busy.set(true);
                                let (engine, url, quote_id) =
                                    (engine.clone(), url.clone(), quote_id.clone());
                                spawn(async move {
                                    match engine.confirm_withdrawal(&url, &quote_id).await {
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
                            .on_press(move |_| stage.set(Stage::Invoice)),
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
                        .text("Paid"),
                )
                .child(
                    label()
                        .color(colors.text_secondary)
                        .text(format!("Lightning fee: {}", fmt::sats(fee))),
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
                        .on_press(move |_| stage.set(Stage::Invoice)),
                )
                .into_element(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Withdraw", true))
            .child(rect().width(Size::fill()).padding((4., 16., 24., 16.)).child(body))
            .into_element()
    }
}
