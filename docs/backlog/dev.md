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
