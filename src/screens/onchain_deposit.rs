use freya::{
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    engine::OnchainDeposit,
    fmt,
    screens::use_selected_mint,
    ui::*,
};

#[derive(Clone, PartialEq)]
enum Stage {
    Start,
    Waiting(OnchainDeposit),
    Done(u64),
    Failed(String),
}

/// Bitcoin in: the mint gives an address, bitcoin confirmed there becomes ecash.
#[derive(PartialEq)]
pub struct OnchainDepositScreen;

impl Component for OnchainDepositScreen {
    fn render(&self) -> impl IntoElement {
        let Some(url) = use_selected_mint() else {
            return page_header("Onchain deposit", true).into_element();
        };
        let engine = use_engine();
        let refresh = use_refresh();
        let mut stage = use_state(|| Stage::Start);
        let mut busy = use_state(|| false);

        let create = move |_| {
            if busy() {
                return;
            }
            busy.set(true);
            let (engine, url) = (engine.clone(), url.clone());
            spawn(async move {
                match engine.start_onchain_deposit(&url).await {
                    Ok(deposit) => {
                        let quote = deposit.quote_id.clone();
                        stage.set(Stage::Waiting(deposit));
                        match engine.finish_onchain_deposit(&url, &quote).await {
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
        };

        let colors = palette();
        let body: Element = match stage.read().clone() {
            Stage::Start => rect()
                .width(Size::fill())
                .spacing(16.)
                .child(
                    label()
                        .color(colors.text_secondary)
                        .text("The mint will give you a bitcoin address. Any amount sent to it once confirmed becomes ecash."),
                )
                .child(
                    wide_button(if busy() { "Asking the mint…" } else { "Get an address" })
                        .filled()
                        .enabled(!busy())
                        .on_press(create),
                )
                .into_element(),
            Stage::Waiting(deposit) => rect()
                .width(Size::fill())
                .spacing(16.)
                .cross_align(Alignment::center())
                .child(QrCard {
                    data: deposit.address.clone(),
                })
                .child(
                    label()
                        .color(colors.text_secondary)
                        .text("Send bitcoin to this address. The ecash arrives once the transaction is confirmed; keep the app open until then."),
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
                        .on_press(move |_| stage.set(Stage::Start)),
                )
                .into_element(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Onchain deposit", true))
            .child(rect().width(Size::fill()).padding((4., 16., 24., 16.)).child(body))
            .into_element()
    }
}
