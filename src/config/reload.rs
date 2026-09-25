//! **配置热重载**（只在 `hot-reload` feature 下编译）。
//!
//! 监视 `config/actions.ron`：存盘后重新装载数值、更新 [`ActionConfig`] 资源、
//! 并让机制域**重新注册技能定义**——于是不用重编译、也不用重启游戏。
//!
//! ## 为什么与 bevy 的资产热重载分开
//!
//! `config/` 走**普通文件读写**（不是资产管线，见本域模块文档），
//! 所以 bevy 的 `file_watcher` 看不到它。这里自己挂一个
//! [`notify-debouncer-full`](notify_debouncer_full) 监视器，只在开发期启用
//! （依赖树因此不进生产构建）。
//!
//! ## 触发之后发生什么
//!
//! ```text
//! 文件变了 ─▶ 重读 + 解析 ─▶ 换掉 ActionConfig 资源 ─▶ 写一条 ReloadActionConfig
//!                                                        │
//!                          各机制域订阅它，重跑自己的注册系统（读新配置、交新定义）
//! ```
//!
//! 目录侧不用新机制：`RegisterAbility` 按 id 覆盖、**不动菜单顺序**，
//! 各域的注册系统本来就"读配置 → 交定义"（见 `docs/skills.md` 第二节）。
//!
//! **解析失败不 panic**：报错并保留**上一份好配置**（与启动时的失败策略一致，
//! 见 `docs/config.md` 第三节）——热重载最忌讳"改错一个数字整局崩掉"。

#![cfg(feature = "hot-reload")]

use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use bevy::prelude::*;
use notify_debouncer_full::notify::RecommendedWatcher;
use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use super::{ActionConfig, CONFIG_DIR, CONFIG_FILE};

/// 去抖窗口：编辑器存盘会连发好几个事件，等它安静下来再读一次。
const DEBOUNCE: Duration = Duration::from_millis(300);

/// 配置被重新装载了（写：本模块的文件监视器；消费：各机制域的注册系统）。
///
/// 走 `Message`：它是一次"提醒各域刷新"的广播，晚一帧合并也完全没问题
/// （与 `RegisterAbility` 同一条路子）。
#[derive(Message, Debug, Clone, Copy)]
pub struct ReloadActionConfig;

/// 文件监视器的接收端（`Resource`：让系统每帧从通道取一次，避免在回调里碰 `World`）。
///
/// 包一层 `Mutex` 是因为 `std::sync::mpsc::Receiver` **不是 `Sync`**，
/// 而 bevy 的 `Resource` 要求 `Send + Sync`。锁其实是空的——只有这一个系统读它。
#[derive(Resource)]
pub struct ConfigWatch(Mutex<Receiver<()>>);

/// 挂监视器（`PreStartup`：与首次装载同阶段，但排在它之后）。
pub fn setup_config_watcher(mut commands: Commands) {
    let (tx, rx) = channel();
    let mut debouncer = match new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
        if result.is_ok() {
            // 只要"发生了改动"这一个bit；具体读文件在系统里做（那边能碰 World）
            let _ = tx.send(());
        }
    }) {
        Ok(debouncer) => debouncer,
        Err(error) => {
            error!("⚙ 配置热重载监视器没挂上：{error}（改文件仍需重启）");
            return;
        }
    };
    let path = Path::new(CONFIG_DIR);
    if let Err(error) = debouncer.watch(path, RecursiveMode::NonRecursive) {
        error!("⚙ 无法监视 {CONFIG_DIR}/：{error}（改文件仍需重启）");
        return;
    }
    info!("♻ 配置热重载已开启：改 {CONFIG_DIR}/{CONFIG_FILE} 存盘即生效");
    // 监视器必须**活到 App 结束**：句柄一 drop，后台线程就停了
    commands.insert_resource(ConfigWatch(Mutex::new(rx)));
    commands.insert_resource(ConfigWatcher(Some(debouncer)));
}

