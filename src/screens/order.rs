use std::{
    cell::RefCell,
    rc::Rc,
};

use freya::{
    icons::lucide,
    prelude::*,
};
use freya_cop::{
    CopEvent,
    CopSession,
    CopView,
    InvoiceOffer,
    InvoiceRequest,
};

use crate::{
    engine::{
        Engine,
        PaymentOffer,
        store::{
            Flow,
            FlowSession,
            unix_now,
        },
    },
    fmt,
    state::PendingFlow,
    ui::*,
};

/// What the wallet asks the payer, over the page.
#[derive(Clone)]
enum Sheet {
    Pay {
        offer: PaymentOffer,
        memo: Option<String>,
    },
    /// The server offers money.
    Receive {
        request: InvoiceRequest,
        mints: Vec<(String, String)>,
    },
    Message(Result<String, String>),
}

/// An order flow: the server's page, and the wallet's own screens for what it asks to be paid.
#[derive(PartialEq)]
pub struct OrderScreen;

impl Component for OrderScreen {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let mut pending = use_consume::<PendingFlow>().0;
        let sheet = use_state(|| None::<Sheet>);

        // The session lives as long as this screen: leaving it ends the flow.
        let session = use_hook({
            let engine = engine.clone();
            move || {
                let request = pending.write().take()?;
                let session = CopSession::open(&engine.handle(), request.uri, request.session, None);
                remember(&engine, &session);
                Some(Rc::new(RefCell::new(session)))
            }
        });
        let Some(session) = session else {
            return rect()
                .expanded()
                .child(page_header("Order", true))
                .child(rect().padding(16.).child(notice("There is no order to open.", NEGATIVE)))
                .into_element();
        };

        let on_event = {
            let (engine, session) = (engine.clone(), session.clone());
            move |event: CopEvent| {
                remember(&engine, &session.borrow());
                handle_event(&engine, sheet, event);
            }
        };

        let (name, endpoint, via_relay) = {
            let mut s = session.borrow_mut();
            let name = s.name();
            (name, s.uri().endpoint.clone(), s.uri().relay.is_some())
        };
        let colors = palette();

        rect()
            .expanded()
            .child(
                ScrollView::new()
                    .width(Size::fill())
                    .height(Size::fill())
                    .child(page_header(
                        name.clone().unwrap_or_else(|| "Order".to_owned()),
                        true,
                    ))
                    .child(
                        rect()
                            .width(Size::fill())
                            .padding((0., 16., 24., 16.))
                            .spacing(14.)
                            .child(
                                // What identifies a server is its key, not what it calls itself.
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .spacing(8.)
                                    .cross_align(Alignment::center())
                                    .child(icon(lucide::shield_check(), 16., colors.text_secondary))
                                    .child(
                                        label()
                                            .font_size(12.)
                                            .color(colors.text_secondary)
                                            .max_lines(1)
                                            .text(format!(
                                                "Server {}{}",
                                                fmt::short(&endpoint),
                                                if via_relay { " · custom relay" } else { "" }
                                            )),
                                    ),
                            )
                            .child(CopView::new(session.clone(), on_event)),
                    ),
            )
            .maybe_child(sheet.read().clone().map(|s| {
                SheetView {
                    sheet: s,
                    state: sheet,
                    session: session.clone(),
                    peer: name,
                }
                .into_element()
            }))
            .into_element()
    }
}

/// Keep the flow in the list of orders, so it can be reopened with the same session.
fn remember(engine: &Engine, session: &CopSession) {
    let flow = Flow {
        session: FlowSession(session.session().to_owned()),
        uri: session.uri().to_string(),
        name: session.uri().name().map(str::to_owned),
        last_opened: unix_now(),
    };
    let _ = engine.db().flows().upsert(&flow);
}

fn handle_event(
    engine: &Engine,
    mut sheet: State<Option<Sheet>>,
    event: CopEvent,
) {
    match event {
        CopEvent::Invoice(InvoiceOffer { invoice, memo }) => {
            let engine = engine.clone();
            spawn(async move {
                match engine.offer(invoice).await {
                    Ok(offer) => sheet.set(Some(Sheet::Pay { offer, memo })),
                    Err(e) => sheet.set(Some(Sheet::Message(Err(e)))),
                }
            });
        }
        CopEvent::InvoiceRequest(request) => {
            let engine = engine.clone();
            spawn(async move {
                match engine.receivable_mints(&request.networks).await {
                    Ok(mints) => sheet.set(Some(Sheet::Receive { request, mints })),
                    Err(e) => sheet.set(Some(Sheet::Message(Err(e)))),
                }
            });
        }
        CopEvent::Closed(_) => {}
    }
}

