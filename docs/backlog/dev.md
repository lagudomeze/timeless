# backlog · dev（跨表现与战斗的条目）

> 活的总览：[`../../TODO.md`](../../TODO.md) · 历史证据：[`../../CHANGELOG.md`](../../CHANGELOG.md)
> 这里放**同时被两块屏幕 / 两个领域读**的条目——它们的难点不在实现，在"谁拥有这份数据"。

- [x] **#57 威胁格可视化的分层决定**（已落地，留档）
      数据 `Threatens.cells` 现在**同时被三处读**：反应系统（决定要不要冻世界）、
      AI（决定要不要躲）、表现层（画在地上）。这正是 `docs/game-design.md`
      「AI 与玩家对称」那条的兑现。
      **分层结论**：表现层**只读**，自己判断"还有没有威胁"会与 `detect_threat_system`
      的关窗判据分叉——所以「关窗即隐藏」由 `ReactionSlot` 的存在性回答，
      表现层不复制一份判据。**新增威胁视觉时沿用这条。**

- [ ] **[P2][feat] 命中 / 施法反馈的"表达层归属"**（原 issue #60 的结构部分）
      **现状**：命中粒子在 `presentation/effects.rs`，它**只读** `DamageEvent`，
      不改任何游戏状态、不影响结算，所以 `combat` 不需要知道"命中要好看"。
      威胁格在 `presentation/threat_grid.rs`，同样只读。
      **要守住的边界（写下来，免得将来破掉）**：
      ① 反馈层**只读**领域事件（`DamageEvent` / `DeathEvent` / `ProjectileArrived`），
      **不许**写回游戏状态；
      ② 反馈**不许**成为结算的输入（伤害数字不能参与命中判定）；
      ③ 需要"逻辑冻结但视觉还在动"时，先查 [`clock.md`](clock.md) 那张表，
      **不许**自己造第二个时钟。
      **触发条件**：等伤害数字（[`presentation.md`](presentation.md)）落地时一起核对这条边界。

- [x] **[P2][dev] 逻辑域里造网格 / 材质的收口**（已修）
      **现象（当时可量化）**：渲染类型出现在**非渲染域**里——
      `combat/attack/arrow.rs` / `fireball.rs` / `melee.rs` 各 2 处 `asset_value(...)` +
      4 处 `Mesh3d` / `StandardMaterial`；`interaction/visual.rs` 是单文件最脏的一个
      （20 处渲染类型、6 处 `asset_value`：高亮环 / AOE 圆盘 / 近战扇形）。
      连带后果：**逻辑域的插件里出现 `init_asset::<Mesh>()` / `ScenePlugin`**
      （`src/lib.rs`、`equipment/systems.rs`、`timeline/mod.rs`）——它们是"逻辑域还要造几何"
      逼出来的，测试里不得不把资产管线整个装起来。
      **改法（已落地，两个新文件）**：
      ① [`combat/attack/scene.rs`](../../src/combat/attack/scene.rs)：`arrow_scene` /
      `melee_scene` / `fireball_scene` 三个投射物工厂搬过来；
      ② [`interaction/scene.rs`](../../src/interaction/scene.rs)：`spawn_hover_highlight` /
      `spawn_preview_indicators` 两个开局工厂 + 全部颜色与几何常量搬过来
      （`visual.rs` 只剩"每帧刷状态"，通过 `use super::scene::{…}` 取常量）。
      **边界（这次实测出来的，比原条目更准）**：判断一个工厂该不该搬，看它**有没有渲染类型**。
      `shoot_action_scene` / `melee_action_scene`（`attack/actions.rs`）只挂载荷与节奏、
      **没有任何 `Mesh3d`**，所以**留在原地**——搬过去只会多一次跳转。
      同理 `visual.rs` 仍会碰到 `MeshMaterial3d<StandardMaterial>`：它要改的是
      **一个具体网格实例的颜色**（`ResMut<Assets<StandardMaterial>>` 改材质），那是它的本职，
      而"这个方块长什么样"已经搬走了。
      **量化验收**（`grep`）：
      - `combat` 只剩 `scene.rs` 一个文件含渲染类型；`movement` / `timeline` / `ai` / `input`
        **归零**；`interaction` 只剩 `scene.rs` + `visual.rs` 的**测试夹具**（造材质给高亮用）。
      - **四个逻辑域的 `plugin.rs` 里 `init_asset` / `ScenePlugin` 全部归零**——
        这正是这条收口真正的收益：逻辑域的单测不再需要装资产管线。
      - `cargo test` 395 通过 / clippy 零警告（纯搬家，行为一行未改）。
      ⚠️ **搬迁过程本身出过一次事故**：我用 `Get-Content | Set-Content` 同一个文件砍行时，
      输入流把文件锁住导致写入失败，第二次尝试又因为 `$body` 取空而**把文件覆盖成只剩文件头**
      （`interaction/visual.rs`）。教训：**大段删改不要用 shell 拼接**——
      要么用编辑工具，要么先写临时文件再整体替换，并且**动完立刻 `cargo check`**。
      这次是靠 `git checkout --` 恢复 HEAD 再重做才没丢东西。

