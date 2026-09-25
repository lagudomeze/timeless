//! 弹药：**远程 / 重击的货币**。
//!
//! ## 为什么和精力分成两条线（`TODO.md` 的「资源分线」）
//!
//! 两条线的**恢复速度不同**，这才是分线的意义：
//!
//! | 池子 | 服务什么 | 恢复 |
//! | :--- | :--- | :--- |
//! | [`Stamina`](super::Stamina) | 防御与机动（翻滚 / 招架 / 冲刺） | 每次重新可决策 **+1**（快） |
//! | `Ammo` | 远程与重击（火球 / 箭矢） | 每 [`AMMO_RECOVER_INTERVAL`] 秒 **+1**（慢） |
//!
//! 同一条池子里做不出这两种手感：伤害手段是"**这一局还能开几炮**"的预算
//! （所以要慢、要攒），而防御手段是"**这一回合还能不能再滚一次**"的即时取舍
//! （所以要快、要跟得上节奏）。
//!
//! **平 A（近战横扫）不花任何资源**，所以"两条线都空了"永远还有事可做——
//! 这条是分线的安全阀，不是顺手的设计。

use bevy::prelude::*;

/// 弹药上限。
pub const AMMO_MAX: u32 = 3;
/// 弹药恢复间隔（虚拟秒）：**比精力的"每次决策 +1"慢得多**。
///
/// 走 `Time<Virtual>`，所以冻结时不回复——暂停不是白送资源的时间。
pub const AMMO_RECOVER_INTERVAL: f32 = 6.0;

/// 弹药槽（挂在单位身上）。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct Ammo {
    pub current: u32,
    pub max: u32,
}

impl Default for Ammo {
    fn default() -> Self {
        Self {
            current: AMMO_MAX,
            max: AMMO_MAX,
        }
    }
}

impl Ammo {
    /// 满弹单位。
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    /// 够不够花。
    pub fn can_afford(&self, cost: u32) -> bool {
        self.current >= cost
    }

    /// 扣费；不够则不扣并返回 `false`（调用方据此放弃这个动作）。
    pub fn try_spend(&mut self, cost: u32) -> bool {
        if !self.can_afford(cost) {
            return false;
        }
        self.current -= cost;
        true
    }

    /// 回复（上限封顶）。
    pub fn regen(&mut self, amount: u32) {
        self.current = (self.current + amount).min(self.max);
    }
}

/// 弹药恢复计时（**跟着单位走**，所以新上场的不会蹭进度，也不会整齐地一起回）。
#[derive(Component, Debug, Clone)]
pub struct AmmoRecoverTimer(pub Timer);

impl Default for AmmoRecoverTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            AMMO_RECOVER_INTERVAL,
            TimerMode::Repeating,
        ))
    }
}

/// 每 [`AMMO_RECOVER_INTERVAL`] 虚拟秒回 1 点（冻结时不回）。
pub fn recover_ammo_system(
    time: Res<Time<Virtual>>,
    mut ammo: Query<(&mut Ammo, &mut AmmoRecoverTimer)>,
) {
    for (mut ammo, mut timer) in &mut ammo {
        if timer.0.tick(time.delta()).just_finished() {
            ammo.regen(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ammo_spends_only_when_affordable() {
        let mut ammo = Ammo::new(2);
        assert!(ammo.try_spend(1));
        assert_eq!(ammo.current, 1);
        assert!(!ammo.try_spend(2), "不够时不该扣费");
        assert_eq!(ammo.current, 1, "失败的花费不改变状态");
    }

    #[test]
    fn ammo_regen_is_capped() {
        let mut ammo = Ammo::new(3);
        ammo.regen(5);
        assert_eq!(ammo.current, 3, "回复不得超过上限");
    }

    /// **弹药回得比精力慢**：这是分线的全部意义。
    ///
    /// 精力的回复点是"每次重新可决策"（跟着动作节奏走，一秒能回好几次），
    /// 而弹药是每 [`AMMO_RECOVER_INTERVAL`] 秒才 1 点——所以伤害手段是"攒出来的"，
    /// 防御手段是"每一手都能用的"。
    #[test]
    fn ammo_recovers_much_slower_than_energy() {
        const {
            assert!(
                AMMO_RECOVER_INTERVAL >= 5.0,
                "弹药间隔太短：那样它和精力就没有手感差别了"
            )
        };
        const {
            assert!(
                AMMO_MAX < 5,
                "弹药上限太高：'这一局还能开几炮'的预算感来自'少而慢'"
            )
        };
    }

    /// 冻结时一分都不回（与 `Focus` 同一条纪律：暂停不是白送资源的时间）。
    #[test]
    fn ammo_recovers_only_while_the_world_runs() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(Update, recover_ammo_system);
        let unit = app
            .world_mut()
            .spawn((
                Ammo {
                    current: 0,
                    max: AMMO_MAX,
                },
                AmmoRecoverTimer::default(),
            ))
            .id();

        app.update();
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Ammo>(unit).unwrap().current,
            0,
            "刚开始世界是冻着的（等玩家决策），不该回弹药"
        );

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        // 6 秒 / 0.1 秒 = 60 帧，多跑几帧保险
        for _ in 0..70 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Ammo>(unit).unwrap().current,
            1,
            "世界走了 {AMMO_RECOVER_INTERVAL} 秒就该回 1 点"
        );
    }
}
