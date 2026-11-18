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
        let mut adding = use_state(|| false);

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
                let hidden = settings.read().hide_balance;
                rect()
                    .width(Size::fill())
                    .spacing(20.)
                    .child(
                        rect()
                            .width(Size::fill())
                            .spacing(8.)
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .content(Content::flex())
                                    .cross_align(Alignment::center())
                                    .margin((0., 4.))
                                    .child(
                                        label()
                                            .width(Size::flex(1.))
                                            .font_size(18.)
                                            .font_weight(FontWeight::BOLD)
                                            .text("Mints"),
                                    )
                                    .child(
                                        rect()
                                            .padding(6.)
                                            .corner_radius(16.)
                                            .on_press(move |_| adding.set(!adding()))
                                            .child(icon(
                                                if adding() { lucide::x() } else { lucide::plus() },
                                                22.,
                                                ACCENT,
                                            )),
                                    ),
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
        .horizontal()
        .width(Size::fill())
        .padding((14., 0., 0., 0.))
        .cross_align(Alignment::center())
        .spacing(10.)
        .child(
            label()
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .text("Praline"),
        )
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
                    ),
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