## 还没做（本文件里剩下的）

- [ ] **[P2][bug] `Faction` ≠「单位」：攻击实体也带阵营，松查询会把它们当成单位**
      **怎么发现的**：实机第 5 节发现"三个单位全在忙，候场区却亮着一个 `E`"，
      追下去是时间轴的 `actors: Query<(Entity, &Faction, Option<&DecisionSlot>)>`
      把**在飞的箭**当成了"没有决策槽的单位"（= 已就绪）→ 占一条车道 + 站候场区。
      **时间轴那处已修**（加 `With<Health>`，见 `timeline/system.rs` 的注释与
      `attack_entities_are_not_units` 测试）。**但根因是全局性的**：
      `combat::attack::scene` 的三个场景工厂（箭矢 / 横扫 / 火球）都给**攻击实体**
      烘了 `Faction`（供命中过滤"不打自己人"），于是**任何**"按 `Faction` 枚举单位"
      的查询都会把它们算进去。
      **已审计到的松查询**（`grep 'Query<.*Faction'`，2026-09-27）：
      | 位置 | 用途 | 把攻击实体算进去的后果 |
      | :--- | :--- | :--- |
      | `presentation/camera.rs` | 镜头跟随"玩家" | ~~待确认~~ **已修**（2026-09-27）：旧代码 `find(faction == Player)` 会选中玩家的箭 → **镜头飞走**。已改为认 `InputDriven`（仓库既有的规矩），并加会咬的测试 `a_player_faction_projectile_is_not_the_player` |
      | `interaction/pointer.rs:74` | 左键**单位**→ 打它 | 点到在飞的箭上会当成"点了某个单位" |
      | `interaction/pointer.rs:149` / `visual.rs:140` | 悬停格的占位者 | 箭所在格被算成"有人占着"（染色 / 可走性读数可能受影响） |
      | `combat/attack/actions.rs:126,251,305`、`fireball.rs:190,278,361` | 找"最近的敌人" | 敌人的瞄准可能选中**玩家的箭**（瞄到一个正在飞走的点） |
      | `combat/defense/actions.rs:137` | 翻滚 / 招架的判定范围 | 同格判定可能被箭影响 |
      | `ai/systems.rs:151` | 敌人找目标 | 同上 |
      | `presentation/hud/timeline/system.rs:294` | 悬停读数的单位表 | 与已修的那处同源 |
      **对照**（已经做对的）：`combat/attack/explosion.rs:46` 用 `With<Health>`、
      `presentation/hud/panels/system.rs` 的 `UnitQuery` 要求 `Health` + `Cell`、
      `ai/systems.rs:75` 带 `&Health`。
      **改法（一次做完，别零敲碎打）**：给"这是个单位"一个**显式标记**——
      要么沿用 `With<Health>`（现成先例最多，改动最小），要么新增一个
      `Unit` 标记组件由 `spawn/unit.rs` 统一挂上（语义最清楚、且将来"能被单位查询"
      变成显式选择）。**倾向于后者**：`Health` 是战斗概念，借它表达"是单位"
      是隐式耦合；而 `Faction` 这个坑正是"借一个碰巧存在的组件表达另一个概念"造成的。
      **触发条件**：**下一次碰相机跟随 / 点击拾取 / 敌人瞄准时**顺手做完——
      单独为它开一轮不划算，但每拖一轮就多一处"屏幕或 AI 在说谎"。
      **验收**：每个改过的查询配一条"攻击实体不算单位"的测试（时间轴那条是范例）；
      外加实机：**射一支箭，看候场区/车道/镜头/悬停占位有没有多出东西**。
