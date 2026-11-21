use freya::{
    clipboard::Clipboard,
    icons::lucide,
    prelude::*,
    router::RouterContext,
};
use qrcode::{
    QrCode,
    render::svg,
};

use crate::{
    engine::{
        Engine,
        TxKind,
        TxStatus,
        TxView,
    },
    fmt,
    state::{
        Refresh,
    },
};

pub const ACCENT: Color = Color::from_rgb(201, 138, 84);
pub const POSITIVE: Color = Color::from_rgb(76, 200, 120);
pub const NEGATIVE: Color = Color::from_rgb(235, 95, 95);

pub fn palette() -> ColorsSheet {
    get_theme_or_default().read().colors.clone()
}

pub fn use_engine() -> Engine {
    use_consume::<Engine>()
}

pub fn use_refresh() -> State<u64> {
    use_consume::<Refresh>().0
}

/// Tell every screen the wallet's data changed.
pub fn bump(mut refresh: State<u64>) {
    let next = refresh() + 1;
    refresh.set(next);
}

pub fn icon(bytes: Bytes, size: f32, color: Color) -> SvgViewer {
    SvgViewer::new(bytes)
        .stroke(color)
        .width(Size::px(size))
        .height(Size::px(size))
}

/// Round icon badge used for list rows and actions.
pub fn icon_badge(bytes: Bytes, size: f32, color: Color, background: Color) -> Rect {
    rect()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(size / 2.)
        .background(background)
        .center()
        .child(icon(bytes, size * 0.5, color))
}

/// Screen title row, with an optional back button.
pub fn page_header(title: impl Into<String>, back: bool) -> Rect {
    let colors = palette();
    rect()
        .horizontal()
        .width(Size::fill())
        .padding((14., 16., 10., 16.))
        .spacing(8.)
        .cross_align(Alignment::center())
        .maybe(back, |el| {
            el.child(
                rect()
                    .padding(6.)
                    .corner_radius(99.)
                    .background(colors.surface_secondary)
                    .cursor(CursorIcon::Pointer)
                    .on_press(|_| RouterContext::get().go_back())
                    .child(icon(lucide::chevron_left(), 22., colors.text_primary)),
            )
        })
        .child(
            label()
                .font_size(24.)
                .font_weight(FontWeight::BOLD)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .text(title.into()),
        )
}

/// The title of a list on a screen.
pub fn heading(text: &'static str) -> Label {
    label()
        .font_size(18.)
        .font_weight(FontWeight::BOLD)
        .margin((0., 4.))
        .text(text)
}

/// A rounded surface with `rows` under each other, divided.
pub fn card(rows: Vec<Rect>) -> Rect {
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

/// A titled group of content on a rounded surface.
pub fn section(title: &'static str, content: impl Into<Element>) -> Rect {
    let colors = palette();
    rect()
        .width(Size::fill())
        .spacing(8.)
        .child(
            label()
                .font_size(13.)
                .font_weight(FontWeight::BOLD)
                .color(colors.text_secondary)
                .text(title.to_uppercase())
                .margin((0., 4.)),
        )
        .child(
            rect()
                .width(Size::fill())
                .background(colors.surface_secondary)
                .corner_radius(16.)
                .padding((4., 14.))
                .child(content),
        )
}

pub fn kv_row(key: impl Into<String>, value: impl Into<String>) -> Rect {
    let colors = palette();
    let value = value.into();
    rect()
        .horizontal()
        .width(Size::fill())
        .padding((10., 0.))
        .spacing(12.)
        .cross_align(Alignment::center())
        .child(label().color(colors.text_secondary).text(key.into()))
        .child(
            SelectableText::new()
                .width(Size::flex(1.))
                .text_align(TextAlign::End)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .span(value),
        )
}

pub fn divider() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(palette().border)
}

pub fn pill(text: impl Into<String>, color: Color) -> Rect {
    rect()
        .padding((3., 8.))
        .corner_radius(99.)
        .background(color.with_a(40))
        .child(
            label()
                .font_size(11.)
                .font_weight(FontWeight::BOLD)
                .color(color)
                .text(text.into()),
        )
}

pub fn loading() -> Rect {
    rect()
        .width(Size::fill())
        .padding(32.)
        .center()
        .child(label().color(palette().text_secondary).text("Loading…"))
}

