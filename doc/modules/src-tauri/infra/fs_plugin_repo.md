# FsPluginRepository

实现plugin/registry::PluginRepository。读取代码目录之外的plugins.json，默认无插件；验证目标清单v2/APIv2、最低应用版本、重复ID、入口canonical边界和源码1MiB上限。无关坏清单不阻止有效插件启动。

代码默认plugins/<目录>/，可由管理员codeRoot指定。授权和声明交集后生成LoadedPlugin，不执行JS或自动批准。见[仓库](../plugin/registry/mod.md)。

approval逐项解析验证，错误进入invalid_plugins；list逐项报告，load只拒绝目标错误，无关非法项不阻塞好插件。整体JSON/根字段或codeRoot非法仍拒绝整份配置。

代码目录扫描同样按条目隔离：条目读取失败、损坏或越界的符号链接、缺清单的目录都只记录脱敏诊断并跳过，绝不让无关条目阻断目标插件；目录候选扫描上限128，只有“达到上限仍未找到目标”才返回limit_exceeded。符号链接只按canonical目标判定，越出codeRoot一律跳过。

宿主侧失败调用plugin_diagnostics输出warn：保留操作阶段及安全错误类别，不输出原始错误文本、URL、凭据、绝对路径或业务内容；插件侧PluginError保持原契约。
