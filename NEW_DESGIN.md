# 任务：重构 Bevy 时间线与战斗结算

## 项目背景

Bevy 0.19 游戏，无回合战斗时间线 + 戴森球式供应链 + roguelike 策略。
当前架构见仓库 `docs/architecture.md` / `docs/timeline.md` / `docs/components.md`。

本次重构目标：去掉冗余状态标记和仲裁资源，改用「时间戳 + 事件」模型，
并新增「反应系统」（威胁检测 + Focus 资源）。

## 核心概念

### 1. DecisionSlot（替代 Ready / BusyRecovery）

```rust
#[derive(Component)]
enum DecisionSlot { Empty, Filled }
```

挂在 actor 上。语义：
- `Empty` = 空闲，可以声明 action
- `Filled` = 忙碌（前摇中 或 后摇中），不能声明

阶段区分不靠槽状态，靠：
- `Filled` + action 实体存在 = 前摇
- `Filled` + action 实体不存在 + `Busy { until }` 存在 = 后摇

### 2. ScheduledAction（去三态）

```rust
#[derive(Component)]
struct ScheduledAction {
    actor: Entity,
    declared_at: f32,
    execute_at: f32,
    recovery: f32,
    interrupt_resist: i32,
}
```

**删除**：`Declared` / `Pending` / `Committed` 三态组件。
**状态由时间戳推导**：
- `now < execute_at` = 前摇（可撤销、可打断）
- `now >= execute_at` = 执行器应处理

### 3. Busy（后摇）

```rust
#[derive(Component)]
struct Busy { until: f32 }
```

后摇期间挂在 actor 上。`recovery_system` 到点清 Busy + 槽 Filled → Empty。

### 4. Cancellable

```rust
#[derive(Component)]
enum Cancellable {
    Free,
    Cost { refund: u32, penalty: u32 },
    Never,
}
```

挂在 action 上。**替代** `ActionCost` / `CancelCost` / `Uncancellable`。

## 暂停机制

### PauseRequest

```rust
#[derive(Message)]
enum PauseRequest {
    Pause(String),
    Resume(String),
}

#[derive(Resource, Default)]
struct PauseReasons(HashSet<String>);
```

### 系统

- `process_pause_requests`：边沿触发，插入/移除原因
- `apply_clock`：**唯一**写 `Time<Virtual>` 的地方
- 位于 `ClockSet`，在 `Update` 帧末运行，影响下一帧

### 暂停原因

- `"manual"`：Space 切换
- `"slot_empty"`：存在空 `InputDriven` 槽
- `"threat"`：combat 检测到有威胁瞄准玩家

### 冻结规则

```
frozen ⟺ PauseReasons 非空
```

### 删除

- `timeline_gate_system`（拆成多个 `compute_*` 系统）
- `pause_toggle_system` 中的 `Time<Virtual>` 写入

## 行动生命周期

### 声明

```
spawn action 实体（ScheduledAction + payload + Cancellable）
actor.slot: Empty → Filled
（可选）扣资源
```

### 执行

各执行器自己处理：

```rust
if now < sa.execute_at { continue; }

// 落地效果
（发 DamageEvent / trigger InterruptEvent / insert debuff / spawn 投射物）

// 收尾
commands.entity(action).despawn();
commands.entity(sa.actor).insert(Busy { until: busy_until });
```

**删除**：`scheduler_system`、`begin_action` / `end_action` 自由函数。

### 后摇

```rust
fn recovery_system(
    q: Query<(Entity, &Busy, &mut DecisionSlot)>,
    time: Res<Time<Virtual>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs();
    for (entity, busy, mut slot) in q {
        if now >= busy.until {
            *slot = DecisionSlot::Empty;
            commands.entity(entity).remove::<Busy>();
        }
    }
}
```

### 撤销

```rust
fn undo_system(
    q: Query<(Entity, &ScheduledAction, &Cancellable)>,
    mut actors: Query<&mut DecisionSlot>,
    time: Res<Time<Virtual>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs();
    for (action, sa, cancellable) in &q {
        if now >= sa.execute_at { continue; }   // 来不及
        if matches!(cancellable, Cancellable::Never) { continue; }

        commands.entity(action).despawn();
        commands.entity(sa.actor).insert(DecisionSlot::Empty);
        // 按 cancellable 处理退款
    }
}
```

## 伤害

```rust
#[derive(Message)]
struct DamageEvent {
    source: Option<Entity>,
    target: Entity,
    amount: i32,
}
```

