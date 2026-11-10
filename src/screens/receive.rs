use freya::{
    clipboard::Clipboard,
    icons::lucide,
    prelude::*,
};
use qrcode::{
    QrCode,
    render::svg,
};

use crate::{
    mock,
    ui::*,
};

const MAX_DIGITS: usize = 10;
const PRESETS: [u64; 4] = [1_000, 5_000, 21_000, 100_000];

#[derive(PartialEq)]
pub struct ReceiveScreen;

impl Component for ReceiveScreen {
    fn render(&self) -> impl IntoElement {
        let amount = use_state(|| 0u64);
        let invoice = use_state(|| None::<String>);

        rect().expanded().child(match invoice.read().clone() {
            Some(code) => InvoiceView {
                amount: amount(),
                invoice: code,
                on_reset: invoice,
            }
            .into_element(),
            None => AmountPicker { amount, invoice }.into_element(),
        })
    }
}

#[derive(PartialEq)]
struct AmountPicker {
    amount: State<u64>,
    invoice: State<Option<String>>,
}

impl Component for AmountPicker {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings();
        let (unit, currency) = {
            let s = settings.read();
            (s.unit, s.currency)
        };
        let colors = palette();
        let mut amount = self.amount;
        let mut invoice = self.invoice;
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
            .expanded()
            .content(Content::flex())
            .child(page_header("Receive", false))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .center()
                    .spacing(6.)
                    .child(
                        label()
                            .font_size(44.)
                            .font_weight(FontWeight::BOLD)
                            .color(if value == 0 {
                                colors.text_placeholder
                            } else {
                                colors.text_primary
                            })
                            .text(mock::format_amount(value as i64, unit)),
                    )
                    .child(
                        label()
                            .color(colors.text_secondary)
                            .text(format!("≈ {}", mock::format_fiat(value as i64, currency))),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .spacing(8.)
                            .margin((14., 0., 0., 0.))
                            .children(PRESETS.map(|preset| {
                                Chip::new()
                                    .selected(value == preset)
                                    .on_press(move |_| amount.set(preset))
                                    .child(format!("{}k", preset / 1000))
                            })),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .padding((0., 16., 16., 16.))
                    .spacing(8.)
                    .children(KEYPAD.map(|row| {
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .spacing(8.)
                            .children(row.map(|key| keypad_button(key, press_key)))
                    }))
                    .child(
                        rect().width(Size::fill()).padding((8., 0., 0., 0.)).child(
                            Button::new()
                                .expanded()
                                .width(Size::fill())
                                .filled()
                                .on_press(move |_| invoice.set(Some(mock::random_invoice(amount()))))
                                .child(if value == 0 {
                                    "Create invoice (any amount)"
                                } else {
                                    "Create invoice"
                                }),
                        ),
                    ),
            )
    }
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

fn keypad_button(key: Key, mut on_press: impl FnMut(Key) + 'static) -> Rect {
    let colors = palette();
    rect()
        .width(Size::flex(1.))
        .height(Size::px(52.))
        .center()
        .corner_radius(14.)
        .background(colors.surface_secondary)
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

fn qr_svg(data: &str) -> Option<Bytes> {
    let code = QrCode::new(data.to_uppercase().as_bytes()).ok()?;
    let image = code
        .render::<svg::Color>()
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Some(Bytes::from(image))
}

#[derive(PartialEq)]
struct InvoiceView {
    amount: u64,
    invoice: String,
    on_reset: State<Option<String>>,
}

impl Component for InvoiceView {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings();
        let mut copied = use_state(|| false);
        let unit = settings.read().unit;
        let colors = palette();
        let mut on_reset = self.on_reset;
        let invoice = self.invoice.clone();
        let short = format!(
            "{}…{}",
            &self.invoice[..14],
            &self.invoice[self.invoice.len() - 10..]
        );
        let qr = use_memo({
            let invoice = self.invoice.clone();
            move || qr_svg(&invoice)
        });

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Receive", false))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((4., 16., 24., 16.))
                    .spacing(16.)
                    .cross_align(Alignment::center())
                    .child(
                        rect()
                            .padding(14.)
                            .corner_radius(24.)
                            .background(Color::WHITE)
                            .map(qr.read().clone(), |el, bytes| {
                                el.child(
                                    SvgViewer::new((self.invoice.clone(), bytes))
                                        .width(Size::px(260.))
                                        .height(Size::px(260.)),
                                )
                            }),
                    )
                    .child(
                        label()
                            .font_size(28.)
                            .font_weight(FontWeight::BOLD)
                            .text(if self.amount == 0 {
                                "Any amount".to_string()
                            } else {
                                mock::format_amount(self.amount as i64, unit)
                            }),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .spacing(6.)
                            .cross_align(Alignment::center())
                            .child(icon(lucide::clock(), 14., colors.text_secondary))
                            .child(
                                label()
                                    .font_size(13.)
                                    .color(colors.text_secondary)
                                    .text("Expires in 60 minutes · Waiting for payment"),
                            ),
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
                            .on_press(move |_| {
                                let _ = Clipboard::set(invoice.clone());
                                copied.set(true);
                            })
                            .child(
                                label()
                                    .width(Size::flex(1.))
                                    .font_size(13.)
                                    .max_lines(1)
                                    .text(short),
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
                    .child(
                        Button::new()
                            .expanded()
                            .width(Size::fill())
                            .outline()
                            .on_press(move |_| on_reset.set(None))
                            .child("New invoice"),
                    ),
            )
    }
}