/// 监视器句柄（只为保命：`Debouncer` 一 drop 监视就停，所以它必须活着）。
///
/// 这个字段**故意没有读者**——它的全部作用就是"别被 drop"。
#[derive(Resource)]
struct ConfigWatcher(#[allow(dead_code)] Option<Debouncer<RecommendedWatcher, RecommendedCache>>);

/// 通道里有动静 → 重新装载配置；解析成功才换资源并广播刷新。
pub fn reload_action_config_system(
    watch: Option<Res<ConfigWatch>>,
    mut commands: Commands,
    mut reloads: MessageWriter<ReloadActionConfig>,
) {
    let Some(watch) = watch else {
        return; // 监视器没挂上（启动时报过错了）
    };
    // 锁只被这一个系统用，绝不会真的竞争；中毒也不该拖垮帧，直接取回
    let Ok(events) = watch.0.lock() else {
        return;
    };
    if events.try_recv().is_err() {
        return; // 这一帧没有改动
    }
    // 可能有多次事件堆积：清空，只读一次文件
    while events.try_recv().is_ok() {}

    apply_reload(&mut commands, &mut reloads);
}

/// 真的去重读文件、换资源、广播——与通道解耦，便于单测（见本文件 `mod tests`）。
fn apply_reload(commands: &mut Commands, reloads: &mut MessageWriter<ReloadActionConfig>) {
    if let Some(config) = resolve(ActionConfig::load_from_disk()) {
        info!("♻ 已重新装载 {CONFIG_DIR}/{CONFIG_FILE}");
        commands.insert_resource(config);
        reloads.write(ReloadActionConfig);
    }
}

/// 读盘结果的**处置策略**（纯函数，便于把三条分支都单测到）：
/// `Some(config)` = 换上新值；`None` = 保留当前值（并说清为什么）。
///
/// 三条分支：
///
/// - 读到且合法 → 换；
/// - 文件不在 → 保留（不是错误，但热重载期间多半是手滑，`warn` 一声）；
/// - 解析失败 → 保留（**改错一个数字不该让正在跑的这局崩掉/变形**，`error` 一声）。
fn resolve(loaded: Result<Option<ActionConfig>, super::ConfigError>) -> Option<ActionConfig> {
    match loaded {
        Ok(Some(config)) => Some(config),
        Ok(None) => {
            warn!("♻ {CONFIG_DIR}/{CONFIG_FILE} 不见了：保留当前数值，等你放回来");
            None
        }
        Err(error) => {
            error!("♻ {error}（保留上一份配置，改好再存盘即可）");
            None
        }
    }
}

/// 插进配置插件（只在 `hot-reload` 下调用）。
pub fn plugin(app: &mut App) {
    app.add_message::<ReloadActionConfig>()
        .add_systems(
            PreStartup,
            setup_config_watcher.after(super::load_action_config_system),
        )
        // 监视端每帧只查一次通道，放 `First` 足够早：本帧余下的注册系统看到的已是新值
        .add_systems(
            First,
            reload_action_config_system.run_if(resource_exists::<ActionConfig>),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 解析失败 → **保留上一份好配置**（返回 `None`，调用方不换资源）。
    ///
    /// 热重载最怕的就是"改错一个数字，整局变形或崩掉"：这里钉住失败分支**不返回
    /// 任何新值**，于是正在跑的那一局继续用旧数值。
    #[test]
    fn a_broken_file_keeps_the_previous_config() {
        let broken = ActionConfig::from_ron("( fireball: (windup: ) )")
            .map(Some)
            .map_err(|error| super::super::ConfigError::Parse(error.to_string()));
        assert!(broken.is_err(), "夹具本身要是一个语法错");
        assert!(resolve(broken).is_none(), "解析失败时不该换上新值");
    }

    /// 文件不在 → 同样保留当前值（不是错误，但也不该把配置清成默认）。
    #[test]
    fn a_missing_file_keeps_the_previous_config() {
        assert!(
            resolve(Ok(None)).is_none(),
            "文件不见了时保留当前数值，而不是退回默认值"
        );
    }

    /// 读到且合法 → 换上新值。
    #[test]
    fn a_good_file_replaces_the_config() {
        let tweaked = ActionConfig {
            melee: super::super::ActionNumbers {
                power: 99,
                ..ActionConfig::default().melee
            },
            ..ActionConfig::default()
        };
        let resolved = resolve(Ok(Some(tweaked))).expect("合法的配置应当被采纳");
        assert_eq!(resolved.melee.power, 99);
    }
}