**流程**：
- 执行器 / 命中系统写 `DamageEvent`
- `apply_damage_system` 读事件 → 扣 `Health.current`
- 首次归零（`was_alive && now_dead`）发 `DeathEvent { entity, killer }`
- `despawn_dead_system` 帧末查 `Health.current <= 0` → despawn

**删除**：
- `Arbitration` 资源
- `phase1_arbitrate_system` / `phase2_apply_system`
- `ModifyHealthEvent`
- `AttackResolved`

**关键规则**：
- 伤害不需要两阶段裁决（纯减法，可交换）
- 扣到负数继续扣，不提前终止
- 同一实体只发一次 `DeathEvent`

## 打断

```rust
#[derive(EntityEvent)]
struct InterruptEvent {
    source: Entity,
    power: i32,
}
```

用 `commands.trigger_targets(InterruptEvent { ... }, target)` 定向发送。

**Observer 处理**：

```rust
app.add_observer(|trigger: On<InterruptEvent>, ...| {
    let target = trigger.entity;
    let now = time.elapsed_secs();

    // 找 target 身上「还没执行」的 action
    let Some((action, sa)) = actions.iter()
        .find(|(_, sa)| sa.actor == target && now < sa.execute_at)
    else { return; };

    // 掷骰对抗
    let atk = trigger.power + 3 + rng.roll_3d5();
    let def = sa.interrupt_resist + 3 + rng.roll_3d5();
    if atk >= def {
        commands.entity(action).despawn();
        commands.entity(target).insert(DecisionSlot::Empty);
    }
});
```

**规则**：
- 只打断 `execute_at > now` 的 action
- `power == 0` 直接返回（不触发对抗）
- 本帧到点的 action 已经执行，打不断

**删除**：`Impact` 破势相关代码。

## debuff

每种 debuff 一个独立组件，执行器直接 `commands.entity(target).insert(Burn { ... })`。

**不做**中心枚举、不做 `Vec<Effect>`、不做 `Resistance` 表。

每种 debuff 有自己的 tick / 过期系统。

## 伤害类型

每种伤害类型一个独立组件 + 一个独立系统：

```rust
struct FireDamage { amount: i32 }
struct FireResist { factor: f32 }

fn apply_fire_damage_system(...) { ... }
```

**加一种伤害** = 加一个组件 + 一个系统。

没有中心 `DamageType` 枚举，没有 `Resistance` 表。

## 反应系统

### 威胁声明

每个 action 自己声明威胁覆盖的格：

```rust
#[derive(Component)]
struct Threatens { cells: Vec<Cell> }
```

- 火球：`Threatens { cells: vec![target_cell] }`
- 近战：`Threatens { cells: melee_arc_cells(...) }`
- 箭：`Threatens { cells: trajectory_cells(...) }`

飞行中的投射物也要声明：

```rust
#[derive(Component)]
struct TargetCell(Cell);   // 投射物瞄准的格
```

### 威胁检测

combat 领域的系统，每帧检测：

```rust
fn detect_threat_system(
    actions: Query<(&ScheduledAction, &Threatens)>,
    projectiles: Query<&TargetCell>,
    players: Query<&Cell, With<InputDriven>>,
    time: Res<Time<Virtual>>,
    mut pause: MessageWriter<PauseRequest>,
) {
    let now = time.elapsed_secs();
    let player_cells: HashSet<Cell> = players.iter().copied().collect();

    let threat = actions.iter().any(|(sa, t)| {
        now < sa.execute_at && t.cells.iter().any(|c| player_cells.contains(c))
    }) || projectiles.iter().any(|target| {
        player_cells.contains(&target.0)
    });

    if threat {
        pause.write(PauseRequest::Pause("threat".into()));
    } else {
        pause.write(PauseRequest::Resume("threat".into()));
    }
}
```

### Focus 资源

```rust
#[derive(Resource)]
struct Focus { current: u32, max: u32 }

// 缓慢恢复：每 10 秒回 1 点，上限 3
const FOCUS_RECOVER_INTERVAL: f32 = 10.0;
const FOCUS_MAX: u32 = 3;

fn recover_focus_system(
    mut focus: ResMut<Focus>,
    time: Res<Time<Virtual>>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer >= FOCUS_RECOVER_INTERVAL {
        *timer -= FOCUS_RECOVER_INTERVAL;
        focus.current = (focus.current + 1).min(focus.max);
    }
}
```

