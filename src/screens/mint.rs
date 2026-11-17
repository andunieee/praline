use freya::{
    prelude::*,
    router::RouterContext,
};

use crate::{
    app::Route,
    engine::MintSummary,
    fmt,
    state::SelectedMint,
    ui::*,
};

/// The mint the mint screens are about, or a way back if there is none.
pub fn use_selected_mint() -> Option<String> {
    use_consume::<SelectedMint>().0.read().clone()
}

#[derive(PartialEq)]
pub struct MintScreen;

impl Component for MintScreen {
    fn render(&self) -> impl IntoElement {
        let Some(url) = use_selected_mint() else {
            return rect().expanded().child(page_header("Mint", true)).into_element();
        };
        let engine = use_engine();
        let refresh = use_refresh();
        let settings = use_settings();

        let data = use_future({
            let (engine, url) = (engine.clone(), url.clone());
            move || {
                let _ = refresh();
                let (engine, url) = (engine.clone(), url.clone());
                async move {
                    let mints = engine.mints().await?;
                    let summary = mints
                        .into_iter()
                        .find(|m| m.url == url)
                        .ok_or_else(|| "this mint was removed".to_owned())?;
                    let txs = engine.transactions(Some(&url)).await?;
                    Ok::<_, String>((summary, txs))
                }
            }
        });

        let body: Element = match &*data.state() {
            FutureState::Fulfilled(Ok((summary, txs))) => {
                let hidden = settings.read().hide_balance;
                rect()
                    .width(Size::fill())
                    .spacing(18.)
                    .child(Header {
                        mint: summary.clone(),
                        hidden,
                    })
                    .child(
                        rect()
                            .width(Size::fill())
                            .spacing(8.)
                            .child(
                                label()
                                    .font_size(18.)
                                    .font_weight(FontWeight::BOLD)
                                    .margin((0., 4.))
                                    .text("Transactions"),
                            )
                            .child(if txs.is_empty() {
                                empty_state("Nothing here yet.").into_element()
                            } else {
                                rect()
                                    .width(Size::fill())
                                    .padding((4., 14.))
                                    .corner_radius(16.)
                                    .background(palette().surface_secondary)
                                    .child(tx_list(txs, false))
                                    .into_element()
                            }),
                    )
                    .child(RemoveMint {
                        url: url.clone(),
                        empty: summary.balance_sat == 0,
                    })
                    .into_element()
            }
            FutureState::Fulfilled(Err(error)) => notice(error.clone(), NEGATIVE).into_element(),
            _ => loading().into_element(),
        };

        let title = match &*data.state() {
            FutureState::Fulfilled(Ok((summary, _))) => summary.name.clone(),
            _ => "Mint".to_owned(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header(title, true))
            .child(rect().width(Size::fill()).padding((4., 16., 24., 16.)).child(body))
            .into_element()
    }
}

#[derive(PartialEq)]
struct Header {
    mint: MintSummary,
    hidden: bool,
}

impl Component for Header {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let action = |name: &'static str, route: Route| {
            rect()
                .width(Size::flex(1.))
                .padding((16., 4.))
                .center()
                .corner_radius(16.)
                .background(colors.surface_secondary)
                .on_press(move |_| {
                    let _ = RouterContext::get().push(route.clone());
                })
                .child(label().font_size(14.).font_weight(FontWeight::BOLD).text(name))
        };
        rect()
            .width(Size::fill())
            .spacing(16.)
            .child(
                rect()
                    .width(Size::fill())
                    .spacing(4.)
                    .center()
                    .child(
                        label()
                            .font_size(36.)
                            .font_weight(FontWeight::BOLD)
                            .text(if self.hidden {
                                "••••••".to_owned()
                            } else {
                                fmt::sats(self.mint.balance_sat)
                            }),
                    )
                    .child(
                        label()
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .text(self.mint.url.clone()),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::flex())
                    .spacing(8.)
                    .child(action("Deposit", Route::Deposit))
                    .child(action("Withdraw", Route::Withdraw))
                    .child(action("Send token", Route::SendToken))
                    .child(action("Redeem", Route::ReceiveToken)),
            )
    }
}

#[derive(PartialEq)]
struct RemoveMint {
    url: String,
    empty: bool,
}

impl Component for RemoveMint {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let mut error = use_state(|| None::<String>);
        let url = self.url.clone();
        rect()
            .width(Size::fill())
            .spacing(8.)
            .maybe_child(error.read().clone().map(|e| notice(e, NEGATIVE)))
            .child(
                wide_button(if self.empty {
                    "Remove this mint"
                } else {
                    "Remove (withdraw the funds first)"
                })
                .flat()
                .enabled(self.empty)
                .on_press(move |_| {
                    let (engine, url) = (engine.clone(), url.clone());
                    spawn(async move {
                        match engine.remove_mint(&url).await {
                            Ok(()) => {
                                bump(refresh);
                                RouterContext::get().go_back();
                            }
                            Err(e) => error.set(Some(e)),
                        }
                    });
                }),
            )
    }
}
