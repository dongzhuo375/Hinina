/// 事件类别，用于订阅范围控制。
///
/// 订阅者可通过 EventCategory 精确订阅感兴趣的事件类。
/// 订阅 `All` 将收到所有事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventCategory {
    Auth,
    Contest,
    Problem,
    Submission,
    Workspace,
    System,
    All,
}
