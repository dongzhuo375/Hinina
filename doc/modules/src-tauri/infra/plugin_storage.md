# Plugin Storage Infra

JsonPluginStorage使用fs4文件锁和atomicwrites替换持久化，I/O在spawn_blocking。目录来自绑定授权ID的hash，JS只传key/value。CAS整段持锁，成功须已持久化；损坏不覆盖、停止不删除、文件总配额1MiB。见[存储规范](../plugin/api/storage.md)。

每次激活所有操作共用60次/分钟令牌桶，工厂跨激活/重载共用240次/分钟总预算，允许相应初始突发。超限在I/O之前立即返回limit_exceeded；最多16个在途事务，许可随阻塞任务保留到实际结束。set/delete/CAS与读操作均计费，防止重复解析或重写1MiB JSON。

宿主调用超时只会丢弃等待的future，spawn_blocking任务本身不可取消；因此事务持有一个“调用已放弃”令牌，等待future被丢弃即置位，阻塞任务在加锁前、读取后与持久化提交前都复查，避免插件已收到timeout后写入仍落盘。已进入的原子替换无法回滚，仍按“已发出的副作用不可撤回”说明。

宿主侧失败调用plugin_diagnostics输出warn：保留操作阶段及安全错误类别，不输出原始错误文本、URL、凭据、绝对路径或业务内容；插件侧PluginError保持原契约。
