//! DTO / Entity 定义（serde 序列化，camelCase 与前端对齐）。

pub mod graph;
pub mod misc;
pub mod project;
pub mod system;
pub mod user;

pub use graph::{Change, EdgeData, Field, FieldChange, Graph, GraphEdge, GraphNode, NodeData, Position, Viewport, Vulnerability};
pub use misc::{AssetType, AuditRecord, PresenceEntry, UploadQuery};
pub use project::{MemberEntry, ProjectRow, ProjectSummary};
pub use user::{DirectoryEntry, PublicUser, User};