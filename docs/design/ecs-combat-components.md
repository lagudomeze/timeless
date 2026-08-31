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
3. **行动即实体**：没有行动枚举 / 行动列表——`Attack` / `MoveTo` / `Roll` / `Fireball` /
   `Parry` 是动作实体的载荷组件，配合 `ScheduledAction` + 状态标记
   `Declared → Pending → Committed` 经时间线调度，执行后 despawn。
   菜单只是「能力组件（`Can*`）+ 技能展示表」→ 动作实体的声明层。
4. **领域层保持纯函数**：小组件在裁决前组装成领域层 `AttackStats` 传入
   `resolve_combat`，伤害公式仍然零 Bevy 依赖、可独立单测。
5. **两阶段结算**：阶段 1 只读裁决（射程 / 同刻破势 / 闪避 / 招架）并挂载
   `CombatResult`，阶段 2 统一扣血 + 反制 + despawn——消除先手优势，保证公平。
   防御（`Dodging` / `Parrying` 标记）不内嵌在裁决公式里，可随时插入新防御。
6. **实体构建用 BSN**：动作实体用 `bsn!` + `spawn_scene` 构建（组件派生
   `Default + Clone`，含 `Entity` 字段的派生 `FromTemplate`）。

## 组件清单

| 组件 | 字段 | 语义 | 生命周期 |
| :--- | :--- | :--- | :--- |
| `Health` | `current` / `max` | 生命值 | 常驻 |
| `Damage` | `u32` | 单次攻击伤害（输出侧） | 常驻 |
| `AttackFrame` | `u32` | 速度帧，越小越先命中 | 常驻 |
| `AttackRange` | `u32` | 射程（切比雪夫） | 常驻 |
| `Impact` | `u32` | 破势，全同时命中时打断对方 | 常驻 |
| `Stamina` | `current` / `max` | 翻滚/翻滚取消消耗 | 常驻 |
| `CanAttack` / `CanMove` / `CanRoll` / `CanFireball` | — | 可行动能力标记（驱动菜单） | 常驻 |
| `ScheduledAction` | `execute_at` / `cast_duration` / `actor` | 动作实体调度数据（前摇 + 绝对执行时刻） | 动作实体 |
| `Declared` / `Pending` / `Committed` | —（ZST） | 动作实体状态：未入队 / 等待中 / 待结算 | 动作实体 |
| `Attack`（载荷） | `target` / `damage` / `range` / `impact` | 攻击动作 | 动作实体 |
| `MoveTo`（载荷） | `velocity: IVec2` | 移动动作，执行时 `Position += velocity` | 动作实体 |
| `Roll`（载荷） | `from: IVec2` | 翻滚动作（后退 + 挂 `Dodging`） | 动作实体 |
| `Fireball`（载荷） | `target` / `speed` / `amount` / `radius` | 火球动作 | 动作实体 |
| `Parry`（载荷） | `target_attack: Entity` | 招架动作（挂 `Parrying`） | 动作实体 |
| `CombatResult` | `target` / `final_damage` / `counter_damage` / `order` | 阶段 1 计算结果，阶段 2 应用 | 结算帧 |
| `Dodging` / `Parrying` | — / `target_attack` | 单位防御标记（本回合闪避 / 招架） | 回合内 |
| `Projectile` | —（标记） | 投射物标记，配合 `LinearVelocity` + `Destination` | 飞行中 |
| `LinearVelocity` | `Vec2`（格/秒） | 线性速度，位置 += 速度×dt 自动移动 | 飞行中 |
| `Destination` | `IVec2` | 投射物目标格（到达后触发爆炸） | 飞行中 |
| `ExplosionDamage` | `amount` / `radius` | 到达后范围伤害 | 飞行中 |

每个单位每回合至多声明一个动作（选择 / WASD 会先清掉旧的 Declared 动作实体）；
动作实体与单位分离，调度器不感知载荷类型。

## 调度与两阶段结算

```text
finalize_declared_actions（Declared → Pending，分配 execute_at = now + 前摇）
  → scheduler（Time<Virtual> 到期 → Committed）
  → 执行器（move / roll / parry / fireball：位移、挂防御标记、生成投射物）
  → combat_phase1_system（只读裁决：射程 / 同刻破势 / 闪避 / 招架 → CombatResult）
  → combat_phase2_system（统一扣血 + 反制 + despawn → HitLanded）
  → death_check_system（清场 + GameOver）
```

