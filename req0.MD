将体素地图纳入你的架构，核心在于**严格贯彻“数据（`world`）与表现（`voxel_render`）分离”**。地图的体素数据、区块管理、地形生成属于纯数据逻辑，应放在独立的 `world` 模块；而将体素数据转化为可渲染网格、材质、光照等视觉表现，则交给 `voxel_render` 模块。

### 📁 完整的项目目录结构

以下是整合了体素地图系统后的推荐目录结构。新增的核心部分是 `world`（纯数据）和 `voxel_render`（渲染表现）。

```
src/
├── main.rs                         # 插件组装，保持简洁
│
├── combat/                         # 战斗领域
│   ├── mod.rs
│   ├── plugin.rs                   # CombatPlugin
│   ├── health/
│   │   ├── components.rs           # Health, MaxHealth
│   │   ├── events.rs               # ModifyHealthEvent
│   │   └── systems.rs              # apply_health_change
│   ├── formula/
│   │   ├── types.rs                # DamageType
│   │   ├── events.rs               # DamageRequest
│   │   └── systems.rs
│   ├── attributes/
│   │   └── components.rs           # Armor, DodgeRate
│   ├── buff/
│   │   └── systems.rs              # Regeneration
│   └── skills/
│       └── systems.rs              # cast_heal_skill, cast_fireball
│
├── movement/                       # 移动领域
│   ├── mod.rs
│   ├── components.rs               # Position, Velocity, Speed
│   └── plugin.rs                   # MovementPlugin
│
├── world/                          # ✅ 新增：体素地图数据领域
│   ├── mod.rs
│   ├── plugin.rs                   # WorldPlugin
│   │
│   ├── voxel/                      # 体素基础定义
│   │   ├── components.rs           # Voxel (方块数据)
│   │   └── types.rs                # VoxelType (Air, Stone, Dirt...)
│   │
│   ├── chunk/                      # 区块系统
│   │   ├── components.rs           # Chunk, ChunkPos, ChunkLoader
│   │   ├── events.rs               # ChunkLoadEvent, ChunkUnloadEvent
│   │   └── systems.rs              # chunk_streaming_system (流式加载/卸载)
│   │
│   ├── terrain/                    # 地形生成
│   │   ├── resources.rs            # TerrainConfig (噪声种子、参数)
│   │   └── systems.rs              # generate_terrain_for_chunk
│   │
│   └── storage/                    # 体素数据存储
│       ├── resources.rs            # ChunkMap (区块查询 Resource)
│       └── systems.rs              # set_voxel, get_voxel
│
├── scense/                         # 展示领域（UI/特效/角色表现）
│   ├── mod.rs
│   ├── plugin.rs                   # ScensePlugin
│   ├── components.rs               # HealthBar, FloatingText
│   └── systems.rs
│
└── voxel_render/                   # ✅ 新增：体素渲染表现领域
    ├── mod.rs
    ├── plugin.rs                   # VoxelRenderPlugin
    │
    ├── meshing/                    # 网格生成
    │   ├── resources.rs            # MeshingConfig
    │   ├── systems.rs              # mesh_chunk (贪婪网格化)
    │   └── utils.rs                # 网格数据构建工具
    │
    ├── materials/                  # 材质与纹理
    │   ├── resources.rs            # VoxelMaterialRegistry
    │   └── assets.rs               # 纹理图集加载
    │
    └── lighting/                   # 光照（可选）
        └── systems.rs              # 环境光遮蔽(AO)计算
```

---

### 🧩 各模块详细说明

#### 1. `world` —— 体素地图数据领域（纯数据，无渲染依赖）

这是整个体素地图的“大脑”，只关心**数据是什么**，不关心**怎么画**。它不依赖 `bevy_sprite`、`bevy_pbr` 等任何渲染库。

**`world/voxel/types.rs`**：定义方块类型枚举。这是体素世界最基础的数据定义。
```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum VoxelType {
    Air,
    Stone,
    Dirt,
    Grass,
    Water,
    Wood,
    Leaves,
}
```

**`world/chunk/components.rs`**：定义区块组件。每个区块是一个实体，存储其内部的所有体素数据。常见的区块尺寸是 `32³`。
```rust
pub const CHUNK_SIZE: usize = 32; // 32x32x32

#[derive(Component)]
pub struct Chunk {
    pub voxels: Box<[VoxelType; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]>,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkPos(pub IVec3); // 区块在区块坐标系中的位置

/// 挂载在需要加载周围区块的实体上（如玩家/相机）
#[derive(Component)]
pub struct ChunkLoader {
    pub radius: IVec3,
}
```
`ChunkLoader` 组件可以驱动一个“区块流式加载”系统，自动在加载器周围生成/销毁区块，实现无限世界。对于已修改的区块，可以使用 `ChunkPinned` 标记来防止被自动卸载。

**`world/storage/resources.rs`**：`ChunkMap` 资源，用于从 `ChunkPos` 快速查找对应的区块实体（O(1) 查询）。
```rust
#[derive(Resource, Default)]
pub struct ChunkMap {
    pub map: HashMap<ChunkPos, Entity>,
}
```

**`world/terrain/systems.rs`**：地形生成系统。当新区块被加载时，根据噪声函数（如 `Perlin` 或 `Simplex`）生成初始地形数据。**注意**：地形生成是纯数据计算，不涉及网格或渲染。使用全局种子确保区块间无缝衔接。

**`world/storage/systems.rs`**：提供 `set_voxel` / `get_voxel` 接口。`set_voxel` 修改体素数据后，会发出一个 `ChunkDirtyEvent` 事件，通知渲染模块需要重新生成该区块的网格。

---