pub fn notice(text: impl Into<String>, color: Color) -> Rect {
    rect()
        .width(Size::fill())
        .padding(12.)
        .corner_radius(12.)
        .background(color.with_a(36))
        .child(label().font_size(14.).color(color).text(text.into()))
}

pub fn empty_state(text: &'static str) -> Rect {
    rect()
        .width(Size::fill())
        .padding((28., 12.))
        .center()
        .child(label().color(palette().text_secondary).text(text))
}

pub fn qr_svg(data: &str) -> Option<Bytes> {
    let code = QrCode::new(data.to_uppercase().as_bytes()).ok()?;
    let image = code
        .render::<svg::Color>()
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Some(Bytes::from(image))
}

/// A QR code of `data`, and a row that copies it.
#[derive(PartialEq)]
pub struct QrCard {
    pub data: String,
}

impl Component for QrCard {
    fn render(&self) -> impl IntoElement {
        let mut copied = use_state(|| false);
        let colors = palette();
        let data = self.data.clone();
        let qr = use_memo({
            let data = self.data.clone();
            move || qr_svg(&data)
        });

        rect()
            .width(Size::fill())
            .spacing(14.)
            .cross_align(Alignment::center())
            .child(
                rect()
                    .padding(14.)
                    .corner_radius(24.)
                    .background(Color::WHITE)
                    .map(qr.read().clone(), |el, bytes| {
                        el.child(
                            SvgViewer::new((self.data.clone(), bytes))
                                .width(Size::px(260.))
                                .height(Size::px(260.)),
                        )
                    }),
            )
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::flex())
                    .cross_align(Alignment::center())
                    .spacing(10.)
                    .padding((12., 14.))
                    .corner_radius(14.)
                    .background(colors.surface_secondary)
                    .cursor(CursorIcon::Pointer)
                    .on_press(move |_| {
                        let _ = Clipboard::set(data.clone());
                        let _ = Clipboard::set_primary(data.clone());
                        copied.set(true);
                    })
                    .child(
                        label()
                            .width(Size::flex(1.))
                            .font_size(13.)
                            .max_lines(1)
                            .text(fmt::short(&self.data)),
                    )
                    .child(icon(
                        if copied() {
                            lucide::check()
                        } else {
                            lucide::copy()
                        },
                        18.,
                        if copied() { POSITIVE } else { ACCENT },
                    )),
            )
    }
}

/// A button that shows a pointer cursor on hover.
pub fn button() -> Button {
    Button::new().cursor(CursorIcon::Pointer)
}

/// A wide button.
pub fn wide_button(text: impl Into<String>) -> Button {
    button()
        .expanded()
        .width(Size::fill())
        .child(text.into())
}

#[derive(Clone, Copy, PartialEq)]
enum Key {
    Digit(u64),
    DoubleZero,
    Backspace,
}

const KEYPAD: [[Key; 3]; 4] = [
    [Key::Digit(1), Key::Digit(2), Key::Digit(3)],
    [Key::Digit(4), Key::Digit(5), Key::Digit(6)],
    [Key::Digit(7), Key::Digit(8), Key::Digit(9)],
    [Key::DoubleZero, Key::Digit(0), Key::Backspace],
];

const MAX_DIGITS: usize = 10;

/// The amount in sats, typed on a keypad: the soft keyboard of Android is too limited to type
/// into fields there.
#[derive(PartialEq)]
pub struct AmountKeypad {
    pub amount: State<u64>,
    pub presets: &'static [u64],
}

