use crate::support::ui::{Console, create_console};

pub(crate) struct PreparedServe {
    pub(crate) router: axum::Router,
}

impl std::fmt::Debug for PreparedServe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedServe")
            .field("router", &"axum::Router { ... }")
            .finish()
    }
}

pub(crate) async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        let ui = create_console(false);
        Console::info(&ui, &format!("Shutdown signal error: {}", e));
    } else {
        let ui = create_console(false);
        Console::info(&ui, "Shutting down...");
    }
}
