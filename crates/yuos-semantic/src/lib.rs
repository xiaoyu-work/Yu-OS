//! Filesystem semantic-search daemon and CLI orchestration for Yu OS.
//!
//! Reusable embedding, extraction, chunking, walking, and storage contracts
//! are owned by [`yuos_embed`]. Compatibility modules preserve the original
//! `yuos_semantic::{chunk, embed, extract, store, watch}` import paths while
//! callers migrate to `yuos_embed` directly.

pub mod config;
pub mod indexer;

pub use yuos_embed::{
    Chunk, DocumentStoreError, EmbedError, EmbedRequest, EmbedResponse, EmbedUsage, Embedder,
    Extractor, MemoryStore, SearchHit, StoreStats, StubEmbedder, TextExtractor, VectorStore,
    EMBED_DIM,
};
pub use config::Config;

pub mod chunk {
    pub use yuos_embed::chunk::*;
}

pub mod embed {
    pub use yuos_embed::embed::*;
}

pub mod extract {
    pub use yuos_embed::extract::*;
}

pub mod store {
    pub use yuos_embed::document_store::*;
}

pub mod watch {
    pub use yuos_embed::walk::*;
}

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/test/unit/lib.rs"));
}
