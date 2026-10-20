# 插件仓库接口已迁移

PluginRepository 已从 core/repository 移至 [plugin/registry](../../plugin/registry/mod.md)，core 不再引用插件 Manifest。旧 Rust 路径不保留重导出。文件实现仍属于 infra，当前显式返回 Unsupported。
