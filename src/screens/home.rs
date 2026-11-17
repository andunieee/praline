use freya::{
    icons::lucide,
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

#[derive(PartialEq)]
pub struct HomeScreen;

impl Component for HomeScreen {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let settings = use_settings();

        let mints = use_future({
            let engine = engine.clone();
            move || {
                let _ = refresh();
                let engine = engine.clone();
                async move { engine.mints().await }
            }
        });

        let body: Element = match &*mints.state() {
            FutureState::Fulfilled(Ok(list)) => {
                let total: u64 = list.iter().map(|m| m.balance_sat).sum();
                let hidden = settings.read().hide_balance;
                rect()
                    .width(Size::fill())
                    .spacing(20.)
                    .child(BalanceCard { total, hidden })
                    .child(
                        rect()
                            .width(Size::fill())
                            .spacing(8.)
                            .child(
                                label()
                                    .font_size(18.)
                                    .font_weight(FontWeight::BOLD)
                                    .margin((0., 4.))
                                    .text("Mints"),
                            )
                            .maybe(list.is_empty(), |el| {
                                el.child(empty_state("Add a mint to start using the wallet."))
                            })
                            .children(list.iter().map(|mint| {
                                rect()
                                    .key(mint.url.clone())
                                    .width(Size::fill())
                                    .child(MintRow {
                                        mint: mint.clone(),
                                        hidden,
                                    })
                                    .into_element()
                            })),
                    )
                    .child(AddMint)
                    .into_element()
            }
            FutureState::Fulfilled(Err(error)) => notice(error.clone(), NEGATIVE).into_element(),
            _ => loading().into_element(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .padding((0., 16., 24., 16.))
                    .spacing(20.)
                    .child(top_bar())
                    .child(body),
            )
    }
}

fn top_bar() -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .padding((14., 0., 0., 0.))
        .cross_align(Alignment::center())
        .spacing(10.)
        .child(icon_badge(lucide::wallet(), 36., Color::WHITE, ACCENT))
        .child(
            label()
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .text("Praline"),
        )
}

#[derive(PartialEq)]
struct BalanceCard {
    total: u64,
    hidden: bool,
}

impl Component for BalanceCard {
    fn render(&self) -> impl IntoElement {
        let mut settings = use_settings();
        let colors = palette();
        let hidden = self.hidden;
        rect()
            .width(Size::fill())
            .padding(20.)
            .spacing(6.)
            .corner_radius(24.)
            .background(
                LinearGradient::new()
                    .angle(135.)
                    .stop(((247, 147, 26), 0.))
                    .stop(((200, 100, 10), 100.)),
            )
            .on_press(move |_| settings.write().hide_balance = !hidden)
            .child(
                label()
                    .font_size(13.)
                    .color(Color::WHITE.with_a(200))
                    .text("Total balance"),
            )
            .child(
                label()
                    .font_size(34.)
                    .font_weight(FontWeight::BOLD)
                    .color(Color::WHITE)
                    .text(if hidden {
                        "••••••".to_owned()
                    } else {
                        fmt::sats(self.total)
                    }),
            )
            .child(
                label()
                    .font_size(12.)
                    .color(colors.text_inverse.with_a(180))
                    .text(if hidden {
                        "Tap to show"
                    } else {
                        "Across all mints · tap to hide"
                    }),
            )
    }
}

#[derive(PartialEq)]
struct MintRow {
    mint: MintSummary,
    hidden: bool,
}

impl Component for MintRow {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let selected = use_consume::<SelectedMint>().0;
        let mint = self.mint.clone();
        let url = mint.url.clone();
        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::flex())
            .padding(14.)
            .spacing(12.)
            .corner_radius(16.)
            .background(colors.surface_secondary)
            .cross_align(Alignment::center())
            .on_press({
                let mut selected = selected;
                move |_| {
                    selected.set(Some(url.clone()));
                    let _ = RouterContext::get().push(Route::Mint);
                }
            })
            .child(icon_badge(lucide::landmark(), 42., ACCENT, ACCENT.with_a(36)))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .spacing(3.)
                    .child(
                        label()
                            .font_size(16.)
                            .font_weight(FontWeight::BOLD)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .text(mint.name.clone()),
                    )
                    .child(
                        label()
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .text(mint.url.clone()),
                    )
                    .maybe(mint.cassis, |el| el.child(pill("Cassis", POSITIVE))),
            )
            .child(
                label()
                    .font_size(16.)
                    .font_weight(FontWeight::BOLD)
                    .text(if self.hidden {
                        "••••".to_owned()
                    } else {
                        fmt::sats(mint.balance_sat)
                    }),
            )
    }
}

#[derive(PartialEq)]
struct AddMint;

impl Component for AddMint {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let url = use_state(String::new);
        let mut busy = use_state(|| false);
        let mut error = use_state(|| None::<String>);

        let mut add = {
            let engine = engine.clone();
            move || {
                let address = url.read().trim().to_owned();
                if address.is_empty() || busy() {
                    return;
                }
                busy.set(true);
                error.set(None);
                let engine = engine.clone();
                let mut url = url;
                spawn(async move {
                    match engine.add_mint(&address).await {
                        Ok(_) => {
                            url.set(String::new());
                            bump(refresh);
                        }
                        Err(e) => error.set(Some(e)),
                    }
                    busy.set(false);
                });
            }
        };

        section(
            "Add a mint",
            rect()
                .width(Size::fill())
                .padding((10., 0.))
                .spacing(10.)
                .child(
                    Input::new(url)
                        .width(Size::fill())
                        .placeholder("mint.example.com")
                        .on_submit({
                            let mut add = add.clone();
                            move |_: String| add()
                        }),
                )
                .maybe_child(error.read().clone().map(|e| notice(e, NEGATIVE)))
                .child(
                    wide_button(if busy() { "Connecting…" } else { "Add mint" })
                        .filled()
                        .enabled(!busy() && !url.read().trim().is_empty())
                        .on_press(move |_| add()),
                ),
        )
    }
}
