use crate::index::embedder::{Embedder, EmbeddingService};

pub trait EmbedderFactory: Send + Sync {
    fn create(&self, model: &str) -> anyhow::Result<Box<dyn EmbeddingService>>;
}

pub fn create_embedder_factory() -> impl EmbedderFactory {
    EmbedderFactoryImpl
}

struct EmbedderFactoryImpl;

impl EmbedderFactory for EmbedderFactoryImpl {
    fn create(&self, model: &str) -> anyhow::Result<Box<dyn EmbeddingService>> {
        Ok(Box::new(Embedder::new(model)?))
    }
}
