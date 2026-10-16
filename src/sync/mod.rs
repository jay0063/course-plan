//! 客户端同步引擎（Phase 2b · 泳道 D2）。
//!
//! - [`core`]：无 UI/线程/文件的轮次状态机（协议 §5 / §6），可用假实现或进程内真实
//!   服务端做确定性测试；
//! - [`worker`]：专用线程 + tokio + reqwest(rustls) 的实现与 [`SyncHandle`]。
//!
//! 真源：`docs/SYNC-PROTOCOL.md` v1.2。

pub mod core;
pub mod worker;

pub use core::{
    ApiError, Clock, FirstLoginPolicy, ProbeSummary, Status, SyncApi, SyncCommand, SyncCore,
    SyncCoreConfig, SyncEvent,
};
pub use worker::SyncHandle;
