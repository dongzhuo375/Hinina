# FsPluginRepository

PluginRepository 端口属于 plugin/registry，不再位于 core。FsPluginRepository 是无状态占位；scan_plugins、load_plugin_code、plugin_dir_exists 全部返回 PluginError::Unsupported，不执行 I/O、不持有 Storage、不 panic，未装配进 AppContext。

后续实现与路径约束见 [仓库契约](../plugin/registry/mod.md)。
