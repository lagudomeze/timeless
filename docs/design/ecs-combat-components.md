# ECS 战斗组件化设计

> 应用层 `timeless-app` 战斗/移动领域的组件建模规范。配套阅读：
> [架构原则与分层](architecture.md) · [时间线系统](timeline.md) ·
> [Bevy 0.19 速查](../bevy/bevy-019.md)。

## 设计原则

1. **实体 = 多个小组件的组合**。不把"单位属性"塞进一个聚合结构（旧 `AttackStats` /
   `ActionQueue`），而是把每个可独立变化、可独立判定的维度拆成组件。
2. **成对出现**：`Health`（生命）对应 `Damage`（伤害）、`Attack` 对应
   `Interrupted`（打断）、`Roll` 对应"免疫本次攻击"。攻击方带什么组件，
   命中时就按什么结算。
3. **行动即组件**：没有行动枚举 / 行动列表——`Attack` / `Move` / `Roll` / `Fireball`
   直接挂载在实体上，决策阶段插入、结算后移除；没有行动组件的单位视为「待机」。
   菜单只是「能力组件（`Can*`）+ 技能展示表」的映射层。
4. **领域层保持纯函数**：小组件在裁决前组装成领域层 `AttackStats` 传入
   `resolve_combat`，伤害公式仍然零 Bevy 依赖、可独立单测。
5. **防御即独立系统**：闪避（Roll）、招架+反制（Parry）不内嵌在裁决里，
   而是作为 `HitPending → dodge_system → parry_system → damage_system` 消息链上的
   独立过滤系统，可随时插入新防御（护甲、护盾等）。

## 组件清单

| 组件 | 字段 | 语义 | 生命周期 |
| :--- | :--- | :--- | :--- |
| `Health` | `current` / `max` | 生命值 | 常驻 |
| `Damage` | `u32` | 单次攻击伤害（输出侧） | 常驻 |
| `AttackFrame` | `u32` | 速度帧，越小越先命中 | 常驻 |
| `AttackRange` | `u32` | 射程（切比雪夫） | 常驻 |
| `Impact` | `u32` | 破势，全同时命中时打断对方 | 常驻 |
| `Stamina` | `current` / `max` | 翻滚/翻滚取消消耗 | 常驻 |
| `Attack` | — | 攻击行动 | 决策→结算 |
| `Move` | `target: Position` | 移动行动 | 决策→结算 |
| `Roll` | `from: GridPos` | 翻滚行动（位移 + 本回合闪避） | 决策→结算 |
| `Fireball` | `target` / `speed` / `amount` / `radius` | 火球行动 | 决策→结算 |
| `Parry` | — | 招架状态（格挡 + 反制一半伤害） | 反应→结算 |
| `Interrupted` | — | 破势打断，攻击取消 | 结算帧 |
| `CanAttack` / `CanMove` / `CanRoll` / `CanFireball` | — | 可行动能力标记（驱动菜单） | 常驻 |
| `Projectile` | `speed: f32` | 投射物速度（格/秒） | 飞行中 |
| `Destination` | `GridPos` | 投射物目的地 | 飞行中 |
| `ExplosionDamage` | `amount` / `radius` | 到达后范围伤害 | 飞行中 |

行动组件是互斥的：任何时刻一个单位至多挂一种行动组件
（菜单技能表的 `insert` 工厂在挂载前先移除其他行动组件）。

## 结算消息链（防御独立系统）

```text
resolve_system（按 帧→射程→破势 裁决）→ HitPending
  → dodge_system（实体带 Roll → 拦截命中）
  → parry_system（实体带 Parry → 拦截并写 CounterHit 反制）
  → damage_system（应用 HitPostParry + CounterHit）→ HitLanded
  → death_check_system（清场 + 阶段推进）
```

每个防御系统只关心自己的组件与消息类型，可以独立增删；
`CounterHit` 是招架的反制产物，伤害应用与普通命中走同一条 `apply_hit` 入口。

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
ai_system（敌人插入行动组件）
  → input_system / reaction_system（玩家插入行动组件）
  → movement::apply_move_intents_system（Move/Roll 位移先行）
  → combat::resolve_system（裁决 → HitPending）
  → combat::dodge_system → parry_system → damage_system（防御链 + 伤害）
  → movement::projectile_system（投射物每帧推进/爆炸）
  → combat::death_check_system（清场 + 阶段推进）
  → combat::message_log_system（事件链日志）
  → display（同步坐标 → 纸片朝向 → 移动箭头 → HUD → 相机）
```

死亡检查独立成系统，因此火球在任意阶段（包括决策暂停）炸死目标都能正确
结束战斗，不依赖回合结算入口。

## 领域文件归属

- `combat.rs`：`Health` / `Damage` / 攻击属性三件套 / `Stamina` /
  `Attack` / `Parry` / `Interrupted` +
  `ai_system` / `resolve_system` / `dodge_system` / `parry_system` /
  `damage_system` / `death_check_system` / `message_log_system`
- `movement.rs`：`Position` / `Move` / `Roll` / `Fireball` / `Projectile` /
  `Destination` / `ExplosionDamage` + 位移结算与投射物系统
- `menu.rs`：能力标记（`Can*`）、`SKILLS` / `REACTIONS` 展示表、`MenuSelection`、
  面板消息 + 输入系统（把选择转换为行动组件）
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
> 应改为查询行动组件（`Option<&Attack>` / `Option<&Move>` 等）；
> 旧 `DodgeActive` / `RetreatIntent` 已合并为 `Roll` 行动组件。
