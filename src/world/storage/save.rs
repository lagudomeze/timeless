//! 区块持久化：**只存"改动"，不存整块地形**。
//!
//! ## 为什么不是"保存区块"
//!
//! 一个区块是 `32³ = 32768` 个体素。默认地形有 3 层区块在场，
//! 存原始数据就是近百 KB，而其中**绝大部分是噪声函数的确定输出**——
//! 地形是 `seed + 坐标` 的纯函数（[`surface_height`](crate::world::terrain::surface_height)），
//! 同一个种子必然重新算出同一块地。存它等于把"算得出来的东西"抄一遍。
//!
//! 所以这里存的是**改动**：一条 `(世界体素坐标, 类型)`。加载时先按噪声生成地形，
//! 再把改动**盖回去**。于是：
//!
//! - 存档大小 = 玩家真的动过几格（玩一会儿也就几十条），与区块数量无关；
//! - 加载顺序天然正确（先生成、后覆盖），不需要"存的是完整快照还是补丁"这种区分；
//! - 换种子开新世界时，旧存档的改动仍然指向**具体坐标**——语义清楚
//!   （"玩家在世界 (3,-1,4) 放了一块石头"），不像存整块快照那样会带着旧种子的地形。
//!
//! ## 格式与位置
//!
//! 一份 `.ron`（与 `config/actions.ron` 同一个格式、同一套 serde），
//! 存一个**世界**：种子的指纹 + 改动列表。
//!
//! ⚠️ **和 `config/` 不是一回事**：`config/` 是**设计数值**（换手感、可手改），
//! 而这是**玩家存档**（运行时产物、不该进版本库）。所以它默认写到 `saves/`，
//! 并加进 `.gitignore`。发行时 `config/` 可以不带，而存档目录是玩出来的。
//!
//! ## 什么时候存
//!
//! **停下就存**（`F5` 重置 / 退出时由 `save_on_exit_system` 落盘），
//! 而不是每改一格就写一次盘——那样连续建造会打出成百上千次小写入。
//! 加载在 `PreStartup`（比区块流式加载早），改动先读进内存，
//! 区块生成时直接盖上去，因此**不需要**"加载后再改一遍"的第二步。
//!
//! **触发条件**（满足任一条就该升级）：① 需要多存档槽 / 存档界面；
//! ② 需要自动定时存（现在只有退出时存）；③ 地形改动量大到 diff 列表不再便宜
//! （那时改成按区块 RLE 存 patches）。

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::TerrainConfig;
use crate::world::voxel::VoxelType;

/// 存档目录（仓库根，**不在 `assets/` 里**——这不是素材，是运行时产物）。
pub const SAVE_DIR: &str = "saves";
/// 存档文件名（单存档槽）。
pub const SAVE_FILE: &str = "world.ron";

/// 一次地形改动：世界体素坐标 + 改成了什么。
///
/// 存**世界坐标**而不是"区块内局部坐标 + 区块坐标"：改动本来就以世界坐标发生
/// （[`set_voxel`](crate::world::storage::set_voxel) 收的就是它），
/// 读回来时再拆成 `(区块, 局部)` 是加载那一侧的事，存档格式不必暴露这层。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoxelEdit {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// 类型名（`"stone"` / `"air"` …，即 [`VoxelType::name`] 的小写形式）。
    ///
    /// **存名字而不是下标**：`VoxelType` 的下标是数组位置（`Air = 0`…），
    /// 加一个新方块类型就会挪动后面的所有下标——那样旧存档会静默读成**另一种方块**。
    /// 名字变了才会报错，报错比悄悄画错强（与 `config/actions.ron` 同一条取舍）。
    pub voxel: String,
}

impl VoxelEdit {
    /// 从世界坐标 + 类型构造（存名字，见 [`VoxelEdit::voxel`]）。
    pub fn new(world: IVec3, voxel: VoxelType) -> Self {
        Self {
            x: world.x,
            y: world.y,
            z: world.z,
            voxel: voxel.name().to_string(),
        }
    }

    /// 世界体素坐标。
    pub fn position(&self) -> IVec3 {
        IVec3::new(self.x, self.y, self.z)
    }

    /// 类型名 → [`VoxelType`]；名字不认识时返回 `None`（旧存档 / 手改坏了）。
    pub fn kind(&self) -> Option<VoxelType> {
        VoxelType::ALL
            .into_iter()
            .find(|candidate| candidate.name() == self.voxel)
    }
}

/// 一份存档。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldSave {
    /// 生成这片世界用的种子。
    ///
    /// **记录它不是为了加载时校验**（改动按坐标盖回去，换了种子照样生效），
    /// 而是为了**告诉玩家/调试者**"这份存档是哪个世界的"——
    /// 换了种子地形会完全不同，而改动还落在同一批坐标上，那多半不是玩家想要的。
    pub seed: u32,
    /// 改动列表（顺序 = 发生顺序）。
    pub edits: Vec<VoxelEdit>,
}

