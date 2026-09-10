# crates/app 未迁移内容盘点与按新范式重设计

> ⚠️ 2026-09-10 更新：`crates/app` 已移到仓库根目录（package `app`，`src/`）并完成领域化
> 重构，**当前结构见 [app-modules.md](app-modules.md)**；本文的「新 app 状态」列停留在
> 重构前，保留作盘点记录。

> 目标：列出旧 `timeless-app` / `timeless-domain` 尚未进入 `crates/app` 的内容，
> 并按 app 的既定范式（Bevy 0.19 + BSN 场景、小组件 + 专属系统、`Message`
> 流水线、世界空间 `Transform + Velocity`）重新设计，避免 OO 聚合与兼容包袱。

## 一、迁移状态总览

| 旧模块 / 能力 | 旧载体 | 新 app 状态 | 处理 |
| :--- | :--- | :--- | :--- |
| 纯裁决逻辑 | `timeless-domain::combat` | 已随远端起删除 | 网格仲裁方向废弃，由新流水线取代 |
| 世界空间流水线 | — | `events.rs` + `health/` + `combat/` + `despawn/` | ✅ 已落地（箭矢→扣血→死亡） |
| 地面 / 网格 / 相机 | `display/map` 等 | `map.rs` Ground/GroundGrid、`camera.rs` | ✅ 已落地（场景化） |
| `Health` | app `combat.rs` | `health/components.rs` | ✅（f32 化） |
| `Player` / `Enemy` 标记 | app `combat.rs` | 缺失 | 重设计为阵营组件 |
| 单位实体（玩家 / 敌人 / 假人） | `setup::spawn_combatants`、纸片 | 缺失 | 重设计为 `bsn!` 场景工厂 |
| 玩家移动输入 | `menu::decision_keyboard` + grid | 缺失 | 重设计为 键盘 → `MoveCommand` 消息 |
| 敌人 AI | `combat::ai_system`（网格走位/攻击） | 缺失 | 重设计为世界空间追踪 + 攻击调度 |
| 攻击生成 | `Fireball` / 近战裁决 | 只有测试直插实体 | 重设计为攻击场景工厂（箭 / 近战 / AOE） |
| 射弹运动 / 碰撞 | `movement::projectile_motion` | `combat::movement` + targeting | ✅（通用 Velocity） |
| 生命周期 / 清理 | `turn_end` / `death_check` | lifecycle / cleanup / despawn | ✅ |
| 伤害类型 | `Damage` / `Impact` 等 | `PhysicalDamage` + 护甲 | ✅ 单物理；火/毒/暴击按蓝图扩展 |
| 翻滚取消 / 招架 / 闪避 | 反应阶段系统 | 缺失 | 状态效果子域（后续阶段） |
| 技能 / 精力 / 冷却 | `menu::SKILLS` | 缺失 | 资源与技能组件（待 UI 阶段） |
| HUD / 战斗日志 | `display/hud`、egui 面板 | 缺失 | 按需重建（先 console `info!`） |
| 战斗重置 | `timeline::reset_system` | 缺失 | 简单场景重载或 R 键重置 |
| 旧格子系统 | grid/回合时间线 | 已删除 | 方向废弃 |

## 二、新范式下的重设计

### 1. 阵营与单位

