use crate::index::embedder::list_supported_models;
use crate::support::ui::Console;

pub fn list_models(console: &dyn Console) {
    for (name, dim) in list_supported_models() {
        console.info(&format!("{} (dim: {})", name, dim));
    }
}
