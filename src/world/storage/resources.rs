//! 区块索引资源：区块坐标 → 区块实体（O(1) 查询）。

use std::collections::HashMap;

use bevy::prelude::*;

use crate::world::chunk::ChunkPos;

/// 已加载区块的实体索引。
///
/// 体素坐标（世界空间）→ 区块实体 → 区块内局部下标，是数据域唯一的寻址路径。
#[derive(Resource, Debug, Default)]
pub struct ChunkMap {
    map: HashMap<ChunkPos, Entity>,
}

impl ChunkMap {
    /// 查区块实体。
    pub fn get(&self, pos: ChunkPos) -> Option<Entity> {
        self.map.get(&pos).copied()
    }

    /// 区块是否已加载。
    pub fn contains(&self, pos: ChunkPos) -> bool {
        self.map.contains_key(&pos)
    }

    /// 登记区块实体（覆盖同坐标的旧记录）。
    pub fn insert(&mut self, pos: ChunkPos, chunk: Entity) -> Option<Entity> {
        self.map.insert(pos, chunk)
    }

    /// 移除登记。
    pub fn remove(&mut self, pos: ChunkPos) -> Option<Entity> {
        self.map.remove(&pos)
    }

    /// 遍历已加载区块。
    pub fn iter(&self) -> impl Iterator<Item = (ChunkPos, Entity)> + '_ {
        self.map.iter().map(|(pos, entity)| (*pos, *entity))
    }

    /// 已加载区块数。
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// 是否没有加载任何区块。
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
