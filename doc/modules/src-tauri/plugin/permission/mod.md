# Plugin Permission

PluginGrant不可变快照来自宿主批准权限与清单声明的交集。清单不能自动授权；每个API和领域事件独立验证。运行中的授权不跟随磁盘配置静默扩张；编辑plugins.json后需stop/reload生效。

事件订阅需要events.subscribe；submission.*需submission.read，contest.*需contest.read，identity.current需identity.read。storage读写分别验证，CAS需两者；network.request还需要精确origin/地址批准；notification独立授权。Resync是强制控制通知。详见[API](../api/mod.md)。
