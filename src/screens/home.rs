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
        let adding = use_state(|| false);

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
                rect()
                    .width(Size::fill())
                    .spacing(24.)
                    .child(Total { sats: total })
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::wrap_spacing(12.))
                            .spacing(12.)
                            .children(list.iter().map(|mint| {
                                rect()
                                    .key(mint.url.clone())
                                    .child(MintCard {
                                        mint: mint.clone(),
                                    })
                                    .into_element()
                            }))
                            .child(AddTile { adding }),
                    )
                    .maybe(list.is_empty(), |el| {
                        el.child(empty_state("Add a mint to start using the wallet."))
                    })
                    .maybe(adding(), |el| el.child(AddMint { adding }))
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
        .width(Size::fill())
        .padding((14., 0., 0., 0.))
        .child(
            label()
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .text("Praline"),
        )
}

/// The balance over every mint.
#[derive(PartialEq)]
struct Total {
    sats: u64,
}

impl Component for Total {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::fill())
            .spacing(2.)
            .child(
                label()
                    .font_size(13.)
                    .color(palette().text_secondary)
                    .text("Balance"),
            )
            .child(
                label()
                    .font_size(40.)
                    .font_weight(FontWeight::BOLD)
                    .text(fmt::sats(self.sats)),
            )
    }
}

const CARD: f32 = 148.;

/// A mint as a small square card, so they sit side by side and wrap.
#[derive(PartialEq)]
struct MintCard {
    mint: MintSummary,
}

impl Component for MintCard {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let selected = use_consume::<SelectedMint>().0;
        let mint = self.mint.clone();
        let url = mint.url.clone();
        rect()
            .width(Size::px(CARD))
            .height(Size::px(CARD))
            .padding(14.)
            .corner_radius(18.)
            .background(colors.surface_secondary)
            .main_align(Alignment::space_between())
            .cursor(CursorIcon::Pointer)
            .on_press({
                let mut selected = selected;
                move |_| {
                    selected.set(Some(url.clone()));
                    let _ = RouterContext::get().push(Route::Mint);
                }
            })
            .child(icon(lucide::landmark(), 22., ACCENT))
            .child(
                rect()
                    .width(Size::fill())
                    .spacing(2.)
                    .child(
                        label()
                            .font_size(20.)
                            .font_weight(FontWeight::BOLD)
                            .max_lines(1)
                            .text(fmt::sats(mint.balance_sat)),
                    )
                    .child(
                        label()
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .text(mint.name.clone()),
                    ),
            )
    }
}

/// The last card of the grid: opens and closes the add-mint form.
#[derive(PartialEq)]
struct AddTile {
    adding: State<bool>,
}

impl Component for AddTile {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let mut adding = self.adding;
        rect()
            .width(Size::px(CARD))
            .height(Size::px(CARD))
            .corner_radius(18.)
            .border(
                Border::new()
                    .fill(colors.border)
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            )
            .center()
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| adding.set(!adding()))
            .child(icon(
                if adding() { lucide::x() } else { lucide::plus() },
                26.,
                colors.text_secondary,
            ))
    }
}

#[derive(PartialEq)]
struct AddMint {
    adding: State<bool>,
}

impl Component for AddMint {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let url = use_state(String::new);
        let adding = self.adding;
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
                let (mut url, mut adding) = (url, adding);
                spawn(async move {
                    match engine.add_mint(&address).await {
                        Ok(_) => {
                            url.set(String::new());
                            adding.set(false);
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
