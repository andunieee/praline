use freya::prelude::*;

use crate::ui::*;

#[derive(PartialEq)]
pub struct TransactionsScreen;

impl Component for TransactionsScreen {
    fn render(&self) -> impl IntoElement {
        let engine = use_engine();
        let refresh = use_refresh();
        let txs = use_future({
            let engine = engine.clone();
            move || {
                let _ = refresh();
                let engine = engine.clone();
                async move { engine.transactions(None).await }
            }
        });

        let body: Element = match &*txs.state() {
            FutureState::Fulfilled(Ok(list)) if list.is_empty() => {
                empty_state("No transactions yet.").into_element()
            }
            FutureState::Fulfilled(Ok(list)) => rect()
                .width(Size::fill())
                .padding((4., 14.))
                .corner_radius(16.)
                .background(palette().surface_secondary)
                .child(tx_list(list, true))
                .into_element(),
            FutureState::Fulfilled(Err(error)) => notice(error.clone(), NEGATIVE).into_element(),
            _ => loading().into_element(),
        };

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(page_header("Transactions", false))
            .child(rect().width(Size::fill()).padding((4., 16., 24., 16.)).child(body))
    }
}
