//! Window insets reported by the platform.
//!
//! On Android the window is edge-to-edge (enforced for `targetSdk 35+`), so the
//! UI itself has to keep its content clear of the system bars. `MainActivity`
//! reports the current insets through JNI and they are applied as padding on
//! the root element.

/// Insets, in logical pixels, reserved for system bars and display cutouts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[cfg(not(target_os = "android"))]
pub fn use_insets() -> Insets {
    Insets::default()
}

#[cfg(target_os = "android")]
mod platform {
    use std::{
        cell::Cell,
        ffi::c_void,
        sync::atomic::{
            AtomicU32,
            Ordering,
        },
    };

    use freya::prelude::{
        State,
        WritableUtils,
        use_state,
    };

    use super::Insets;

    static LEFT: AtomicU32 = AtomicU32::new(0);
    static TOP: AtomicU32 = AtomicU32::new(0);
    static RIGHT: AtomicU32 = AtomicU32::new(0);
    static BOTTOM: AtomicU32 = AtomicU32::new(0);

    thread_local! {
        static STATE: Cell<Option<State<Insets>>> = const { Cell::new(None) };
    }

    pub fn use_insets() -> Insets {
        let state = use_state(Insets::current);
        STATE.with(|cell| cell.set(Some(state)));
        *state.read()
    }

    impl Insets {
        fn current() -> Self {
            Self {
                left: f32::from_bits(LEFT.load(Ordering::Relaxed)),
                top: f32::from_bits(TOP.load(Ordering::Relaxed)),
                right: f32::from_bits(RIGHT.load(Ordering::Relaxed)),
                bottom: f32::from_bits(BOTTOM.load(Ordering::Relaxed)),
            }
        }
    }

    fn store(insets: Insets) {
        LEFT.store(insets.left.to_bits(), Ordering::Relaxed);
        TOP.store(insets.top.to_bits(), Ordering::Relaxed);
        RIGHT.store(insets.right.to_bits(), Ordering::Relaxed);
        BOTTOM.store(insets.bottom.to_bits(), Ordering::Relaxed);
    }

    /// Called from `MainActivity` on the Java main thread whenever the window
    /// insets change. The values are in logical pixels already.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_caskade_wallet_MainActivity_nativeSetInsets(
        _env: *mut c_void,
        _this: *mut c_void,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) {
        let insets = Insets {
            left,
            top,
            right,
            bottom,
        };
        store(insets);

        // Freya state is single threaded, so hop onto the renderer thread
        // before applying the new insets.
        crate::android::post_to_ui(move |_| {
            STATE.with(|cell| {
                if let Some(mut state) = cell.get() {
                    state.set(insets);
                }
            });
        });
    }
}

#[cfg(target_os = "android")]
pub use platform::use_insets;