**恢复条件**：虚拟时间推进时（`Time<Virtual>` 走），冻结时不恢复。

### 反应流程

1. 威胁检测 → `Pause("threat")` → 世界冻结
2. 玩家可以：
   - **不做任何事** → 继续冻结（等威胁消失）
   - **撤销当前 action**（前摇中才行）→ 槽 Empty
   - **撤销 + 声明新 action**（消耗 1 Focus，前摇归零）→ 新 action `execute_at = now`
3. 玩家声明新 action 时，如果 `Focus.current > 0` 且想归零前摇：
   - 扣 1 Focus
   - `execute_at = now`
4. 威胁消失（敌人前摇结束 / 打断 / 投射物消失）→ `Resume("threat")` → 解冻

### Focus 换前摇

```rust
fn declare_with_focus(
    mut focus: Query<&mut Focus>,
    mut actor: Query<&DecisionSlot>,
    // ...
) {
    if focus.current > 0 && wants_zero_windup {
        focus.current -= 1;
        execute_at = now;   // 下一帧执行
    } else {
        execute_at = now + windup;
    }
}
```

**关键时序**：`execute_at = now` 的 action 由**下一帧**的执行器处理
（声明系统和执行器在同帧，执行器先跑）。这带来一帧延迟，
玩家感知为「瞬时生效」，语义上不属于前摇。

## 系统集顺序

```
Startup:  PreloadSet → AssemblySet

Update:
  SpawnSet
  InputSet
  InteractionSet
  AiSet

  TimelineSet
    compute_manual_pause
    compute_player_awaiting
    interrupt
    undo
    recovery
    recover_focus

  MovementSet
    declare_move / declare_jump / declare_roll
    move_action_executor / jump_action_executor / roll_executor
    move_entities / follow_terrain / jump_motion

  CombatSet
    detect_threat_system
    declare_fireball / declare_melee / declare_parry
    fireball_action_executor / melee_action_executor / parry_executor
    fireball_arrival / explosion
    apply_damage           ← 扣 Health + DeathEvent
    apply_interrupt        ← Observer
    burn_tick / slow_tick / ...
    despawn_hit_attacks
    despawn_dead           ← 最后

  VoxelRenderSet
  PresentationSet

  ClockSet                  ← 帧末
    process_pause_requests
    apply_clock
```

## 需要删除的东西

**组件**：
`Ready` / `BusyRecovery` / `Declared` / `Pending` / `Committed` /
`ActionCost` / `CancelCost` / `Uncancellable` / `Impact` /
`CollisionTarget`（视情况）

**资源**：
`Arbitration` / `Timeline.draft` / `Timeline.waiting_for_input`

**系统**：
`timeline_gate_system` / `scheduler_system` /
`phase1_arbitrate_system` / `phase2_apply_system` /
`commit_bridge_system` / `request_damage_system`

**自由函数**：
`begin_action` / `end_action` / `end_action_until` / `insert_on_actor`

**消息**：
`ModifyHealthEvent` / `AttackResolved`

## 验收标准

- [ ] `cargo check` 通过
- [ ] `cargo test` 通过
- [ ] 时间线领域没有 `Time<Virtual>` 的写入（只有 `apply_clock`）
- [ ] 没有三态标记、`Arbitration`、`phase1` / `phase2`
- [ ] 加一种伤害类型只需：加组件 + 加系统 + 挂载
- [ ] 手动暂停不再只前进一帧
- [ ] 威胁触发时世界冻结，玩家可以做反应
- [ ] Focus 消耗后前摇归零
- [ ] 火球飞行中被击杀时能正常死亡（走通用死亡链路）

## 约束

- `configure_pipeline` 是唯一的顺序声明入口
- 领域层零 Bevy 依赖（`combat/formula/domain.rs` 不变或简化）
- `world` 零渲染依赖
- 每步独立可测试，不要一次性大重写

## 实施步骤

1. **暂停机制**：加 `PauseRequest` + `PauseReasons` + `apply_clock`，删除旧门控
2. **DecisionSlot**：替代 `Ready` / `BusyRecovery`
3. **去三态**：删除 `Declared` / `Pending` / `Committed`，执行器自己收尾
4. **伤害简化**：删 `Arbitration` / phase1 / phase2，直接扣血
5. **打断**：加 `InterruptEvent` + Observer
6. **反应系统**：加 `Threatens` + `detect_threat_system` + `Focus`
