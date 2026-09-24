//! 光照：面朝向明暗 + 顶点环境光遮蔽（AO）。两者都烘焙进顶点色。

pub mod systems;

pub use systems::{MIN_AO_SHADE, face_shade, occlusion_level, shade_of_level, vertex_occlusion};
