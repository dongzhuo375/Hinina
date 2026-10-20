# FsPluginRepository

实现plugin/registry::PluginRepository。读取代码目录之外的plugins.json，默认无插件；验证目标清单v2/APIv2、最低应用版本、重复ID、入口canonical边界和源码1MiB上限。无关坏清单不阻止有效插件启动。

代码默认plugins/<目录>/，可由管理员codeRoot指定。授权和声明交集后生成LoadedPlugin，不执行JS或自动批准。见[仓库](../plugin/registry/mod.md)。