`combat/faction.rs`：一个 `Faction` 组件代替旧的 `Player` / `Enemy` 两个标记
（同一机制、同一查询路径，便于以后多阵营 / 中立单位）。

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Faction { #[default] Player, Enemy }
```

单位不再是 setup 里一大坨 spawn，而是各角色一个场景工厂，组合小部件：

```rust
pub fn dummy_archer() -> impl Scene {
    bsn! {
        Faction::Enemy
        Health { max: 50.0, current: 50.0 }
        HitRadius(0.8)
        Collidable
        Velocity(Vec3::ZERO)
        Transform::from_xyz(0.0, 0.0, 0.0)
        WorldAssetRoot("models/nature/rock_largeA.glb#Scene0")
    }
}
```

> 角色模型从纸片占位切换为 glTF；需要外观 / 逻辑分开时，用子实体
> （`Children []`）挂视觉，根实体只放逻辑组件——与纸片时代的“Billboard
> 子实体”思路同构，但由 BSN 原生表达。

### 2. 玩家移动：键盘只翻译，落盘走消息

`control/` 模块：

- `control/components.rs`：`MoveSpeed(f32)`；
- `control/messages.rs`：`MoveCommand { axis: Vec2 }`（键盘归一化方向，
  世界空间速度 = axis × MoveSpeed）；
- `control/input.rs`：`player_move_input_system`（`ButtonInput` → `MoveCommand`，
  只翻译不改状态）；
- `control/systems.rs`：`apply_move_command_system`
  （`Query<(Entity, &mut Velocity, &MoveSpeed), With<Faction::Player?>>` 按需
  使用 `Faction`，消费消息设置 `Velocity`）。

停止键 / 松开方向键后发 `MoveCommand { axis: Vec2::ZERO }` 归零速度，
统一走消息，不旁路 ECS。

### 3. 攻击生成：一次“攻击实体 + 效果组件”而不是技能枚举

`attacks/` 子域，每个攻击类型一个场景工厂 + 载荷组件：

```rust
pub fn arrow(from: Vec3, to: Entity) -> impl Scene {
    let dir = /* 世界坐标朝向目标 */;
    bsn! {
        Velocity(dir.normalize() * 12.0)
        Projectile { max_hits: 1, current_hits: 0, finished: false }
        PhysicalDamage(10.0)
        HitRadius(0.2)
        Owner(from_owner_entity /* EntityTemplate */)
        Mesh3d(asset_value(Cuboid::new(0.1, 0.1, 0.5)))
        Transform { translation: from, ..default() }
    }
}
```

近战：生成带 `Melee { range, arc }` + 临时 `Lifetime` 的短命攻击实体，
命中由新增 `targeting::melee` 系统负责（读取 `Melee`，对扇形/圆形范围内实体
挂 `Target(s)`），伤害与清理复用现有 damage / cleanup。这样旧 `menu` 的技能
表 + 声明 + 提交全部被“场景工厂 + 冷却组件”取代。

### 4. 敌人 AI：持续决策，不依赖回合

`ai/` 模块：

- 组件：`EnemyBrain { engage_range: f32 }`、`AttackCooldown(Timer)`；
- 系统：`enemy_ai_system`（Update 链最前）：
  1. 找最近的 `Faction::Player`；
  2. 距离 > engage_range → 写自己的 `Velocity` 指向玩家；
  3. 距离 ≤ engage_range 且冷却结束 → 生成攻击场景并重置冷却；
  4. 攻击实体走通用流水线，AI 不感知命中结果。

冷却用组件内 `Timer`，配合 `Time<Virtual>` 天然支持暂停/停表，不需要回合门控。

### 5. 日志 / 状态展示（先最小化）

伤害与死亡已经由 `DamageEvent` / `DeathEvent` 解耦；当前用 `info!` 输出。
重建 `BattleLog` 时做成 `Resource + MessageReader`（订阅 `DamageEvent` /
`DeathEvent` 之外再加一个 `CombatLog` 消息），UI 显示推迟到有 egui/UI 决策后。

### 6. 战斗重置

`restart.rs`：R 键 → `ResetBattle` 消息 → `restart_system`：despawn 旧单位 /
攻击实体 → 重新 `spawn_scene_list`（玩家 + 敌人场景工厂），复用同一套场景。

## 三、建议实现顺序

1. **单位与阵营**：`Faction` + `dummy` 敌/我场景工厂（先替换测试假人，肉眼可见）；
2. **玩家控制**：移动输入流水线；
3. **攻击工厂**：箭矢场景 + 由玩家按键发射，接上现有 damage/清理；
4. **敌人 AI**：追踪 + 自动攻击 + 冷却；
5. **近战 / AOE / 状态效果 / 日志 / 重置**：按蓝图逐项扩展。

> 每步都是一个独立可编译、可测试的小提交；不实现旧版 “技能表 + 回合门控”，
> 用“组件 + 场景 + 消息流水线”替代。