#### 2. `voxel_render` —— 体素渲染表现领域

这是“画家”，负责把 `world` 模块的体素数据**画出来**。它依赖 `bevy_pbr`、`bevy_render` 等渲染库。

**`voxel_render/meshing/systems.rs`**：**贪婪网格化（Greedy Meshing）** 系统。这是体素渲染的核心优化技术。它不把每个方块渲染成独立的立方体，而是把相邻的、暴露在外的同类型方块面合并成一个大的矩形面，大幅减少三角形数量。

推荐使用成熟的 crate：
- **`binary-greedy-meshing`**：性能极佳，比 `block-mesh-rs` 快约 30 倍，约 3ms 就能网格化一个区块。
- **`block-mesh-rs`**：功能更全面，支持透明方块等特性。

网格化是 CPU 密集型任务，**必须放在异步任务池中执行**，否则会阻塞主线程导致卡顿。使用 Bevy 的 `AsyncComputeTaskPool` 来派发任务。

**`voxel_render/meshing/systems.rs`** 的核心逻辑示意：
```rust
/// 监听 ChunkDirtyEvent，对脏区块进行异步网格化
fn schedule_meshing(
    mut commands: Commands,
    mut dirty_events: EventReader<ChunkDirtyEvent>,
    chunk_query: Query<(&Chunk, &ChunkPos)>,
    meshing_config: Res<MeshingConfig>,
) {
    for event in dirty_events.read() {
        if let Ok((chunk, pos)) = chunk_query.get(event.chunk_entity) {
            // 克隆区块数据，发送到异步任务池
            let voxels = chunk.voxels.clone();
            let config = meshing_config.clone();
            let entity = event.chunk_entity;
            
            AsyncComputeTaskPool::get().spawn(async move {
                let mesh = generate_mesh(&voxels, &config);
                (entity, mesh)
            }).detach();
        }
    }
}

/// 接收网格化结果，应用到区块实体
fn apply_meshing_result(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut ChunkMesh)>,
    // ... 处理异步任务完成的结果
) {
    // ...
}
```

**`voxel_render/materials/`**：材质与纹理图集管理。体素游戏通常使用**纹理图集（Texture Atlas）** 来减少 draw call。每个方块类型的每个面（顶、侧、底）对应图集中的一个 tile。材质注册表将 `VoxelType` 映射到图集索引。

**`voxel_render/lighting/`**（可选）：体素环境光遮蔽（AO）。通过计算每个顶点周围被遮挡的体素数量，可以得到柔和的阴影效果，大幅提升视觉质量。

---

#### 3. 与现有模块的协作关系

**`world` 与 `voxel_render` 通过事件解耦**：

```
world/chunk/systems.rs              world/storage/systems.rs
  chunk_streaming_system              set_voxel / get_voxel
       │                                    │
       │ 发出 ChunkLoadEvent               │ 修改体素后发出 ChunkDirtyEvent
       ▼                                    ▼
voxel_render/meshing/systems.rs
  schedule_meshing → AsyncComputeTaskPool → apply_meshing_result
       │
       ▼
  生成 Mesh 并挂载到区块实体
```

**`world` 与 `combat` / `movement` 的交互**：
- **移动碰撞**：`movement` 模块的碰撞系统查询 `world` 的 `ChunkMap`，判断目标位置是否为固体方块。
- **战斗破坏地形**：`combat` 的爆炸或技能系统发出 `TerrainModifyEvent`，`world` 模块监听并调用 `set_voxel` 将方块设为 `Air`，同时触发 `ChunkDirtyEvent` 重新网格化。
- **方块交互**：玩家放置/破坏方块，同样通过 `world/storage/systems.rs` 的 `set_voxel` 接口。

**与 `scense` 的关系**：`scense` 负责 UI、角色、特效等**非体素**的展示；`voxel_render` 专门负责**体素地图**的展示。两者在 `main.rs` 中作为独立的 Plugin 注册，互不干扰。

---

### 💎 核心设计原则总结

| 原则 | 在体素地图中的体现 |
| :--- | :--- |
| **数据与表现分离** | `world` 只存 `VoxelType`，`voxel_render` 负责网格化和材质 |
| **事件驱动** | `ChunkLoadEvent`、`ChunkDirtyEvent` 驱动渲染更新 |
| **异步计算** | 网格化在 `AsyncComputeTaskPool` 中执行，不阻塞主线程 |
| **区块化** | 固定尺寸（如 32³）的区块是数据存储和渲染的基本单位 |
| **按需加载** | `ChunkLoader` 组件驱动流式加载，只加载玩家周围的区块 |
| **纯数据可测试** | `world` 模块可以脱离渲染环境进行单元测试 |

---

### 📚 推荐参考的 Crate

| Crate | 用途 | 说明 |
| :--- | :--- | :--- |
| `chunky-bevy` | 区块管理 | 提供 `ChunkManager`、`ChunkLoader` 等，可直接集成 |
| `binary-greedy-meshing` | 贪婪网格化 | 高性能网格生成，支持透明方块 |
| `bevy_voxel_world` | 完整方案参考 | 展示了双层世界（程序化 + 持久化）的设计模式 |
| `bevy_vox_scene` | 体素模型导入 | 加载 Magica Voxel `.vox` 文件，适合预制建筑 |

**对你当前的实操建议**：先不要急着引入完整的第三方体素引擎。按照上面的结构，从 `world` 模块开始，用最简单的数据结构（`HashMap<ChunkPos, Chunk>`）和最简单的网格生成（每个方块一个立方体）验证流程。跑通“加载区块 → 生成数据 → 生成网格 → 渲染”这条链路后，再逐步引入贪婪网格化和异步任务池进行优化。