impl Default for WorldSave {
    fn default() -> Self {
        Self {
            seed: TerrainConfig::default().seed,
            edits: Vec::new(),
        }
    }
}

impl WorldSave {
    /// 存档文件的完整路径。
    pub fn path() -> PathBuf {
        Path::new(SAVE_DIR).join(SAVE_FILE)
    }

    /// 从 `.ron` 文本解析（失败返回带位置的错误）。
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    /// 序列化成 `.ron` 文本。
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .unwrap_or_else(|error| format!("// 序列化失败：{error}"))
    }

    /// 从磁盘读；**文件不存在不是错误**（首次运行），返回 `Ok(None)`。
    pub fn load_from_disk() -> Result<Option<Self>, SaveError> {
        let path = Self::path();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(SaveError::Read(error.to_string())),
        };
        Self::from_ron(&text)
            .map(Some)
            .map_err(|error| SaveError::Parse(error.to_string()))
    }

    /// 写回磁盘（目录不存在就建）。
    ///
    /// 写失败**只报错不 panic**：存档写不进去不该让正在跑的游戏崩掉
    /// （与 `config` 的失败策略同一条：大声说、但别把游戏带走）。
    pub fn write_to_disk(&self) -> Result<(), SaveError> {
        let path = Self::path();
        if let Some(parent) = path.parent()
            && let Err(error) = std::fs::create_dir_all(parent)
        {
            return Err(SaveError::Write(error.to_string()));
        }
        std::fs::write(&path, self.to_ron()).map_err(|error| SaveError::Write(error.to_string()))
    }

    /// 记一条改动；**同一格只记最后一次**（在同一个位置反复放挖不该堆成几十条）。
    pub fn record(&mut self, edit: VoxelEdit) {
        if let Some(existing) = self
            .edits
            .iter_mut()
            .find(|existing| existing.position() == edit.position())
        {
            *existing = edit;
            return;
        }
        self.edits.push(edit);
    }
}

/// 存档读写失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// 文件在，但读不动
    Read(String),
    /// 读到了，但解析不了（含行号）
    Parse(String),
    /// 写不进去
    Write(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(detail) => write!(formatter, "读不动 {SAVE_DIR}/{SAVE_FILE}：{detail}"),
            Self::Parse(detail) => write!(
                formatter,
                "{SAVE_DIR}/{SAVE_FILE} 解析失败（**本次按无存档启动**）：{detail}"
            ),
            Self::Write(detail) => write!(formatter, "写不进 {SAVE_DIR}/{SAVE_FILE}：{detail}"),
        }
    }
}

/// 世界级的存档资源：**只装改动，不装整块地形**（理由见模块文档）。
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct WorldEdits(WorldSave);

impl WorldEdits {
    /// 从已有存档起步。
    pub fn from_save(save: WorldSave) -> Self {
        Self(save)
    }

    /// 记一条改动（同格覆盖）。
    pub fn record(&mut self, edit: VoxelEdit) {
        self.0.record(edit);
    }

    /// 某个世界坐标上被改成什么（没改过返回 `None`）。
    pub fn edit_at(&self, world: IVec3) -> Option<VoxelType> {
        self.0
            .edits
            .iter()
            .find(|edit| edit.position() == world)
            .and_then(VoxelEdit::kind)
    }

    /// 改动条数。
    pub fn len(&self) -> usize {
        self.0.edits.len()
    }

    /// 逐条读改动（加载时按区块筛）。
    pub fn iter(&self) -> impl Iterator<Item = &VoxelEdit> {
        self.0.edits.iter()
    }

    /// 没有任何改动（此时的存档与"没存档"等价）。
    pub fn is_empty(&self) -> bool {
        self.0.edits.is_empty()
    }

    /// 种子（写进存档，标明这是哪个世界）。
    pub fn seed(&self) -> u32 {
        self.0.seed
    }

    /// 转成可落盘的存档快照。
    pub fn to_save(&self) -> WorldSave {
        self.0.clone()
    }
}

