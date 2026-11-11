//! Bridge from Kotlin callbacks to the Freya renderer thread.

use std::sync::OnceLock;

use freya::prelude::{
    LaunchProxy,
    RendererContext,
};
use freya_winit::renderer::NativeEvent;
use winit::event_loop::EventLoopProxy;

static PROXY: OnceLock<EventLoopProxy<NativeEvent>> = OnceLock::new();

pub fn install_proxy(proxy: EventLoopProxy<NativeEvent>) {
    let _ = PROXY.set(proxy);
}

/// Runs `callback` on the renderer thread, where Freya state can be mutated.
pub fn post_to_ui(callback: impl FnOnce(&mut RendererContext) + 'static) {
    if let Some(proxy) = PROXY.get() {
        let _ = LaunchProxy(proxy.clone()).post_callback(callback);
    }
}