同刻互击（`execute_at` 相同且互为目标）用领域层 `resolve_combat`
（帧相同 → 距离 → 破势）裁决；单方攻击用 `resolve_attack` 做射程判定。
伤害应用统一走 `apply_hit` 入口并广播 `HitLanded`。

## 火球（投射物）示例

火球是一个普通实体，不写专属系统分支，只靠组件组合表达：

```rust
commands.spawn((
    Projectile,                     // 投射物标记
    LinearVelocity(velocity),       // 速度（格/秒）：位置 += 速度×dt 自动飞行
    Destination(to),                // 目标格：到达后触发爆炸
    ExplosionDamage { amount: 8, radius: 1 }, // 到达后触发
    Mesh3d(sphere),                 // 表现
    MeshMaterial3d(fire_material),
    Transform::from_xyz(...),
    Visibility::default(),
));
```

`movement::projectile_motion_system` 按 位置 += 速度×dt 自动推进，飞抵目标格后广播
`ProjectileArrived`；`combat::explosion_system` 按 `radius`（切比雪夫距离）对范围内
所有 `Health` 实体结算并销毁投射物（范围内无单位则落空）。爆炸伤害应用复用
`combat::apply_hit`，与近战命中走同一条事件链。

## 结算流水线（main.rs 系统链）

```text
sync_pause_system（按阶段同步 Time<Virtual> 暂停）
  → ai_system（敌人声明动作实体）
  → menu 键盘→消息→选择/提交管线（玩家声明动作实体）
  → finalize_declared_actions（Declared → Pending）
  → reaction_trigger_system（双方 Pending 攻击 → Reaction 暂停）
  → scheduler（Pending → Committed）
  → move / roll / parry / fireball 执行器（位移、防御标记、投射物）
  → combat_phase1 → combat_phase2（两阶段结算）
  → movement::projectile_motion_system（位置+速度自动飞行 → ProjectileArrived）
  → combat::explosion_system（到达爆炸 + 销毁投射物）
  → combat::death_check_system（清场 + GameOver）
  → timeline::turn_end_system（动作清空 → 下一回合 Decision）
  → combat::message_log_system（HitLanded 日志）
  → display（同步坐标 → 纸片朝向 → 移动箭头 → HUD → 相机）
```

暂停由 `Time<Virtual>` 驱动（Decision / Reaction / GameOver 冻结，Resolving 放行），
不再用手写阶段门控；死亡检查独立成系统，火球在任何时刻炸死目标都能正确收尾。

## 领域文件归属

- `timeline.rs`：`ScheduledAction` / `Declared` / `Pending` / `Committed`、
  回合阶段机（`TurnPhase` + `Time<Virtual>` 暂停同步）、
  `finalize_declared_actions` / `scheduler` / `turn_end_system`
- `combat.rs`：`Health` / `Damage` / 攻击属性三件套 / `Stamina` +
  动作载荷（`Attack` / `Parry` / `Fireball`）、防御标记（`Dodging` / `Parrying`）、
  `CombatResult` / `ExplosionDamage` / `FireballAssets` +
  `ai_system` / `reaction_trigger_system` / `combat_phase1_system` /
  `combat_phase2_system` / `fireball_executor` / `parry_executor` /
  `explosion_system` / `death_check_system` / `message_log_system` / `spawn_fireball`
- `movement.rs`：`Position` / `GridMath` / `Projectile` / `LinearVelocity` /
  `Destination` + 位移动作载荷（`MoveTo` / `Roll`）、用户移动指令（`MoveInput`）+
  `move_input_system` / `move_executor` / `roll_executor` / `projectile_motion_system`；
  火球/爆炸归 `combat.rs`
- `menu.rs`：能力标记（`Can*`）、`SKILLS` / `REACTIONS` 展示表、`MenuSelection`、
  键盘翻译 + 面板消息 → 单一职责系统（选择 / 声明动作 / 提交 / 反应执行）
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
> 领域层 `AttackStats` 保留作为纯函数入参。旧代码若引用挂载在单位上的行动组件，
> 应改为查询动作实体（`ScheduledAction.actor == 单位` 的 `Declared` / `Pending` 动作）。
