# 插件宿主资源上限

plugin/limits.rs 是 Runtime、Manager 和查询桥共用的内部常量模块，不向插件开放可调策略。

激活10秒、事件30秒、启动等待为激活期限加2秒、停止5秒。启动截止边界采用fail closed：调用方未及时观察到ready，即使引擎刚完成激活也撤销并记录timeout。

桥响应含JSON包络最多1MiB；单native提交投影页最多1MiB；每次激活全部保留游标余项的JSON字节合计最多4MiB，同时保留原有32游标/1000条native页上限。这些字节预算不等于整个进程的RSS上限。超限统一返回limit_exceeded。
