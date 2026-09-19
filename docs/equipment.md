# 装备：容器与内容分离

> ⚠️ **目标设计，代码里完全没有**。关系模型的通用规则见 [relations.md](relations.md)。

## 一、两件事分开表达

| | 是什么 | 关系 |
| :--- | :--- | :--- |
| **装备槽位** `EquipmentSlot` | PC 的**物理延伸**（手里的位置、背后的位置） | `ChildOf(pc)` —— 槽位跟着 PC 动 |
| **装备物品** | **独立实体**，可以自由脱离 | `EquippedTo(slot)` —— 装备逻辑 |

**为什么不用一个关系**：`ChildOf` 管"跟着动"（空间），`EquippedTo` 管"装备在哪个槽"
（逻辑）。槽位本身要跟着 PC 走，所以槽位挂 `ChildOf`；而"这个物品装在哪个槽里"是
逻辑关系，卸下时物品未必被销毁（可能掉地上、进背包）。用一个 `ChildOf` 表达两件事，
卸下时就没法只断逻辑、保留空间。

```rust
#[derive(Component)]
pub struct EquipmentSlot { pub slot_type: SlotType }   // MainHand / OffHand / Armor …

#[derive(Component)]
#[relationship(relationship_target = EquippedItems)]
pub struct EquippedTo(Entity);          // 物品 → 槽位

#[derive(Component)]
#[relationship_target(relationship = EquippedTo)]
pub struct EquippedItems(Vec<Entity>);  // 槽位 → 它装着什么
```

**槽位不用 `linked_spawn`**：卸下装备时物品要活着（掉地上 / 进包），
所以"物品随槽位销毁"不是我们要的语义。

## 二、穿上 / 卸下

**穿上**（一次原子操作，三件事缺一不可）：

```text
① 物品 insert EquippedTo(slot)      —— 逻辑归属（hook 自动把它加进 slot 的 EquippedItems）
② 物品 insert ChildOf(slot)         —— 空间附着（跟着槽位动）
③ 校验：类型对不对、槽位有没有满（见第三节）
```

**卸下**：

```text
① 物品 remove EquippedTo            —— 断逻辑
② 物品 remove ChildOf               —— 断空间
③ **把 GlobalTransform 转回 Transform** —— 否则物品会跳到世界原点
```

第 ③ 步是最容易忘的一步：子实体的 `Transform` 是**局部**的，脱离父级后必须先把
累积的世界变换写回 `Transform`，再让它独立存在。

## 三、类型与容量校验

用 Observer 监听**关系插入**，而不是在采购 / UI / 拾取各处各写一遍：

```rust
// 槽只收对的东西，不符合当场退回
app.add_observer(|trigger: On<Insert, EquippedTo>, slots: Query<&EquipmentSlot>, items: Query<&ItemKind>| {
    // 槽类型 ≠ 物品类型 → 移除 EquippedTo（退回）
});
```

**校验点只有一个**：无论装备是被玩家拖上去、被 AI 装上、还是被脚本塞进去，
都经过同一个 Observer。

## 四、与战斗的关系

装备是**数据的来源**，不是新的机制：武器 / 护甲通过装备**往单位身上挂组件**：

| 装备提供 | 效果 |
| :--- | :--- |
| `ActionTiming`（前摇 / 后摇 / 打断抗性） | **武器决定动作节奏**——这也是为什么 `*_TIMING` 归载荷而不是时间线（见 [skills.md](skills.md)） |
| `PhysicalDamage` / `FireDamage`… | 攻击数值 |
| `CombatTags` | 能不能被打断 / 招架 / 格挡 |
| `AbilityDef` 的解锁 | "有剑才能用斩击"（`Requirement::HasWeapon`） |

**成长不改变结构**：属性影响公式与条件，不新增系统。这是设计总纲里那条
「RPG 成长通过属性影响公式和条件，不改变系统结构」。

## 五、未定 🚧

- 槽位集合与命名（主手 / 副手 / 护甲 / 饰品？）
- 属性的叠加规则（同名组件怎么合并？**同一组件只能有一个 → 需要"基础值 + 加成"结构**）
- 卸下后物品去哪（地面实体 / 背包 / 直接销毁）
- 耐久与修理
