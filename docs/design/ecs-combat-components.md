# ECS 战斗组件化设计

> 应用层 `timeless-app` 战斗/移动领域的组件建模规范。配套阅读：
> [架构原则与分层](architecture.md) · [时间线系统](timeline.md) ·
> [Bevy 0.19 速查](../bevy/bevy-019.md)。

## 设计原则

1. **实体 = 多个小组件的组合**。不把"单位属性"塞进一个聚合结构（旧 `AttackStats` /
   `ActionQueue`），而是把每个可独立变化、可独立判定的维度拆成组件。
2. **成对出现**：`Health`（生命）对应 `Damage`（伤害）、`AttackIntent` 对应
   `Interrupted`（打断）、`DodgeActive` 对应"免疫本次攻击"。攻击方带什么组件，
   命中时就按什么结算。
3. **意图即组件**：回合行动不是枚举值，而是挂载在实体上的意图组件
   （`AttackIntent` / `MoveIntent` / `RetreatIntent`），结算后由对应领域移除。
4. **领域层保持纯函数**：小组件在裁决前组装成领域层 `AttackStats` 传入
   `resolve_combat`，伤害公式仍然零 Bevy 依赖、可独立单测。

## 组件清单

| 组件 | 字段 | 语义 | 生命周期 |
| :--- | :--- | :--- | :--- |
| `Health` | `current` / `max` | 生命值 | 常驻 |
| `Damage` | `u32` | 单次攻击伤害（输出侧） | 常驻 |
| `AttackFrame` | `u32` | 速度帧，越小越先命中 | 常驻 |
| `AttackRange` | `u32` | 射程（切比雪夫） | 常驻 |
| `Impact` | `u32` | 破势，全同时命中时打断对方 | 常驻 |
| `Stamina` | `current` / `max` | 翻滚/翻滚取消消耗 | 常驻 |
| `AttackIntent` | — | 本回合将攻击 | 决策→结算 |
| `MoveIntent` | `target: Position` | 本回合移动目标 | 决策→结算 |
| `RetreatIntent` | `from: GridPos` | 翻滚后退（远离敌人） | 决策→结算 |
| `DodgeActive` | — | 闪躲无敌帧，免疫敌方攻击 | 决策→结算 |
| `Parry` | — | 招架（Phase 3 预留） | — |
| `Interrupted` | — | 破势打断，攻击取消 | 结算帧 |
| `Projectile` | `speed: f32` | 投射物速度（格/秒） | 飞行中 |
| `Destination` | `GridPos` | 投射物目的地 | 飞行中 |
| `ExplosionDamage` | `amount` / `radius` | 到达后范围伤害 | 飞行中 |

意图组件是互斥的：任何时刻一个单位至多挂一种行动意图
（`set_pending_action` 先清除旧意图再挂新意图）。

## 火球（投射物）示例

火球是一个普通实体，不写专属系统分支，只靠组件组合表达：

```rust
commands.spawn((
    Position(from),                 // 逻辑位置
    Destination(to),                // 目的地
    Projectile { speed: 4.0 },      // 每帧推进 4 格/秒
    ExplosionDamage { amount: 8, radius: 1 }, // 到达后触发
    Mesh3d(sphere),                 // 表现
    MeshMaterial3d(fire_material),
    Transform::from_xyz(...),
    Visibility::default(),
));
```

`movement::projectile_system` 每帧朝目的地插值推进；到达后按 `radius`
（切比雪夫距离）对范围内所有 `Health` 实体结算，广播 `HitLanded` 后销毁。
爆炸伤害应用复用 `combat::apply_hit`，与近战命中走同一条事件链。

## 结算流水线（main.rs 系统链）

```text
ai_system（敌人生成意图）
  → input_system / reaction_system（玩家写入意图）
  → movement::apply_move_intents_system（先位移，改变站位）
  → combat::resolve_system（领域层裁决 + 伤害）
  → movement::projectile_system（投射物每帧推进/爆炸）
  → combat::death_check_system（清场 + GameOver）
  → combat::message_log_system（事件链日志）
  → display（同步坐标 → 纸片朝向 → 移动箭头 → HUD → 相机）
```

死亡检查独立成系统，因此火球在任意阶段（包括决策暂停）炸死目标都能正确
结束战斗，不依赖回合结算入口。

## 领域文件归属

- `combat.rs`：`Health` / `Damage` / 攻击属性三件套 / `Stamina` /
  `AttackIntent` / `DodgeActive` / `Parry` / `Interrupted` +
  `ai_system` / `resolve_system` / `death_check_system` / `message_log_system`
- `movement.rs`：`Position` / `MoveIntent` / `RetreatIntent` / `Projectile` /
  `Destination` / `ExplosionDamage` + 位移结算与投射物系统
- `menu.rs`：`Action` 枚举（仅 UI 选项标识）、`MenuSelection`、面板消息 +
  输入系统（把选择转换为意图组件）
- `display/`：表现子域（`camera` / `unit` / `hints` / `hud` / `map`），
  不参与规则

## 社区参考

- **bevy-compose**（`Health(i32)` + `Damage(i32)` 小组件）：验证了"属性成对拆分"
  的可行性，本项目把伤害量放进 `Damage` 组件而非攻击聚合。
- **bevy_turn_based_combat**（phases / roll initiative / 动作队列）：回合状态机
  与"意图先行、结算后清"的节奏参考。
- **maciejglowka 的 bevy_turn_based**（2024 博客，action queue + one-shot
  systems）：行动实体化/意图组件、`hit → damage → kill` 事件链；本项目据此把
  伤害应用与死亡检查拆成独立系统。
- **endless docs / projectiles**：投射物 movement 与 collision 分离的做法，
  对应本项目的 `Projectile` / `Destination` / `ExplosionDamage` 拆分。

> 迁移提示：旧 `AttackStats`（四合一）与 `ActionQueue` 已从应用层移除，
> 领域层 `AttackStats` 保留作为纯函数入参。旧代码若引用 `ActionQueue`，
> 应改为查询意图组件（`Option<&AttackIntent>` 等）。
