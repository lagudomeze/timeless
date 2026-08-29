//! # timeless-domain：纯 Rust 领域层（零引擎依赖）
//!
//! 约束：本 crate 不得依赖任何引擎/框架 crate（尤其 `bevy`），
//! 保证战斗裁决可被 `cargo test` 独立覆盖、可被任何宿主复用。

pub mod combat;
pub mod grid;
