//! OpenMind Connectors - 数据源连接器实现
//!
//! 提供多种数据源的Connector实现，每个Connector独立可测试：
//! - `vault`: OpenVault文件同步Connector
//! - `blog`: 博客文章摄入Connector
//! - `bookmark`: 书签导入Connector
//! - `note`: 备忘录同步Connector
//!
//! 所有Connector通过EnhancedConnector trait实现能力声明，
//! 注册到ConnectorRegistry即可被系统自动发现和编排。

pub mod blog;
pub mod bookmark;
pub mod note;
pub mod vault;

pub use blog::BlogConnector;
pub use bookmark::BookmarkConnector;
pub use note::NoteConnector;
pub use vault::VaultConnector;
