use freya::{
    prelude::*,
    router::RouterContext,
};

use crate::{
    app::Route,
    engine::{MintProofs, MintSummary, ProofHolder},
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
                    let proofs = engine.proofs(&url).await?;
                    Ok::<_, String>((summary, txs, proofs))
                }
            }
        });

        let body: Element = match &*data.state() {
            FutureState::Fulfilled(Ok((summary, txs, proofs))) => {
                rect()
                    .width(Size::fill())
                    .spacing(18.)
                    .child(Header {
                        mint: summary.clone(),
                    })
                    .maybe_child((!proofs.locked.is_empty()).then(|| locked_proofs(proofs)))
                    .child(held_proofs(proofs))
                    .child(
                        rect()
                            .width(Size::fill())
                            .spacing(8.)
                            .child(heading("Transactions"))
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
            FutureState::Fulfilled(Ok((summary, _, _))) => summary.name.clone(),
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

fn heading(text: &'static str) -> Label {
    label()
        .font_size(18.)
        .font_weight(FontWeight::BOLD)
        .margin((0., 4.))
        .text(text)
}

/// A rounded surface with `rows` under each other, divided.
fn card(rows: Vec<Rect>) -> Rect {
    rect()
        .width(Size::fill())
        .padding((4., 14.))
        .corner_radius(16.)
        .background(palette().surface_secondary)
        .children(rows.into_iter().enumerate().map(|(i, row)| {
            rect()
                .width(Size::fill())
                .maybe(i > 0, |el| el.child(divider()))
                .child(row)
                .into_element()
        }))
}

/// One proof: what it is worth on the right, and what to know about it under `title`.
fn proof_row(title: String, detail: String, amount_sat: u64) -> Rect {
    let colors = palette();
    rect()
        .horizontal()
        .width(Size::fill())
        .padding((10., 0.))
        .spacing(12.)
        .cross_align(Alignment::center())
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(label().font_size(15.).text(title))
                .child(
                    label()
                        .font_size(12.)
                        .color(colors.text_secondary)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .text(detail),
                ),
        )
        .child(
            label()
                .font_size(16.)
                .font_weight(FontWeight::BOLD)
                .text(fmt::sats(amount_sat)),
        )
}

fn locked_proofs(proofs: &MintProofs) -> Rect {
    rect()
        .width(Size::fill())
        .spacing(8.)
        .child(heading("Pending HTLC proofs"))
        .child(card(
            proofs
                .locked
                .iter()
                .map(|p| {
                    proof_row(
                        format!("HTLC …{}", &p.payment_hash[p.payment_hash.len().saturating_sub(8)..]),
                        format!("Refundable {}", fmt::when(p.refundable_at)),
                        p.amount_sat,
                    )
                })
                .collect(),
        ))
}

fn held_proofs(proofs: &MintProofs) -> Rect {
    rect()
        .width(Size::fill())
        .spacing(8.)
        .child(heading("Proofs"))
        .child(if proofs.held.is_empty() {
            empty_state("No proofs.")
        } else {
            card(
                proofs
                    .held
                    .iter()
                    .map(|p| {
                        let holder = match p.holder {
                            ProofHolder::Wallet => "Wallet",
                            ProofHolder::Cassis => "Cassis",
                        };
                        let state = if p.reserved { " · in use" } else { "" };
                        proof_row(holder.to_owned(), format!("Keyset {}{state}", p.keyset), p.amount_sat)
                    })
                    .collect(),
            )
        })
}

#[derive(PartialEq)]
struct Header {
    mint: MintSummary,
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
                .cursor(CursorIcon::Pointer)
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
                            .text(fmt::sats(self.mint.balance_sat)),
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
                    .width(Size::fill())
                    .spacing(8.)
                    .child(row([
                        Some(action("Lightning deposit", Route::Deposit)),
                        Some(action("Lightning withdraw", Route::Withdraw)),
                    ]))
                    .maybe_child((self.mint.onchain_deposit || self.mint.onchain_withdraw).then(|| {
                        row([
                            self.mint
                                .onchain_deposit
                                .then(|| action("Onchain deposit", Route::OnchainDeposit)),
                            self.mint
                                .onchain_withdraw
                                .then(|| action("Onchain withdraw", Route::OnchainWithdraw)),
                        ])
                    }))
                    .child(row([
                        Some(action("Send token", Route::SendToken)),
                        Some(action("Redeem", Route::ReceiveToken)),
                    ])),
            )
    }
}

/// A row of actions, each taking an equal share of the width.
fn row(actions: impl IntoIterator<Item = Option<Rect>>) -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::flex())
        .spacing(8.)
        .children(actions.into_iter().flatten().map(IntoElement::into_element))
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