/// 启动时（`PreStartup`，早于区块流式加载）把存档读进来。
///
/// **读失败不 panic**：报错并按"无存档"启动——存档坏了不该让游戏起不来
/// （与 `config` 的失败策略一致，见 `docs/config.md`）。
pub fn load_world_edits_system(mut commands: Commands, terrain: Res<TerrainConfig>) {
    match WorldSave::load_from_disk() {
        Ok(Some(save)) => {
            let count = save.edits.len();
            info!("💾 已装载存档：{count} 条改动（种子 {}）", save.seed);
            commands.insert_resource(WorldEdits::from_save(save));
        }
        Ok(None) => {
            info!("💾 没有存档：从新世界开始");
            commands.insert_resource(WorldEdits::default());
        }
        Err(error) => {
            error!("{error}");
            commands.insert_resource(WorldEdits::default());
        }
    }
    // 种子以启动时的配置为准（存档里那份只用来提示"这是哪个世界"）
    let _ = terrain;
}

/// 退出时把改动落盘。
///
/// 挂在 `AppExit` 消息上，而不是每改一格写一次盘：连续建造会打出成百上千次小写入，
/// 而"停下就存"已经足够（玩家关窗口 / `F5` 重置时都走这条路）。
pub fn save_on_exit_system(
    mut exits: MessageReader<AppExit>,
    edits: Res<WorldEdits>,
    terrain: Res<TerrainConfig>,
) {
    if exits.read().next().is_none() {
        return;
    }
    let mut save = edits.to_save();
    save.seed = terrain.seed;
    match save.write_to_disk() {
        Ok(()) => info!("💾 已保存 {} 条地形改动", save.edits.len()),
        Err(error) => error!("{error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_save_survives_a_write_read_round_trip() {
        let mut save = WorldSave {
            seed: 7,
            edits: Vec::new(),
        };
        save.record(VoxelEdit::new(IVec3::new(3, -1, 4), VoxelType::Stone));
        save.record(VoxelEdit::new(IVec3::new(-2, 0, 9), VoxelType::Air));

        let text = save.to_ron();
        let parsed = WorldSave::from_ron(&text).expect("自己写出来的存档必须读得回去");
        assert_eq!(parsed, save);
    }

    /// **同一格只留最后一条**：反复放挖同一格不该把存档堆成几十条。
    #[test]
    fn recording_the_same_voxel_twice_keeps_only_the_last() {
        let mut save = WorldSave::default();
        let at = IVec3::new(1, 2, 3);
        save.record(VoxelEdit::new(at, VoxelType::Stone));
        save.record(VoxelEdit::new(at, VoxelType::Air));

        assert_eq!(save.edits.len(), 1, "同一格只该有一条");
        assert_eq!(
            save.edits[0].kind(),
            Some(VoxelType::Air),
            "留下的是最后一次"
        );
    }

    /// 存的是**类型名**而不是下标：下标会随"加一个新方块"整体挪位，
    /// 那样旧存档会静默读成另一种方块。
    #[test]
    fn the_voxel_is_stored_by_name_not_by_index() {
        let edit = VoxelEdit::new(IVec3::ZERO, VoxelType::Grass);
        assert_eq!(edit.voxel, "grass", "存名字（小写，即 `VoxelType::name`）");
        assert_eq!(edit.kind(), Some(VoxelType::Grass), "读得回来");

        // 名字不认识（旧存档里的方块被删了 / 手改坏了）→ None，不是 panic
        let unknown = VoxelEdit {
            x: 0,
            y: 0,
            z: 0,
            voxel: "unobtainium".to_string(),
        };
        assert_eq!(unknown.kind(), None);
    }

    /// 缺字段的档案用默认值补齐（`#[serde(default)]`）——加字段时旧存档还能读。
    #[test]
    fn a_partial_save_falls_back_to_defaults() {
        let parsed = WorldSave::from_ron("(edits: [])").expect("部分字段应当能解析");
        assert!(parsed.edits.is_empty());
        assert_eq!(parsed.seed, TerrainConfig::default().seed, "缺 seed 用默认");
    }

    /// 语法错要**报错并带位置**，而不是静默给个空存档。
    #[test]
    fn a_broken_save_reports_where_it_broke() {
        let error = WorldSave::from_ron("(edits: [").expect_err("语法错必须报错");
        let shown = error.to_string();
        assert!(
            shown.contains("position") || shown.contains(':'),
            "错误信息该指出位置，实际：{shown}"
        );
    }

    /// `WorldEdits` 是**按坐标查询**的：加载时靠它把噪声地形盖回成玩家改过的样子。
    #[test]
    fn edits_are_queryable_by_world_position() {
        let mut edits = WorldEdits::default();
        let at = IVec3::new(5, -1, 7);
        assert_eq!(edits.edit_at(at), None, "没改过就是 None");

        edits.record(VoxelEdit::new(at, VoxelType::Stone));
        assert_eq!(edits.edit_at(at), Some(VoxelType::Stone));
        assert_eq!(edits.len(), 1);
        assert!(!edits.is_empty());

        // 旁边那一格不受影响
        assert_eq!(edits.edit_at(IVec3::new(6, -1, 7)), None);
    }
}
