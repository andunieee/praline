# Praline

A [Cashu](https://cashu.space) wallet for desktop and Android, built with [Freya](https://github.com/marc2332/freya), whose main way to pay is [Cassis](../cassis) through the [Cassis Order Protocol](../cassis_order_protocol) (COP).

- **Mints**: the home screen lists the mints you added with their balances. Tapping one shows its balance, its transactions and what you can do there: deposit and withdraw over Lightning (mint and melt), send a token, redeem one.
- **Activity**: every transaction, tagged by type (deposit, withdraw, token sent or received, Cassis sent or received), with its amount, description and mint.
- **Pay**: scan or paste a `cop://` address or a web address that publishes one, a Lightning invoice or a cashu token. An order is a hypereffect page the merchant streams; when it sends an invoice, the wallet shows its own payment screen for it, and when it offers money (a refund) the wallet makes an invoice for it.
- One recovery phrase covers everything: cdk's seed and the Cassis keys derive from it.

## Layout

```
src/
  engine/        the wallet without a UI
    store.rs       orangedb: Cassis payments, saved order flows, and cdk's WalletDatabase
    mints.rs       cdk wallets: add mint, deposit (mint), withdraw (melt), tokens
    cassis.rs      Cassis node: pay an invoice, answer an invoicerequest
    transactions.rs  the lists
  app.rs         theme, routes, shell with bottom navigation
  screens/       home (mints), mint, transactions, deposit, withdraw, token, pay, order, settings
  ui.rs          shared widgets
```

cdk ships no database, only the `WalletDatabase` trait, so `CdkStore` implements it on orangedb: cdk's types are stored as JSON rows, indexed by kind and mint, in `praline.redb` next to the wallet's own tables. The Cassis node keeps its own `cassis/wallet.redb` and `cassis/config.toml`.

A mint's Cassis network (`cashu::<host>`) keeps its own ecash, locked into HTLCs per payment. Paying moves what is needed there from the cdk wallet and brings the rest back afterwards; the balance shown is both together. Only mints served at the root of their host have a Cassis network.

The page of an order is drawn by [`freya-hypereffect`](../freya-hypereffect) and the protocol is [`freya-cop`](../freya-cop).

## Desktop

```sh
cargo run
```

Data lives in `$PRALINE_HOME`, else `$XDG_DATA_HOME/praline`, else `~/.local/share/praline`. Back up `cassis/config.toml`, or write down the recovery phrase in Settings.

## Android

Requirements: Android SDK with API 36, NDK r26, `cargo-ndk`, and the `aarch64-linux-android` / `x86_64-linux-android` Rust targets.

```sh
just android
```

Android support in Freya is still experimental. Soft keyboard input is limited, which is why amounts use their own keypad.

## Trying it

Public test mints that pay their own invoices, like `https://testnut.cashudevkit.org`, make deposits instant. For orders, run a merchant from the Cassis repo and scan its `cop:` address (or paste it):

```sh
cassis-wallet enable cashu::testnut.cashudevkit.org
cassis-wallet order-demo
```
