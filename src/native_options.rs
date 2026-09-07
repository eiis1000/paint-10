use eframe::{egui::ViewportBuilder, EventLoopBuilderHook, NativeOptions};

/// Keep native keyboard actions inside the application's close/cancel flow.
pub(crate) fn with_viewport(viewport: ViewportBuilder) -> NativeOptions {
    NativeOptions {
        viewport,
        event_loop_builder: event_loop_builder(),
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn event_loop_builder() -> Option<EventLoopBuilderHook> {
    use winit::platform::macos::EventLoopBuilderExtMacOS;

    Some(Box::new(|builder| {
        // Winit's default Quit menu calls AppKit terminate: directly. It skips
        // Paint's unsaved-change prompt and never returns a helper's result.
        // Our window handles Cmd+Q itself; the format picker treats it as Cancel.
        builder.with_default_menu(false);
    }))
}

#[cfg(not(target_os = "macos"))]
fn event_loop_builder() -> Option<EventLoopBuilderHook> {
    None
}
