use anyhow::Context;
use async_trait::async_trait;
use axum::Router;

use crate::support::ui::WorkflowUi;

#[async_trait]
pub trait Server: Send + Sync {
    async fn serve(&self, router: Router, port: u16, ui: &dyn WorkflowUi) -> anyhow::Result<()>;
}

pub struct TokioHttpServer;

#[async_trait]
impl Server for TokioHttpServer {
    async fn serve(&self, router: Router, port: u16, ui: &dyn WorkflowUi) -> anyhow::Result<()> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .context("Failed to bind TCP listener")?;
        let local_addr = listener
            .local_addr()
            .context("Failed to get local address")?;

        ui.info(&format!(
            "docent server listening on http://{} (open in browser for web UI)",
            local_addr,
        ));

        axum::serve(listener, router)
            .with_graceful_shutdown(super::bootstrap::shutdown_signal())
            .await
            .context("Server error")?;

        Ok(())
    }
}
