# Plugin Storage Infra

JsonPluginStorage使用fs4文件锁和atomicwrites替换持久化，I/O在spawn_blocking。目录来自绑定授权ID的hash，JS只传key/value。CAS整段持锁，成功须已持久化；损坏不覆盖、停止不删除、文件总配额1MiB。见[存储规范](../plugin/api/storage.md)。

每次激活所有操作共用60次/分钟令牌桶，工厂跨激活/重载共用240次/分钟总预算，允许相应初始突发。超限在I/O之前立即返回limit_exceeded；最多16个在途事务，许可随阻塞任务保留到实际结束。set/delete/CAS与读操作均计费，防止重复解析或重写1MiB JSON。
