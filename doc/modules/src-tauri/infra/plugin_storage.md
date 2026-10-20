# Plugin Storage Infra

JsonPluginStorage使用fs4文件锁和atomicwrites替换持久化，I/O在spawn_blocking。目录来自绑定授权ID的hash，JS只传key/value。CAS整段持锁，成功须已持久化；损坏不覆盖、停止不删除、文件总配额1MiB。见[存储规范](../plugin/api/storage.md)。
