# FsPluginRepository

实现plugin/registry::PluginRepository。读取代码目录之外的plugins.json，默认无插件；验证目标清单v2/APIv2、最低应用版本、重复ID、入口canonical边界和源码1MiB上限。无关坏清单不阻止有效插件启动。

代码默认plugins/<目录>/，可由管理员codeRoot指定。授权和声明交集后生成LoadedPlugin，不执行JS或自动批准。见[仓库](../plugin/registry/mod.md)。

approval逐项解析验证，错误进入invalid_plugins；list逐项报告，load只拒绝目标错误，无关非法项不阻塞好插件。整体JSON/根字段或codeRoot非法仍拒绝整份配置。
