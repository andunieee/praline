//! Fake wallet data. Everything here is made up and will be replaced by a real backend.

use std::fmt::Write;

use crate::state::{
    Currency,
    Unit,
};

pub const BALANCE_SAT: i64 = 1_284_517;
pub const PENDING_SAT: i64 = 25_000;
pub const BLOCK_HEIGHT: u64 = 921_337;

#[derive(Clone, Copy, PartialEq)]
pub enum TxKind {
    Lightning,
    Onchain,
}

#[derive(Clone, PartialEq)]
pub struct Transaction {
    pub id: u32,
    pub label: &'static str,
    pub amount_sat: i64,
    pub fee_sat: u64,
    pub when: &'static str,
    pub kind: TxKind,
    pub pending: bool,
}

pub fn transactions() -> Vec<Transaction> {
    use TxKind::*;
    [
        ("Coffee at Satoshi's", -4_200, 3, "Today, 09:41", Lightning, false),
        ("Incoming payment", 25_000, 0, "Today, 08:12", Onchain, true),
        ("Split dinner", 18_500, 0, "Yesterday", Lightning, false),
        ("VPN subscription", -12_000, 8, "Yesterday", Lightning, false),
        ("Salary stack", 500_000, 0, "Nov 3", Onchain, false),
        ("Podcast boost", -2_100, 1, "Nov 2", Lightning, false),
        ("Channel open", -210_000, 1_840, "Oct 30", Onchain, false),
        ("Refund", 7_777, 0, "Oct 28", Lightning, false),
        ("Hardware wallet", -95_000, 2_310, "Oct 21", Onchain, false),
        ("Tip from a friend", 1_000, 0, "Oct 20", Lightning, false),
        ("Deposit", 1_050_000, 0, "Oct 12", Onchain, false),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (label, amount_sat, fee_sat, when, kind, pending))| Transaction {
        id: i as u32,
        label,
        amount_sat,
        fee_sat,
        when,
        kind,
        pending,
    })
    .collect()
}

#[derive(Clone, PartialEq)]
pub struct Utxo {
    pub outpoint: String,
    pub address: &'static str,
    pub value_sat: u64,
    pub confirmations: u32,
    pub frozen: bool,
}

pub fn utxos() -> Vec<Utxo> {
    [
        ("9f2c1ab47e", 0, "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh", 812_340, 1_204, false),
        ("44be09d1c3", 1, "bc1q9h7garjwz6yqs3mz0m4rmsh5n8hxk9w3cn6qve", 300_000, 312, false),
        ("d0a771e2f9", 0, "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq", 120_177, 58, true),
        ("1c5e3b8a02", 2, "bc1qc7slrfxkknqcq2jevvvkdgvrt8080852dfjewde", 27_000, 6, false),
        ("e83f62d415", 0, "bc1q34aq5drpuwy3wgl9lhup9892qp6svr8ldzyy7c", 25_000, 0, false),
    ]
    .into_iter()
    .map(|(txid, vout, address, value_sat, confirmations, frozen)| Utxo {
        outpoint: format!("{txid}…:{vout}"),
        address,
        value_sat,
        confirmations,
        frozen,
    })
    .collect()
}

#[derive(Clone, PartialEq)]
pub struct Channel {
    pub alias: &'static str,
    pub capacity_sat: u64,
    pub local_sat: u64,
    pub active: bool,
}

pub fn channels() -> Vec<Channel> {
    vec![
        Channel {
            alias: "ACINQ",
            capacity_sat: 500_000,
            local_sat: 312_400,
            active: true,
        },
        Channel {
            alias: "Voltage LSP",
            capacity_sat: 210_000,
            local_sat: 18_900,
            active: true,
        },
        Channel {
            alias: "zap.stream",
            capacity_sat: 100_000,
            local_sat: 61_000,
            active: false,
        },
    ]
}

pub struct FeeEstimate {
    pub target: &'static str,
    pub sat_per_vb: u32,
}

pub fn fee_estimates() -> [FeeEstimate; 3] {
    [
        FeeEstimate {
            target: "Next block",
            sat_per_vb: 14,
        },
        FeeEstimate {
            target: "~30 min",
            sat_per_vb: 9,
        },
        FeeEstimate {
            target: "~1 hour",
            sat_per_vb: 4,
        },
    ]
}

