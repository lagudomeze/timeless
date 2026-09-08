//! # 玩家控制模块
//!
//! 遵循「输入只翻译、不执行」：键盘把操作翻译成 [`MoveCommand`] 消息，
//! 由消费系统设置实体 `Velocity`；方向/速度逻辑集中在 ECS 数据与消息里。
pub mod components;
pub mod input;
pub mod messages;
pub mod systems;
pub use components::MoveSpeed;
pub use input::player_move_input_system;
pub use messages::MoveCommand;
pub use systems::apply_move_command_system;
