//! 行动实体与行动者的标记组件。
//!
//! 「这行动是什么」由载荷组件决定（`MoveAction` / `FireballAction` / `MeleeAction`…），
//! 调度数据在 [`ScheduledAction`](super::ScheduledAction)，行动者的三阶段在
//! [`DecisionSlot`](super::DecisionSlot)——本文件只放两者都要读的**规则标记**。

use bevy::prelude::*;

/// 「这条行动不给撤」。
///
/// 撤销的**代价**不在这里：花了什么、退多少、收多少手续费，都由花钱的那个领域
/// 订阅 [`ActionCancelled`](super::ActionCancelled) 自己算——行动实体上只留
/// "能不能撤"这一条规则。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Uncancellable;

/// 「这个单位的决策来自玩家输入」。
///
/// 时间线（等谁决策、冻结世界）与反应系统（谁被威胁）都只认这个标记，
/// 不再到处 `find(|faction| faction == Faction::Player)`：
/// `Faction` 管**战斗目标过滤**，`InputDriven` 管**输入归属**，两者语义不同。
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputDriven;