pub fn sync_log() -> [&'static str; 6] {
    [
        "18:42:07  electrum: connected to ssl://electrum.blockstream.info:50002",
        "18:42:08  chain: tip at 921337 (000000…3f9a)",
        "18:42:09  wallet: scanned 412 scripts, 5 unspent",
        "18:42:11  ln: 2/3 channels active, 1 peer offline",
        "18:42:11  gossip: rapid sync 14.2k channels",
        "18:43:00  fees: refreshed estimates",
    ]
}

pub const NODE_ID: &str = "03a1f2c9e4b7d6085e1c3f2a9b8d7e6f5a4b3c2d1e0f9a8b7c6d5e4f3a2b1c0d9e";

const BECH32: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// Generates a random string shaped like a BOLT11 invoice. It does not decode to anything.
pub fn random_invoice(amount_sat: u64) -> String {
    let mut invoice = String::from("lnbc");
    if amount_sat > 0 {
        let _ = write!(invoice, "{}n", amount_sat * 10);
    }
    invoice.push('1');
    for _ in 0..240 {
        invoice.push(BECH32[fastrand::usize(..BECH32.len())] as char);
    }
    invoice
}

pub struct DecodedInvoice {
    pub amount_sat: u64,
    pub description: &'static str,
    pub payee: String,
    pub expires_in: &'static str,
    pub fee_sat: u64,
}

/// Pretends to decode an invoice. The amount is read from the human readable part if present,
/// everything else is derived from the string so the same invoice always looks the same.
pub fn decode_invoice(invoice: &str) -> DecodedInvoice {
    let seed = invoice
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    let mut rng = fastrand::Rng::with_seed(seed);

    let lower = invoice.to_lowercase();
    let amount_sat = lower
        .strip_prefix("lnbc")
        .and_then(|rest| {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let multiplier = rest.chars().nth(digits.len())?;
            let value: u64 = digits.parse().ok()?;
            match multiplier {
                'm' => Some(value * 100_000),
                'u' => Some(value * 100),
                'n' => Some(value / 10),
                _ => None,
            }
        })
        .unwrap_or_else(|| rng.u64(1_000..80_000));

    let descriptions = [
        "Coffee and a croissant",
        "Invoice #4021",
        "Donation, thank you!",
        "Concert tickets x2",
        "Pizza night",
    ];

    let payee = (0..16)
        .map(|_| format!("{:x}", rng.u8(..16)))
        .collect::<String>();

    DecodedInvoice {
        amount_sat,
        description: descriptions[rng.usize(..descriptions.len())],
        payee: format!("02{payee}"),
        expires_in: "59 minutes",
        fee_sat: (amount_sat / 1000).max(1),
    }
}

pub fn mock_clipboard_invoice() -> String {
    random_invoice(fastrand::u64(1..50) * 1000)
}

fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn format_amount(sat: i64, unit: Unit) -> String {
    let sign = if sat < 0 { "-" } else { "" };
    let abs = sat.unsigned_abs();
    match unit {
        Unit::Sats => format!("{sign}{} sat", group_thousands(abs)),
        Unit::Btc => format!("{sign}₿{}.{:08}", abs / 100_000_000, abs % 100_000_000),
    }
}

pub fn format_signed_amount(sat: i64, unit: Unit) -> String {
    let formatted = format_amount(sat, unit);
    if sat > 0 { format!("+{formatted}") } else { formatted }
}

pub fn format_fiat(sat: i64, currency: Currency) -> String {
    let value = sat as f64 / 100_000_000. * currency.btc_price();
    let whole = value.abs().trunc() as u64;
    let cents = ((value.abs().fract() * 100.).round() as u64).min(99);
    let sign = if value < 0. { "-" } else { "" };
    if currency == Currency::Jpy {
        format!("{sign}{}{}", currency.symbol(), group_thousands(whole))
    } else {
        format!("{sign}{}{}.{cents:02}", currency.symbol(), group_thousands(whole))
    }
}