struct SheetView {
    sheet: Sheet,
    state: State<Option<Sheet>>,
    session: Rc<RefCell<CopSession>>,
    peer: Option<String>,
}

impl PartialEq for SheetView {
    fn eq(&self, other: &Self) -> bool {
        self.sheet == other.sheet
            && self.state == other.state
            && Rc::ptr_eq(&self.session, &other.session)
            && self.peer == other.peer
    }
}

impl PartialEq for Sheet {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Sheet::Pay { offer: a, memo: x }, Sheet::Pay { offer: b, memo: y }) => a == b && x == y,
            (
                Sheet::Receive { request: a, mints: x },
                Sheet::Receive { request: b, mints: y },
            ) => a == b && x == y,
            (Sheet::Message(a), Sheet::Message(b)) => a == b,
            _ => false,
        }
    }
}

impl Component for SheetView {
    fn render(&self) -> impl IntoElement {
        let colors = palette();
        let mut state = self.state;
        let dismiss = move |_| state.set(None);
        let card = rect()
            .vertical()
            .width(Size::fill())
            .max_width(Size::px(440.))
            .padding(20.)
            .spacing(14.)
            .corner_radius(20.)
            .background(colors.surface_secondary);

        match &self.sheet {
            Sheet::Pay { offer, memo } => overlay(
                card.child(PayCard {
                    offer: offer.clone(),
                    memo: memo.clone(),
                    state,
                    peer: self.peer.clone(),
                }),
            ),
            Sheet::Receive { request, mints } => overlay(
                card.child(ReceiveCard {
                    request: request.clone(),
                    mints: mints.clone(),
                    state,
                    session: self.session.clone(),
                    peer: self.peer.clone(),
                }),
            ),
            Sheet::Message(result) => {
                let (text, color) = match result {
                    Ok(t) => (t.clone(), POSITIVE),
                    Err(e) => (e.clone(), NEGATIVE),
                };
                overlay(
                    card.child(notice(text, color))
                        .child(wide_button("Close").outline().on_press(dismiss)),
                )
            }
        }
    }
}

/// The wallet's payment screen: amount and payee from the invoice, never from the page.
#[derive(PartialEq)]
struct PayCard {
    offer: PaymentOffer,
    memo: Option<String>,
    state: State<Option<Sheet>>,
    peer: Option<String>,
}

impl Component for PayCard {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let colors = palette();
        let mut chosen = use_state(|| 0usize);
        let mut busy = use_state(|| false);
        let mut state = self.state;
        let offer = self.offer.clone();
        let options = offer.options.clone();
        let current = options.get(chosen()).cloned();
        let can_pay = current.as_ref().is_some_and(|o| o.enough) && !busy();

        let pay = {
            let (engine, offer, memo, peer) =
                (engine.clone(), offer.clone(), self.memo.clone(), self.peer.clone());
            let current = current.clone();
            move |_| {
                let Some(option) = current.clone() else { return };
                busy.set(true);
                let (engine, offer, memo, peer) =
                    (engine.clone(), offer.clone(), memo.clone(), peer.clone());
                spawn(async move {
                    let outcome = engine.pay(&offer, &option.mint_url, peer, memo).await;
                    bump(refresh);
                    busy.set(false);
                    state.set(Some(Sheet::Message(outcome.map(|paid| {
                        if paid.fee_sat > 0 {
                            format!("Paid {} (fee {})", fmt::sats(paid.amount_sat), fmt::sats(paid.fee_sat))
                        } else {
                            format!("Paid {}", fmt::sats(paid.amount_sat))
                        }
                    }))));
                });
            }
        };

