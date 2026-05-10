//! Server bootstrap — types and helpers for the axum serve lifecycle.

use crate::support::ui::{ConsoleUi, WorkflowUi};

/// Result of preflight that does not require a TCP listener.
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

/// Return a future that resolves when a shutdown signal (Ctrl+C) is received.
pub(crate) async fn shutdown_signal() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        let ui = ConsoleUi;
        WorkflowUi::info(&ui, &format!("Shutdown signal error: {}", e));
    } else {
        let ui = ConsoleUi;
        WorkflowUi::info(&ui, "Shutting down...");
    }
}
