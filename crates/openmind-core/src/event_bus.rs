//! 事件总线
//!
//! 组件间通过事件总线通信，不直接依赖，方便未来加订阅者。
//! 支持同步和异步事件处理。

use std::sync::Mutex;

/// 事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 知识条目已创建
    EntryCreated(String),
    /// 知识条目已更新
    EntryUpdated(String),
    /// 知识条目已删除
    EntryDeleted(String),
    /// 嵌入已完成
    EmbeddingCompleted(String),
    /// 嵌入失败
    EmbeddingFailed(String),
    /// 同步已开始
    SyncStarted(String),
    /// 同步已完成
    SyncCompleted(String),
    /// 同步失败
    SyncFailed(String),
    /// Connector已注册
    ConnectorRegistered(String),
    /// Connector健康状态变更
    ConnectorHealthChanged(String, bool),
}

impl Event {
    /// 事件名称
    pub fn name(&self) -> &str {
        match self {
            Self::EntryCreated(_) => "entry.created",
            Self::EntryUpdated(_) => "entry.updated",
            Self::EntryDeleted(_) => "entry.deleted",
            Self::EmbeddingCompleted(_) => "embedding.completed",
            Self::EmbeddingFailed(_) => "embedding.failed",
            Self::SyncStarted(_) => "sync.started",
            Self::SyncCompleted(_) => "sync.completed",
            Self::SyncFailed(_) => "sync.failed",
            Self::ConnectorRegistered(_) => "connector.registered",
            Self::ConnectorHealthChanged(_, _) => "connector.health_changed",
        }
    }
}

/// 事件处理器类型
pub type EventHandler = Box<dyn Fn(&Event) + Send + Sync>;

/// 事件总线
///
/// 发布-订阅模式，组件间松耦合通信。
/// 每个组件可订阅感兴趣的事件，发布者无需知道谁在监听。
pub struct EventBus {
    handlers: Mutex<Vec<EventHandler>>,
}

impl EventBus {
    /// 创建空的事件总线
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(Vec::new()),
        }
    }

    /// 订阅事件
    pub fn subscribe(&self, handler: EventHandler) {
        let mut handlers = self.handlers.lock().unwrap();
        handlers.push(handler);
    }

    /// 发布事件
    pub fn publish(&self, event: Event) {
        let handlers = self.handlers.lock().unwrap();
        for handler in handlers.iter() {
            handler(&event);
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_event_bus_publish_subscribe() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = counter.clone();
        bus.subscribe(Box::new(move |_event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));

        bus.publish(Event::EntryCreated("test-1".to_string()));
        bus.publish(Event::EntryDeleted("test-2".to_string()));

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_event_names() {
        assert_eq!(Event::EntryCreated("x".to_string()).name(), "entry.created");
        assert_eq!(Event::SyncCompleted("vault".to_string()).name(), "sync.completed");
    }
}
