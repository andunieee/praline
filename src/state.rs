use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Usd,
    Eur,
    Gbp,
    Chf,
    Jpy,
    Brl,
}

impl Currency {
    pub const ALL: [Currency; 6] = [
        Currency::Usd,
        Currency::Eur,
        Currency::Gbp,
        Currency::Chf,
        Currency::Jpy,
        Currency::Brl,
    ];

    pub fn symbol(self) -> &'static str {
        match self {
            Currency::Usd => "$",
            Currency::Eur => "€",
            Currency::Gbp => "£",
            Currency::Chf => "CHF ",
            Currency::Jpy => "¥",
            Currency::Brl => "R$",
        }
    }

    pub fn btc_price(self) -> f64 {
        match self {
            Currency::Usd => 98_450.,
            Currency::Eur => 91_120.,
            Currency::Gbp => 77_830.,
            Currency::Chf => 86_900.,
            Currency::Jpy => 15_020_000.,
            Currency::Brl => 571_300.,
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Chf => "CHF",
            Currency::Jpy => "JPY",
            Currency::Brl => "BRL",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Sats,
    Btc,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl Network {
    pub const ALL: [Network; 4] = [
        Network::Mainnet,
        Network::Testnet,
        Network::Signet,
        Network::Regtest,
    ];
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Network::Mainnet => "Mainnet",
            Network::Testnet => "Testnet",
            Network::Signet => "Signet",
            Network::Regtest => "Regtest",
        })
    }
}

#[derive(Clone, PartialEq)]
pub struct Settings {
    pub currency: Currency,
    pub unit: Unit,
    pub network: Network,
    pub server: String,
    pub lsp: String,
    pub use_tor: bool,
    pub hide_balance: bool,
    pub dark_mode: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            currency: Currency::Usd,
            unit: Unit::Sats,
            network: Network::Mainnet,
            server: "ssl://electrum.blockstream.info:50002".to_string(),
            lsp: "https://lsp.example.com".to_string(),
            use_tor: false,
            hide_balance: false,
            dark_mode: true,
        }
    }
}
