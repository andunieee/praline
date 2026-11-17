use freya::prelude::State;
use freya_cop::CopUri;

#[derive(Clone, PartialEq)]
pub struct Settings {
    pub hide_balance: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hide_balance: false,
        }
    }
}

/// The mint the mint screens are about.
#[derive(Clone, Copy, PartialEq)]
pub struct SelectedMint(pub State<Option<String>>);

/// A flow to open: the order screen takes it when it appears.
#[derive(Clone, PartialEq)]
pub struct FlowRequest {
    pub uri: CopUri,
    /// The session to reopen, or none for a new one.
    pub session: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct PendingFlow(pub State<Option<FlowRequest>>);

/// Bumped when the wallet's data changed, so screens load it again.
#[derive(Clone, Copy, PartialEq)]
pub struct Refresh(pub State<u64>);

/// A Lightning invoice to pay, for the withdraw screen to pick up.
#[derive(Clone, Copy, PartialEq)]
pub struct PendingInvoice(pub State<Option<String>>);
