//! 方块材质注册表、基色表与**程序生成的方块贴图**。

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::world::VoxelType;

/// 方块基色（纯数据，便于单测；将来换成纹理图集索引时这里改成 UV 表）。
///
/// 返回 `None` 表示该类型不产生几何（空气）。
pub fn voxel_color(voxel: VoxelType) -> Option<Color> {
    match voxel {
        VoxelType::Air => None,
        VoxelType::Grass => Some(Color::srgb(0.30, 0.58, 0.24)),
        VoxelType::Dirt => Some(Color::srgb(0.42, 0.31, 0.20)),
        VoxelType::Stone => Some(Color::srgb(0.48, 0.48, 0.50)),
        VoxelType::Water => Some(Color::srgba(0.16, 0.38, 0.62, 0.72)),
        VoxelType::Wood => Some(Color::srgb(0.45, 0.32, 0.19)),
        VoxelType::Leaves => Some(Color::srgb(0.22, 0.45, 0.20)),
    }
}

/// 方块贴图的边长（像素）。
///
/// 小图就够：网格给每个方块铺**一格**纹理（贪婪合并出来的面按矩形尺寸重复，
/// 见 `meshing::utils::MeshBuilder::push_quad`），所以分辨率只决定"颗粒多细"。
pub const VOXEL_TEXTURE_SIZE: u32 = 16;

/// 生成一种方块的**可平铺**贴图（像素噪声，围绕基色起伏）。
///
/// **为什么程序生成**：`textures/units/shadow.png` 与技能图标本来就是这个路子
/// （见 `assets/LICENSES.md`），于是不必引新素材、也不必担心许可——
/// 而方块贴图要的只是"有点颗粒、看不出是一块纯色"。
///
/// **可平铺**是关键：UV 会超出 `0..1`（合并出来的面按格数重复），采样器因此设
/// [`ImageAddressMode::Repeat`]。噪声在这里是**逐像素哈希**，与坐标无关地分散，
/// 所以左右接缝本就不明显；哪怕看得出接缝，那也只是"多了一种颗粒"。
pub fn voxel_texture(voxel: VoxelType) -> Option<Image> {
    let base = voxel_color(voxel)?;
    let base = base.to_srgba();
    let size = VOXEL_TEXTURE_SIZE;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            // 逐像素哈希 → 0..1 的噪声；围绕基色做 ±8% 的明暗起伏
            let mut hash = x.wrapping_mul(0x9E37_79B9)
                ^ y.wrapping_mul(0x85EB_CA6B)
                ^ (voxel.index() as u32).wrapping_mul(0xC2B2_AE35);
            hash ^= hash >> 15;
            hash = hash.wrapping_mul(0x2545_F491);
            hash ^= hash >> 13;
            let noise = (hash & 0xFF) as f32 / 255.0;
            let factor = 0.92 + noise * 0.16;
            let channel = |value: f32| (value * factor).clamp(0.0, 1.0);
            data.extend_from_slice(&[
                (channel(base.red) * 255.0) as u8,
                (channel(base.green) * 255.0) as u8,
                (channel(base.blue) * 255.0) as u8,
                (base.alpha * 255.0) as u8,
            ]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    // **重复采样**：UV 会超出 0..1（合并出来的面按矩形尺寸铺开）。
    // 默认是 `ClampToEdge`——那样超出部分会拉成边缘那一条颜色，整片地面糊掉。
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::nearest()
    });
    Some(image)
}

/// 方块类型 → 材质句柄。
///
/// 一个区块按类型拆成多个网格实体，各自用这里的材质——
/// 这样同类方块共享一个材质，不同类之间也不需要多材质网格。
#[derive(Resource, Debug, Default)]
pub struct VoxelMaterialRegistry {
    materials: HashMap<VoxelType, Handle<StandardMaterial>>,
}

impl VoxelMaterialRegistry {
    /// 登记一种方块的材质。
    pub fn insert(&mut self, voxel: VoxelType, material: Handle<StandardMaterial>) {
        self.materials.insert(voxel, material);
    }

