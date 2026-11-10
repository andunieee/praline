use freya::{
    icons::lucide,
    prelude::*,
    router::RouterContext,
};

use crate::{
    app::Route,
    mock,
    ui::*,
};

#[derive(Clone, Copy, PartialEq)]
enum PayStatus {
    Review,
    Sending,
    Sent,
}

#[derive(PartialEq)]
pub struct PayScreen {
    pub invoice: String,
}

impl Component for PayScreen {
    fn render(&self) -> impl IntoElement {
        let settings = use_settings();
        let mut status = use_state(|| PayStatus::Review);
        let (unit, currency) = {
            let s = settings.read();
            (s.unit, s.currency)
        };
        let colors = palette();
        let decoded = mock::decode_invoice(&self.invoice);
        let total = (decoded.amount_sat + decoded.fee_sat) as i64;

        if status() == PayStatus::Sent {
            return rect()
                .expanded()
                .center()
                .spacing(14.)
                .padding(24.)
                .child(icon_badge(lucide::check(), 96., POSITIVE, POSITIVE.with_a(40)))
                .child(
                    label()
                        .font_size(24.)
                        .font_weight(FontWeight::BOLD)
                        .text("Payment sent"),
                )
                .child(
                    label()
                        .color(colors.text_secondary)
                        .text(format!(
                            "{} to {}",
                            mock::format_amount(decoded.amount_sat as i64, unit),
                            decoded.payee
                        )),
                )
                .child(
                    rect().width(Size::px(220.)).padding((16., 0., 0., 0.)).child(
                        Button::new()
                            .expanded()
                            .width(Size::fill())
                            .filled()
                            .on_press(|_| {
                                let _ = RouterContext::get().replace(Route::Home);
                            })
                            .child("Done"),
                    ),
                )
                .into_element();
        }

        let sending = status() == PayStatus::Sending;

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Confirm payment", true))
            .child(
                rect()
                    .width(Size::fill())
                    .padding((4., 16., 24., 16.))
                    .spacing(20.)
                    .child(
                        rect()
                            .width(Size::fill())
                            .center()
                            .spacing(6.)
                            .padding((18., 0.))
                            .child(icon_badge(lucide::zap(), 56., ACCENT, ACCENT.with_a(40)))
                            .child(
                                label()
                                    .font_size(34.)
                                    .font_weight(FontWeight::BOLD)
                                    .text(mock::format_amount(decoded.amount_sat as i64, unit)),
                            )
                            .child(
                                label()
                                    .color(colors.text_secondary)
                                    .text(format!(
                                        "≈ {}",
                                        mock::format_fiat(decoded.amount_sat as i64, currency)
                                    )),
                            ),
                    )
                    .child(section(
                        "Details",
                        rect()
                            .width(Size::fill())
                            .child(kv_row("Description", decoded.description))
                            .child(divider())
                            .child(kv_row("Pay to", decoded.payee.clone()))
                            .child(divider())
                            .child(kv_row("Expires in", decoded.expires_in))
                            .child(divider())
                            .child(kv_row(
                                "Network fee",
                                format!("~{}", mock::format_amount(decoded.fee_sat as i64, unit)),
                            ))
                            .child(divider())
                            .child(kv_row("Total", mock::format_amount(total, unit))),
                    ))
                    .child(section(
                        "Invoice",
                        paragraph()
                            .width(Size::fill())
                            .padding((10., 0.))
                            .font_size(12.)
                            .color(colors.text_secondary)
                            .max_lines(4)
                            .text_overflow(TextOverflow::Ellipsis)
                            .span(self.invoice.clone()),
                    ))
                    .child(
                        Button::new()
                            .expanded()
                            .width(Size::fill())
                            .filled()
                            .enabled(!sending)
                            .on_press(move |_| {
                                status.set(PayStatus::Sending);
                                spawn(async move {
                                    async_io::Timer::after(std::time::Duration::from_millis(1200))
                                        .await;
                                    status.set(PayStatus::Sent);
                                });
                            })
                            .child(if sending { "Sending…" } else { "Confirm & pay" }),
                    ),
            )
            .into_element()
    }
}
