use freya::{
    clipboard::Clipboard,
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    fmt,
    screens::use_selected_mint,
    ui::*,
};

const PRESETS: &[u64] = &[100, 1_000, 5_000, 21_000];

#[derive(Clone, PartialEq)]
enum Sent {
    Asking,
    Token(String, u64),
    Failed(String),
}

/// Ecash out of the mint as a token to hand to someone.
#[derive(PartialEq)]
pub struct SendTokenScreen;

impl Component for SendTokenScreen {
    fn render(&self) -> impl IntoElement {
        let Some(url) = use_selected_mint() else {
            return page_header("Send token", true).into_element();
        };
        let engine = use_engine();
        let refresh = use_refresh();
        let amount = use_state(|| 0u64);
        let mut sent = use_state(|| Sent::Asking);
        let mut busy = use_state(|| false);

        let make = move |_| {
            if amount() == 0 || busy() {
                return;
            }
            busy.set(true);
            let (engine, url, sats) = (engine.clone(), url.clone(), amount());
            spawn(async move {
                match engine.send_token(&url, sats, None).await {
                    Ok(token) => {
                        bump(refresh);
                        sent.set(Sent::Token(token, sats));
                    }
                    Err(e) => sent.set(Sent::Failed(e)),
                }
                busy.set(false);
            });
        };

        let body: Element = match sent.read().clone() {
            Sent::Asking => rect()
                .width(Size::fill())
                .spacing(16.)
                .child(AmountKeypad {
                    amount,
                    presets: PRESETS,
                })
                .child(
                    wide_button(if busy() { "Making the token…" } else { "Create token" })
                        .filled()
                        .enabled(amount() > 0 && !busy())
                        .on_press(make),
                )
                .into_element(),
            Sent::Token(token, sats) => rect()
                .width(Size::fill())
                .spacing(16.)
                .cross_align(Alignment::center())
                .child(
                    label()
                        .font_size(28.)
                        .font_weight(FontWeight::BOLD)
                        .text(fmt::sats(sats)),
                )
                .child(QrCard { data: token })
                .child(
                    label()
                        .color(palette().text_secondary)
                        .text("A token is money: whoever has it can redeem it."),
                )
                .child(wide_button("Done").filled().on_press(|_| {
                    RouterContext::get().go_back();
                }))
                .into_element(),
            Sent::Failed(error) => rect()
                .width(Size::fill())
                .spacing(12.)
                .child(notice(error, NEGATIVE))
                .child(
                    wide_button("Try again")
                        .outline()
                        .on_press(move |_| sent.set(Sent::Asking)),
                )
                .into_element(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Send token", true))
            .child(rect().width(Size::fill()).padding((4., 16., 24., 16.)).child(body))
            .into_element()
    }
}

/// A token somebody gave: redeemed into the wallet.
#[derive(PartialEq)]
pub struct ReceiveTokenScreen;

impl Component for ReceiveTokenScreen {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let mut token = use_state(String::new);
        let mut busy = use_state(|| false);
        let mut result = use_state(|| None::<Result<u64, String>>);

        #[cfg(target_os = "android")]
        {
            let mut scan = crate::scanner::use_scan_result();
            use_side_effect(move || {
                let scanned = scan.read().clone();
                if let Some(contents) = scanned {
                    scan.set(None);
                    token.set(contents.trim().to_owned());
                }
            });
        }

        let redeem = move |_| {
            let text = token.read().trim().to_owned();
            if text.is_empty() || busy() {
                return;
            }
            busy.set(true);
            result.set(None);
            let engine = engine.clone();
            spawn(async move {
                let outcome = engine.receive_token(&text).await.map(|(_, sats)| sats);
                if outcome.is_ok() {
                    token.set(String::new());
                    bump(refresh);
                }
                result.set(Some(outcome));
                busy.set(false);
            });
        };

        let outcome: Option<Element> = result.read().clone().map(|r| match r {
            Ok(sats) => notice(format!("Received {}", fmt::sats(sats)), POSITIVE).into_element(),
            Err(e) => notice(e, NEGATIVE).into_element(),
        });

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Redeem a token", true))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((4., 16., 24., 16.))
                    .spacing(12.)
                    .child(
                        Input::new(token)
                            .width(Size::fill())
                            .placeholder("cashuA… / cashuB…"),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .spacing(8.)
                            .child(
                                Button::new()
                                    .width(Size::flex(1.))
                                    .outline()
                                    .on_press(move |_| {
                                        if let Ok(text) = Clipboard::get() {
                                            token.set(text.trim().to_owned());
                                        }
                                    })
                                    .child("Paste"),
                            )
                            .maybe(cfg!(target_os = "android"), |el| {
                                el.child(
                                    Button::new()
                                        .width(Size::flex(1.))
                                        .outline()
                                        .on_press(|_| {
                                            #[cfg(target_os = "android")]
                                            crate::scanner::start();
                                        })
                                        .child(icon(lucide::scan_line(), 18., palette().primary)),
                                )
                            }),
                    )
                    .maybe_child(outcome)
                    .child(
                        wide_button(if busy() { "Redeeming…" } else { "Redeem" })
                            .filled()
                            .enabled(!token.read().trim().is_empty() && !busy())
                            .on_press(redeem),
                    ),
            )
    }
}
