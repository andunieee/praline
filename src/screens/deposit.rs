use freya::{
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    engine::Deposit,
    fmt,
    screens::use_selected_mint,
    ui::*,
};

const PRESETS: &[u64] = &[1_000, 5_000, 21_000, 100_000];

#[derive(Clone, PartialEq)]
enum Stage {
    Amount,
    Waiting(Deposit, u64),
    Done(u64),
    Failed(String),
}

/// Lightning in: the mint makes an invoice, paying it makes ecash.
#[derive(PartialEq)]
pub struct DepositScreen;

impl Component for DepositScreen {
    fn render(&self) -> impl IntoElement {
        let Some(url) = use_selected_mint() else {
            return page_header("Deposit", true).into_element();
        };
        let engine = use_engine();
        let refresh = use_refresh();
        let amount = use_state(|| 0u64);
        let mut stage = use_state(|| Stage::Amount);
        let mut busy = use_state(|| false);

        let create = {
            let url = url.clone();
            move |_| {
                if amount() == 0 || busy() {
                    return;
                }
                busy.set(true);
                let (engine, url, sats) = (engine.clone(), url.clone(), amount());
                spawn(async move {
                    match engine.start_deposit(&url, sats).await {
                        Ok(deposit) => {
                            let quote = deposit.quote_id.clone();
                            stage.set(Stage::Waiting(deposit, sats));
                            match engine.finish_deposit(&url, &quote).await {
                                Ok(received) => {
                                    bump(refresh);
                                    stage.set(Stage::Done(received));
                                }
                                Err(e) => stage.set(Stage::Failed(e)),
                            }
                        }
                        Err(e) => stage.set(Stage::Failed(e)),
                    }
                    busy.set(false);
                });
            }
        };

        let body: Element = match stage.read().clone() {
            Stage::Amount => rect()
                .width(Size::fill())
                .spacing(16.)
                .child(AmountKeypad {
                    amount,
                    presets: PRESETS,
                })
                .child(
                    wide_button("Create invoice")
                        .filled()
                        .enabled(amount() > 0 && !busy())
                        .on_press(create),
                )
                .into_element(),
            Stage::Waiting(deposit, sats) => rect()
                .width(Size::fill())
                .spacing(16.)
                .cross_align(Alignment::center())
                .child(
                    label()
                        .font_size(28.)
                        .font_weight(FontWeight::BOLD)
                        .text(fmt::sats(sats)),
                )
                .child(QrCard {
                    data: deposit.invoice.clone(),
                })
                .child(
                    label()
                        .color(palette().text_secondary)
                        .text("Pay this Lightning invoice. The ecash arrives when it is paid."),
                )
                .into_element(),
            Stage::Done(received) => rect()
                .width(Size::fill())
                .padding((24., 0.))
                .spacing(14.)
                .center()
                .child(icon_badge(lucide::check(), 72., Color::WHITE, POSITIVE))
                .child(
                    label()
                        .font_size(24.)
                        .font_weight(FontWeight::BOLD)
                        .text(format!("Received {}", fmt::sats(received))),
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
                        .on_press(move |_| stage.set(Stage::Amount)),
                )
                .into_element(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Deposit", true))
            .child(rect().width(Size::fill()).padding((4., 16., 24., 16.)).child(body))
            .into_element()
    }
}
