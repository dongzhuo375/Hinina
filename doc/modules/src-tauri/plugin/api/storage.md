# Plugin Storage

JSON 文件位于 `<base_dir>/plugin-data/<插件ID的SHA-256>/storage.json`，与插件代码分开；键只是数据字符串，不能指定文件路径。每次调用独立检查宿主授权和停止状态。

`get(key)` 返回 `{state:"missing"}` 或 `{state:"value",value:...}`，JSON null 与不存在明确区分。set/delete 需 storage.write；compareAndSet(key, expected, value) 同时需读写权限，expected 使用同一带 state 的类型。

键 1..256 UTF-8 字节，无控制字符；value JSON 编码 <=64KiB；完整文件编码（含 key）<=1MiB。独立 storage.lock 经 fs4 跨实例锁定，读取→比较→原子持久化均在锁内。CAS 只有 atomicwrites 成功后才返回 true；锁等待最多5秒，可被停止取消；阻塞文件操作放在 spawn_blocking。

损坏、不可读或异常超大文件报错并保留原件，不静默重置；停止、卸载、重载不删数据。宿主停止前已开始的原子写可能完成，不能保证回滚。CAS 只保证本地 JSON 事务，不代表远端 exactly-once。
