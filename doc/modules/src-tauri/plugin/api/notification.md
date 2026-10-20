# notification

info/success/warn/error(message) 需 notification 权限。文本1..512 UTF-8字节，禁止非换行/制表控制字符；每插件10条/分钟。Tauri 输出不存在则 unavailable。

前端经 Bridge → Service → Store → PluginNotifications，用 Vue 文本插值渲染，不使用 v-html；同时最多5条，6秒消失，可手动关闭。投递是易失的用户提示，不是业务成功凭据；前端尚未监听、窗口关闭或丢失事件时不承诺显示。
