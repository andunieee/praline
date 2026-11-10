use freya::{
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    app::Route,
    mock::{
        self,
        Transaction,
        TxKind,
    },
    ui::*,
};

#[derive(PartialEq)]
pub struct HomeScreen;

impl Component for HomeScreen {
    fn render(&self) -> impl IntoElement {
        let transactions = use_hook(mock::transactions);

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .padding((0., 16., 24., 16.))
                    .spacing(20.)
                    .child(top_bar())
                    .child(BalanceCard)
                    .child(action_row())
                    .child(
                        rect()
                            .width(Size::fill())
                            .spacing(4.)
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .content(Content::flex())
                                    .padding((0., 4., 4., 4.))
                                    .child(
                                        label()
                                            .width(Size::flex(1.))
                                            .font_size(18.)
                                            .font_weight(FontWeight::BOLD)
                                            .text("Transactions"),
                                    )
                                    .child(
                                        label()
                                            .font_size(13.)
                                            .color(palette().text_secondary)
                                            .text(format!("{} total", transactions.len())),
                                    ),
                            )
                            .children(
                                transactions
                                    .iter()
                                    .map(|tx| TransactionRow::new(tx.clone()).key(tx.id)),
                            ),
                    ),
            )
    }
}

fn top_bar() -> Rect {
    let colors = palette();
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::flex())
        .padding((14., 0., 0., 0.))
        .cross_align(Alignment::center())
        .spacing(10.)
        .child(icon_badge(lucide::bitcoin(), 36., Color::WHITE, ACCENT))
        .child(
            label()
                .width(Size::flex(1.))
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .text("Caskade"),
        )
        .child(
            rect()
                .horizontal()
                .spacing(6.)
                .cross_align(Alignment::center())
                .child(
                    rect()
                        .width(Size::px(8.))
                        .height(Size::px(8.))
                        .corner_radius(4.)
                        .background(POSITIVE),
                )
                .child(
                    label()
                        .font_size(12.)
                        .color(colors.text_secondary)
                        .text(format!("Synced · {}", mock::BLOCK_HEIGHT)),
                ),
        )
}

#[derive(PartialEq)]
struct BalanceCard;

impl Component for BalanceCard {
    fn render(&self) -> impl IntoElement {
        let mut settings = use_settings();
        let (unit, currency, hidden) = {
            let s = settings.read();
            (s.unit, s.currency, s.hide_balance)
        };

        let (main, fiat, pending) = if hidden {
            ("••••••".to_string(), "Tap to reveal".to_string(), None)
        } else {
            (
                mock::format_amount(mock::BALANCE_SAT, unit),
                format!("≈ {}", mock::format_fiat(mock::BALANCE_SAT, currency)),
                Some(mock::format_signed_amount(mock::PENDING_SAT, unit)),
            )
        };

        rect()
            .width(Size::fill())
            .padding(22.)
            .spacing(6.)
            .corner_radius(24.)
            .background(
                LinearGradient::new()
                    .angle(135.)
                    .stop(((255, 166, 48), 0.))
                    .stop(((236, 112, 18), 100.)),
            )
            .color(Color::WHITE)
            .on_press(move |_| {
                let mut s = settings.write();
                s.hide_balance = !s.hide_balance;
            })
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::flex())
                    .cross_align(Alignment::center())
                    .child(
                        label()
                            .width(Size::flex(1.))
                            .font_size(14.)
                            .color(Color::from_af32rgb(0.85, 255, 255, 255))
                            .text("Total balance"),
                    )
                    .child(icon(
                        if hidden {
                            lucide::eye_off()
                        } else {
                            lucide::eye()
                        },
                        18.,
                        Color::WHITE,
                    )),
            )
            .child(
                label()
                    .font_size(34.)
                    .font_weight(FontWeight::BOLD)
                    .text(main),
            )
            .child(
                label()
                    .font_size(15.)
                    .color(Color::from_af32rgb(0.85, 255, 255, 255))
                    .text(fiat),
            )
            .map(pending, |el, pending| {
                el.child(
                    rect()
                        .margin((10., 0., 0., 0.))
                        .horizontal()
                        .spacing(6.)
                        .cross_align(Alignment::center())
                        .padding((5., 10.))
                        .corner_radius(99.)
                        .background(Color::from_af32rgb(0.2, 0, 0, 0))
                        .child(icon(lucide::clock(), 14., Color::WHITE))
                        .child(label().font_size(12.).text(format!("{pending} pending"))),
                )
            })
    }
}

fn action_row() -> Rect {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::flex())
        .spacing(12.)
        .child(action_button(
            "Send",
            lucide::scan_line(),
            Route::Send,
        ))
        .child(action_button(
            "Receive",
            lucide::arrow_down_left(),
            Route::Receive,
        ))
}

fn action_button(text: &'static str, bytes: Bytes, route: Route) -> Rect {
    let colors = palette();
    rect()
        .width(Size::flex(1.))
        .horizontal()
        .center()
        .spacing(8.)
        .padding(16.)
        .corner_radius(16.)
        .background(colors.surface_secondary)
        .on_press(move |_| {
            let _ = RouterContext::get().push(route.clone());
        })
        .child(icon(bytes, 20., ACCENT))
        .child(label().font_weight(FontWeight::BOLD).text(text))
}

#[derive(PartialEq)]
struct TransactionRow {
    tx: Transaction,
    key: DiffKey,
}

impl TransactionRow {
    fn new(tx: Transaction) -> Self {
        Self {
            tx,
            key: DiffKey::None,
        }
    }
}

impl KeyExt for TransactionRow {
    fn key_mut(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for TransactionRow {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings();
        let (unit, currency, hidden) = {
            let s = settings.read();
            (s.unit, s.currency, s.hide_balance)
        };
        let colors = palette();
        let tx = &self.tx;

        let incoming = tx.amount_sat > 0;
        let (badge_icon, badge_color) = match (tx.kind, incoming) {
            (TxKind::Lightning, _) => (lucide::zap(), ACCENT),
            (TxKind::Onchain, true) => (lucide::arrow_down_left(), POSITIVE),
            (TxKind::Onchain, false) => (lucide::arrow_up_right(), NEGATIVE),
        };
        let amount_color = if incoming {
            POSITIVE
        } else {
            colors.text_primary
        };
        let (amount, fiat) = if hidden {
            ("•••".to_string(), String::new())
        } else {
            (
                mock::format_signed_amount(tx.amount_sat, unit),
                mock::format_fiat(tx.amount_sat.abs(), currency),
            )
        };
        let subtitle = match tx.kind {
            TxKind::Lightning => format!("{} · Lightning", tx.when),
            TxKind::Onchain => format!("{} · On-chain", tx.when),
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::flex())
            .cross_align(Alignment::center())
            .spacing(12.)
            .padding((10., 4.))
            .child(icon_badge(badge_icon, 40., badge_color, badge_color.with_a(36)))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .spacing(3.)
                    .child(
                        rect()
                            .horizontal()
                            .spacing(6.)
                            .cross_align(Alignment::center())
                            .child(
                                label()
                                    .font_weight(FontWeight::BOLD)
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .text(tx.label),
                            )
                            .maybe(tx.pending, |el| el.child(pill("Pending", ACCENT))),
                    )
                    .child(
                        label()
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .text(subtitle),
                    ),
            )
            .child(
                rect()
                    .cross_align(Alignment::end())
                    .spacing(3.)
                    .child(
                        label()
                            .font_weight(FontWeight::BOLD)
                            .color(amount_color)
                            .text(amount),
                    )
                    .child(
                        label()
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .text(fiat),
                    ),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
