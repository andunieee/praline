//! Android QR scanning.
//!
//! The scanner is the embedded ZXing `CaptureActivity`. Kotlin reports the
//! decoded text through `nativeOnQrScanned`, which is forwarded to the Freya
//! renderer thread and handed to the UI as a `State<Option<String>>`.

use std::{
    cell::Cell,
    sync::Mutex,
};

use freya::prelude::{
    GlobalContexts,
    State,
    WritableUtils,
    use_state,
};
use jni::{
    EnvUnowned,
    JavaVM,
    jni_sig,
    jni_str,
    objects::{
        JObject,
        JString,
    },
};
use winit::platform::android::activity::AndroidApp;

use crate::android::post_to_ui;

static PENDING: Mutex<Option<String>> = Mutex::new(None);

thread_local! {
    static STATE: Cell<Option<State<Option<String>>>> = const { Cell::new(None) };
}

/// Returns the state that receives scanned QR contents, if any.
pub fn use_scan_result() -> State<Option<String>> {
    let state = use_state(|| PENDING.lock().unwrap().take());
    STATE.with(|cell| cell.set(Some(state)));
    state
}

/// Launches the embedded scanner activity.
pub fn start() {
    let Some(app) = GlobalContexts::get().try_get_context::<AndroidApp>() else {
        return;
    };
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) };

    let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr() as *mut _) };
        env.call_method(&activity, jni_str!("startScanner"), jni_sig!("()V"), &[])?;
        Ok(())
    });
}

fn deliver(contents: String) {
    *PENDING.lock().unwrap() = Some(contents.clone());

    post_to_ui(move |_| {
        STATE.with(|cell| {
            if let Some(mut state) = cell.get() {
                state.set(Some(contents));
            }
        });
    });
}

/// Called by `MainActivity` with the contents of a scanned QR code.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_caskade_wallet_MainActivity_nativeOnQrScanned(
    mut unowned_env: EnvUnowned<'_>,
    _this: JObject<'_>,
    contents: JString<'_>,
) {
    let outcome = unowned_env.with_env(|env| -> jni::errors::Result<()> {
        deliver(contents.try_to_string(env)?);
        Ok(())
    });
    outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}
