# 配置：数值外置（`config/`）

> ✅ **已落地**：动作数值外置 + **热重载**（`cargo run --features hot-reload` 下改文件存盘即生效）。
> 登记在 [../TODO.md](../TODO.md) 的「动作数值外置」。

## 一、为什么不在 `assets/` 里

| | `assets/` | `config/` |
| :--- | :--- | :--- |
| 是什么 | **素材**（贴图 / 字体 / 模型） | **数值**（前摇 / 后摇 / 消耗 / 威力） |
| 改它等于 | 换皮 | 换手感 |
| 走哪条路 | bevy 资产管线（异步、可热重载、会打包） | **普通文件读写**（`PreStartup` 同步读） |
| 发行时 | 必须在 | **可以不带**（用内置默认值跑） |

**不走资产管线的原因**：数值必须在**各域注册技能定义之前**就绪（各域在 `Startup`
里把 `AbilityDef` 交上来，而那些定义要读配置）。资产管线是异步的，用它就得处理
"加载好了没"的时序问题；而这是本地小文件，同步读没有代价。

## 二、文件长什么样

`config/actions.ron`，字段名与 Rust 结构体一致（`serde` 默认蛇形）：

```ron
(
    fireball: (
        windup: 0.3,       // 前摇（秒）
        recovery: 0.5,     // 后摇（秒）
        interrupt_resist: 2,
        cost: 2,           // 精力
        power: 12,         // 展示 + 参考威力
        frame: 7,          // 速度帧（"谁先动"的读数）
    ),
)
```

**只写想改的字段**：结构体带 `#[serde(default)]`，没写的自动用内置默认值补齐。
所以"只想把火球改贵一点"不必抄一整份文件。

`move_` 带下划线是因为 `move` 是 Rust 关键字——**文件里也写成 `move_`**，
与字段名一致，不引入第二套命名。

## 三、失败怎么办

```text
文件不存在        → 内置默认值（= 各域常量）：**没有它也能跑**
文件在但解析失败  → **大声报错**（带行号）→ 退回默认值，**不 panic**
```

最后一条是刻意的：打错的数字不该让游戏起不来；但也不该被静默忽略——
"改了没效果、又不知道为什么"比崩溃更难查。

## 四、一处真相

各域的常量（`MOVE_TIMING` / `FIREBALL_TIMING`…）**保留**，它们是**默认值**，
也是没有配置文件时的行为。配置是**覆盖**：

```text
ActionConfig（资源，来自 .ron 或默认值）
      ├─▶ 技能目录（各域的 AbilityDef，Startup 时交上去）
      ├─▶ 声明系统（玩家按键 / AI 决策 → 行动实体）
      └─▶ HUD（帮助面板不再写死秒数）
```

**改一处全都跟着变**——`a_changed_config_reaches_the_catalogue_and_the_declaration`
这条测试分三段钉住它（目录 / 行动实体 / 玩家按键声明），
任何一段断了都会红。

## 五、还没做的

| 缺口 | 说明 |
| :--- | :--- |
| **`ActionRegistry`** | HUD 仍读 `SKILLS` 数组而非技能目录（见 `TODO.md` 的 M24c 审计结论） |
| **地形 / 相机等数值** | 本次只做了**动作**相关；`TerrainConfig` 等已有自己的资源，外置与否另说 |

### 热重载 ✅ 已落地

开发期跑 `cargo run --features hot-reload`：改 `config/actions.ron` **存盘即生效**，
不用重编译、也不用重启。

```text
文件变了 ─▶ 重读 + 解析 ─▶ 换掉 ActionConfig 资源 ─▶ 广播 ReloadActionConfig
                                                       │
                         各机制域重跑自己的注册系统（读新配置、交新定义）
```

- **为什么自己挂监视器**：`config/` 走普通文件读写（不是资产管线），bevy 的
  `file_watcher` 看不到它，所以 `reload.rs` 自己用 `notify-debouncer-full`
  （可选依赖，只在 feature 下进依赖树）。去抖 300ms——编辑器存盘会连发好几个事件。
- **为什么不用新机制刷新目录**：`RegisterAbility` 按 id 覆盖、**不动菜单顺序**，
  各域的注册系统本来就是"读配置 → 交定义"，重跑一次即可（见 `docs/skills.md` 第二节）。
- **改动能立刻被读到**，不需要重启：各声明系统与三个攻击执行器**每帧**从
  `ActionConfig` 资源读数值。
- **失败不崩**（与启动时同一条策略）：解析失败 → 报错并**保留上一份好配置**；
  文件被删 → 说一声、保留现值。热重载最忌讳"改错一个数字整局崩掉"。

验收（单测）：`reload::tests` 三条把处置策略的三条分支都钉住（合法 → 换；
文件不在 → 保留；语法错 → 保留）。**实机验了整条往返**：跑 `--features hot-reload`，
把 `fireball.power` 从 12 改成 40 存盘 → 日志出现「已重新装载」→ 按 `Q` 打一发，
敌人掉 **41** 点血（40 + 武器加成 1），血从 50 → 9。把文件放回去再改一次，行为一致。

### 伤害数值 ✅ 已外置

`power` 不再只是展示用的读数——三个执行器（近战 / 箭矢 / 火球）现在都从
**配置**取伤害（`melee_damage(config)` / `arrow_damage(config)` / `fireball_damage(config)`，
缺省 = 各域常量），于是改 `config/actions.ron` 的 `power` **真的改掉扣血**：

```text
配置 melee.power ──▶ 近战执行器 ──▶ 攻击实体 PhysicalDamage ──▶ 命中公式（扣多少血）
```

**一处真相仍然成立**：技能目录的 `power` 与载荷的伤害是**同一次读配置**的结果
（`abilities_from(&config)` 与执行器读同一个字段），`menu_matches_the_catalogue`
钉住两边不许分叉。武器加成仍由执行器在生成时加上
（`equipment::weapon_damage(基础, 加成)`），装备域不认识配置。

验收：`a_changed_config_reaches_the_damage_formula`——把 `melee.power` 调成 23，
按近战热键打一发，敌人**恰好**掉 23 点；**验过不是空跑**（让执行器忽略配置、
退回 `MELEE_DAMAGE`(15) 后转红，报实际掉 15）。
