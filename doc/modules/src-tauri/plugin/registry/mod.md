# 插件仓库端口

PluginRepository 从 core/repository 移入 plugin/registry，消除 core 对插件 Manifest 的反向依赖。scan_plugins 返回清单列表，load_plugin_code 接收已选清单，plugin_dir_exists 返回 PluginResult<bool>。

当前只有 trait，FsPluginRepository 的全部方法返回 Unsupported，没有目录扫描、JS 读取或注册管理。AppContext 不装配这个占位仓库。

后续文件实现归 infra：限制根目录、校验清单和入口存在性/canonical 边界（符号链接/Windows junction）、拒绝重复 ID、有限读取（建议源码上限 1 MiB）。单插件无效时诊断后跳过，不能阻止核心应用启动。目录服务故障返回安全错误，不能以空列表假装成功。当前无 PluginManager，也没有第二套事件注册中心。