    /// 取材质；未登记的方块（如空气）返回 `None`，渲染层直接跳过。
    pub fn get(&self, voxel: VoxelType) -> Option<&Handle<StandardMaterial>> {
        self.materials.get(&voxel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_visible_voxel_type_has_a_colour() {
        for voxel in VoxelType::ALL.into_iter().filter(|v| v.is_visible()) {
            assert!(voxel_color(voxel).is_some(), "{} 缺少基色", voxel.name());
        }
        assert!(voxel_color(VoxelType::Air).is_none(), "空气不产生几何");
    }

    /// 每种可见方块都有一张**尺寸正确、带 alpha** 的贴图；空气没有。
    #[test]
    fn every_visible_voxel_type_has_a_texture() {
        for voxel in VoxelType::ALL.into_iter().filter(|v| v.is_visible()) {
            let image = voxel_texture(voxel).unwrap_or_else(|| panic!("{} 缺少贴图", voxel.name()));
            assert_eq!(
                (image.width(), image.height()),
                (VOXEL_TEXTURE_SIZE, VOXEL_TEXTURE_SIZE),
                "{} 的贴图尺寸应当是 {VOXEL_TEXTURE_SIZE} 方图",
                voxel.name()
            );
            let data = image.data.as_ref().expect("贴图应当带像素数据");
            assert_eq!(
                data.len(),
                (VOXEL_TEXTURE_SIZE * VOXEL_TEXTURE_SIZE * 4) as usize,
                "{} 的贴图应当是 RGBA（每像素 4 字节）",
                voxel.name()
            );
        }
        assert!(voxel_texture(VoxelType::Air).is_none(), "空气不产生几何");
    }

    /// **采样必须是 `Repeat`**：网格的 UV 会超出 `0..1`（贪婪合并出来的面按矩形
    /// 尺寸铺开），默认的 `ClampToEdge` 会把超出部分拉成边缘那一条颜色——整片地面糊成一条。
    #[test]
    fn the_texture_tiles_instead_of_clamping() {
        let image = voxel_texture(VoxelType::Stone).unwrap();
        let ImageSampler::Descriptor(descriptor) = &image.sampler else {
            panic!("应当显式设了采样描述符，而不是用全局默认");
        };
        assert_eq!(
            descriptor.address_mode_u,
            ImageAddressMode::Repeat,
            "U 方向必须重复，否则合并出来的面会把贴图拉糊"
        );
        assert_eq!(descriptor.address_mode_v, ImageAddressMode::Repeat);
    }

    /// 贴图**不是纯色**：否则"加贴图"这件事白做（顶点色已经在管明暗了）。
    /// 判据是像素值不止一种，且都落在基色附近（不是乱涂）。
    #[test]
    fn the_texture_has_grain_around_its_base_colour() {
        let image = voxel_texture(VoxelType::Grass).unwrap();
        let data = image.data.as_ref().unwrap();
        let reds: Vec<u8> = data.chunks(4).map(|px| px[0]).collect();
        let distinct: std::collections::BTreeSet<u8> = reds.iter().copied().collect();
        assert!(
            distinct.len() > 1,
            "贴图应当有颗粒（不止一个亮度），实际只有 {distinct:?}"
        );

        // 颗粒是围绕基色的**小幅**起伏：极差应当远小于整个色域
        let min = *reds.iter().min().unwrap();
        let max = *reds.iter().max().unwrap();
        assert!(
            max - min < 64,
            "起伏应当是'颗粒'而不是噪点：极差 {} 太大",
            max - min
        );
    }

    /// 不同方块的贴图**不一样**（哈希里掺了类型下标）——否则所有方块会用同一张噪声图。
    #[test]
    fn different_voxel_types_get_different_textures() {
        let grass = voxel_texture(VoxelType::Grass).unwrap();
        let stone = voxel_texture(VoxelType::Stone).unwrap();
        assert_ne!(grass.data, stone.data, "两种方块的贴图不该逐像素相同");
    }
}