        rect()
            .width(Size::fill())
            .spacing(14.)
            .child(
                label()
                    .font_size(13.)
                    .color(colors.text_secondary)
                    .text("Payment request"),
            )
            .child(
                label()
                    .font_size(34.)
                    .font_weight(FontWeight::BOLD)
                    .text(fmt::sats(offer.amount_sat)),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .child(kv_row("To", fmt::short(&offer.payee)))
                    .maybe_child(
                        offer
                            .description
                            .clone()
                            .map(|d| kv_row("For", d).into_element()),
                    )
                    .maybe_child(self.memo.clone().map(|m| kv_row("Note", m).into_element()))
                    .child(kv_row("Expires", fmt::when(offer.expires_at))),
            )
            .child(if options.is_empty() {
                notice(
                    "None of your mints can pay this. Add a mint on one of the invoice's networks.",
                    NEGATIVE,
                )
                .into_element()
            } else {
                rect()
                    .width(Size::fill())
                    .spacing(6.)
                    .child(label().font_size(13.).color(colors.text_secondary).text("Pay from"))
                    .children(options.iter().enumerate().map(|(i, option)| {
                        let selected = chosen() == i;
                        rect()
                            .key(option.mint_url.clone())
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::flex())
                            .padding((10., 12.))
                            .spacing(8.)
                            .corner_radius(12.)
                            .background(if selected {
                                ACCENT.with_a(40)
                            } else {
                                colors.surface_tertiary
                            })
                            .on_press(move |_| chosen.set(i))
                            .child(
                                label()
                                    .width(Size::flex(1.))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .text(option.mint_name.clone()),
                            )
                            .child(
                                label()
                                    .color(if option.enough { colors.text_secondary } else { NEGATIVE })
                                    .text(if option.enough {
                                        fmt::sats(option.balance_sat)
                                    } else {
                                        format!("{} · not enough", fmt::sats(option.balance_sat))
                                    }),
                            )
                            .into_element()
                    }))
                    .into_element()
            })
            .child(
                wide_button(if busy() { "Paying…" } else { "Pay" })
                    .filled()
                    .enabled(can_pay)
                    .on_press(pay),
            )
            .child(
                wide_button("Not now")
                    .flat()
                    .enabled(!busy())
                    .on_press(move |_| state.set(None)),
            )
    }
}

/// The server offers money: the wallet makes an invoice for it, if the payer accepts.
struct ReceiveCard {
    request: InvoiceRequest,
    mints: Vec<(String, String)>,
    state: State<Option<Sheet>>,
    session: Rc<RefCell<CopSession>>,
    peer: Option<String>,
}

impl PartialEq for ReceiveCard {
    fn eq(&self, other: &Self) -> bool {
        self.request == other.request
            && self.mints == other.mints
            && self.state == other.state
            && Rc::ptr_eq(&self.session, &other.session)
            && self.peer == other.peer
    }
}

impl Component for ReceiveCard {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let colors = palette();
        let mut chosen = use_state(|| 0usize);
        let mut busy = use_state(|| false);
        let mut state = self.state;
        let request = self.request.clone();
        let mints = self.mints.clone();

        let accept = {
            let (engine, request, mints, session, peer) = (
                engine.clone(),
                request.clone(),
                mints.clone(),
                self.session.clone(),
                self.peer.clone(),
            );
            move |_| {
                let Some((url, _)) = mints.get(chosen()).cloned() else { return };
                busy.set(true);
                let (engine, request, session, peer) =
                    (engine.clone(), request.clone(), session.clone(), peer.clone());
                spawn(async move {
                    let outcome = engine
                        .invoice_for_offer(
                            &url,
                            request.amount_sat,
                            request.memo.clone(),
                            request.expires_in,
                            peer,
                        )
                        .await;
                    bump(refresh);
                    busy.set(false);
                    state.set(Some(Sheet::Message(outcome.map(|invoice| {
                        session.borrow_mut().send_invoice(invoice);
                        format!(
                            "Invoice sent. {} will arrive when the server pays it.",
                            fmt::sats(request.amount_sat)
                        )
                    }))));
                });
            }
        };

        rect()
            .width(Size::fill())
            .spacing(14.)
            .child(label().font_size(13.).color(colors.text_secondary).text("The server offers you"))
            .child(
                label()
                    .font_size(34.)
                    .font_weight(FontWeight::BOLD)
                    .color(POSITIVE)
                    .text(fmt::sats(request.amount_sat)),
            )
            .maybe_child(request.memo.clone().map(|m| kv_row("For", m).into_element()))
            .child(if mints.is_empty() {
                notice(
                    format!(
                        "You have no mint on any network it can pay from ({}).",
                        request.networks.join(", ")
                    ),
                    NEGATIVE,
                )
                .into_element()
            } else {
                rect()
                    .width(Size::fill())
                    .spacing(6.)
                    .child(label().font_size(13.).color(colors.text_secondary).text("Receive at"))
                    .children(mints.iter().enumerate().map(|(i, (url, name))| {
                        rect()
                            .key(url.clone())
                            .width(Size::fill())
                            .padding((10., 12.))
                            .corner_radius(12.)
                            .background(if chosen() == i {
                                ACCENT.with_a(40)
                            } else {
                                colors.surface_tertiary
                            })
                            .on_press(move |_| chosen.set(i))
                            .child(label().max_lines(1).text(name.clone()))
                            .into_element()
                    }))
                    .into_element()
            })
            .child(
                wide_button(if busy() { "Making the invoice…" } else { "Accept" })
                    .filled()
                    .enabled(!mints.is_empty() && !busy())
                    .on_press(accept),
            )
            .child(
                wide_button("Decline")
                    .flat()
                    .enabled(!busy())
                    .on_press(move |_| state.set(None)),
            )
    }
}

