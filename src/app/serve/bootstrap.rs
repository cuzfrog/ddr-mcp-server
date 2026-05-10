use crate::support::ui::{Console, create_console};

pub(crate) async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        let ui = create_console(false);
        Console::info(&ui, &format!("Shutdown signal error: {}", e));
    } else {
        let ui = create_console(false);
        Console::info(&ui, "Shutting down...");
    }
}
