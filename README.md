# Caskade

A Bitcoin wallet UI for desktop and Android, built with [Freya](https://github.com/marc2332/freya).

This is UI only. Every number, transaction, UTXO and invoice comes from `src/mock.rs`, and nothing touches the network or any keys.

## Layout

```
src/
  main.rs          desktop entry point
  lib.rs           android_main + crate root
  app.rs           theme, routes, shell with bottom navigation
  state.rs         Settings (currency, unit, network, server...) shared via context
  mock.rs          fake data, invoice generator/"decoder", formatting
  ui.rs            shared helpers (headers, sections, rows, pills, icons)
  screens/
    home.rs        balance card + transaction history
    send.rs        QR scanner placeholder, paste invoice, manual entry
    pay.rs         payment confirmation -> sent
    receive.rs     amount keypad -> random invoice + QR code
    internals.rs   node info, UTXOs, channels, fee estimates, sync log
    settings.rs    currency, unit, theme, network, servers, Tor, security
android/           Gradle project that wraps the Rust cdylib
```

## Desktop

```sh
cargo run
```

## Android

Requirements: Android SDK with API 36, NDK r26, `cargo-ndk`, and the `aarch64-linux-android` / `x86_64-linux-android` Rust targets.

```sh
export ANDROID_HOME=/path/to/android-sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/26.3.11579264
cd android
./gradlew installDebug && adb shell am start -n dev.caskade.wallet/.MainActivity
```

Gradle runs `cargo ndk ... build --lib --release` from the repo root before packaging.

Android support in Freya is still experimental. Soft keyboard input is limited, which is why Receive uses its own numeric keypad.