impl Component for AmountKeypad {
    fn render(&self) -> impl IntoElement {
        let mut amount = self.amount;
        let colors = palette();
        let value = amount();
        let press_key = move |key: Key| {
            let current = amount();
            let next = match key {
                Key::Digit(d) if current.to_string().len() < MAX_DIGITS => current * 10 + d,
                Key::DoubleZero if current.to_string().len() + 2 <= MAX_DIGITS => current * 100,
                Key::Backspace => current / 10,
                _ => current,
            };
            amount.set_if_modified(next);
        };

        rect()
            .width(Size::fill())
            .spacing(16.)
            .child(
                rect()
                    .width(Size::fill())
                    .padding((8., 0.))
                    .center()
                    .spacing(10.)
                    .child(
                        label()
                            .font_size(44.)
                            .font_weight(FontWeight::BOLD)
                            .color(if value == 0 {
                                colors.text_placeholder
                            } else {
                                colors.text_primary
                            })
                            .text(fmt::sats(value)),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .spacing(8.)
                            .children(self.presets.iter().map(|&preset| {
                                Chip::new()
                                    .cursor(CursorIcon::Pointer)
                                    .selected(value == preset)
                                    .on_press(move |_| amount.set(preset))
                                    .child(fmt::group_thousands(preset))
                            })),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .spacing(8.)
                    .children(KEYPAD.map(|row| {
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .spacing(8.)
                            .children(row.map(|key| keypad_button(key, press_key)))
                    })),
            )
    }
}

fn keypad_button(key: Key, mut on_press: impl FnMut(Key) + 'static) -> Rect {
    let colors = palette();
    rect()
        .width(Size::flex(1.))
        .height(Size::px(52.))
        .center()
        .corner_radius(14.)
        .background(colors.surface_secondary)
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| on_press(key))
        .child(match key {
            Key::Digit(d) => label()
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .text(d.to_string())
                .into_element(),
            Key::DoubleZero => label()
                .font_size(22.)
                .font_weight(FontWeight::BOLD)
                .text("00")
                .into_element(),
            Key::Backspace => icon(lucide::delete(), 24., colors.text_primary).into_element(),
        })
}

/// One transaction in a list.
#[derive(PartialEq)]
pub struct TxRow {
    pub tx: TxView,
    pub show_mint: bool,
}

impl Component for TxRow {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let tx = &self.tx;
        let (glyph, tint) = match tx.kind {
            TxKind::Deposit => (lucide::zap(), POSITIVE),
            TxKind::Withdraw => (lucide::zap(), NEGATIVE),
            TxKind::TokenSent => (lucide::arrow_up_right(), NEGATIVE),
            TxKind::TokenReceived => (lucide::arrow_down_left(), POSITIVE),
            TxKind::CassisSent => (lucide::arrow_up_right(), NEGATIVE),
            TxKind::CassisReceived => (lucide::arrow_down_left(), POSITIVE),
        };
        let amount_color = match tx.status {
            TxStatus::Failed => colors.text_secondary,
            _ if tx.kind.incoming() => POSITIVE,
            _ => colors.text_primary,
        };
        let amount = fmt::signed_sats(tx.amount_sat, tx.kind.incoming());
        let status = match tx.status {
            TxStatus::Pending => Some(pill("Pending", ACCENT)),
            TxStatus::Failed => Some(pill("Failed", NEGATIVE)),
            TxStatus::Done => None,
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::flex())
            .padding((10., 0.))
            .spacing(12.)
            .cross_align(Alignment::center())
            .child(icon_badge(glyph, 40., tint, tint.with_a(36)))
            .child(
                rect()
                    .width(Size::flex(1.))
                    .spacing(3.)
                    .child(
                        rect()
                            .horizontal()
                            .spacing(8.)
                            .cross_align(Alignment::center())
                            .child(
                                label()
                                    .font_size(12.)
                                    .font_weight(FontWeight::BOLD)
                                    .color(tint)
                                    .text(tx.kind.label()),
                            )
                            .maybe_child(status),
                    )
                    .child(
                        label()
                            .font_size(15.)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .text(tx.description.clone()),
                    )
                    .child(
                        label()
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .text(if self.show_mint {
                                format!("{} · {}", tx.mint_name, fmt::when(tx.at))
                            } else {
                                fmt::when(tx.at)
                            }),
                    ),
            )
            .child(
                label()
                    .font_size(16.)
                    .font_weight(FontWeight::BOLD)
                    .color(amount_color)
                    .text(amount),
            )
    }
}

/// Rows for a list of transactions, with dividers between them.
pub fn tx_list(txs: &[TxView], show_mint: bool) -> Rect {
    rect().width(Size::fill()).children(txs.iter().enumerate().map(|(i, tx)| {
        rect()
            .key(tx.id.clone())
            .width(Size::fill())
            .maybe(i > 0, |el| el.child(divider()))
            .child(TxRow {
                tx: tx.clone(),
                show_mint,
            })
            .into_element()
    }))
}

/// A dimmed overlay over the window with `content` centered on it: the wallet's own screens that
/// a page cannot imitate (payments). Taps on the dim part do nothing.
pub fn overlay(content: impl Into<Element>) -> Rect {
    rect()
        .layer(Layer::Overlay)
        .position(Position::new_global().top(0.).left(0.))
        .width(Size::window_percent(100.))
        .height(Size::window_percent(100.))
        .center()
        .padding(16.)
        .background(Color::BLACK.with_a(170))
        .child(content)
}
