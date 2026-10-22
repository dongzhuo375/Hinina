use super::*;
use rquickjs::{
    function::{Async, Func},
    AsyncContext, AsyncRuntime, Module,
};
use std::time::{Duration, Instant};

#[test]
fn quickjs_runtime_implements_the_plugin_runtime_port() {
    fn assert_runtime<T: PluginRuntime>() {}
    assert_runtime::<quickjs::QuickJsRuntime>();
}

#[tokio::test]
async fn real_engine_module_awaits_rust_future_without_ambient_io() {
    let runtime = AsyncRuntime::new().unwrap();
    let context = AsyncContext::full(&runtime).await.unwrap();
    context.async_with(async |ctx| {
        ctx.globals().set("host", Func::from(Async(|| async {
            tokio::time::sleep(Duration::from_millis(1)).await;
            42
        }))).unwrap();
        let (module, promise) = Module::declare(ctx.clone(), "plugin", "export const result = await host();").unwrap().eval().unwrap();
        promise.into_future::<()>().await.unwrap();
        assert_eq!(module.get::<_, i32>("result").unwrap(), 42);
        assert!(ctx.eval::<bool, _>("['process','fetch','require','fs','WebSocket'].every(k => typeof globalThis[k] === 'undefined')").unwrap());
        assert!(Module::declare(ctx.clone(), "bad", "import x from 'fs';").is_err());
    }).await;
}

#[tokio::test]
async fn real_engine_interrupts_even_caught_infinite_loop() {
    let runtime = AsyncRuntime::new().unwrap();
    let start = Instant::now();
    runtime
        .set_interrupt_handler(Some(Box::new(move || {
            start.elapsed() > Duration::from_millis(30)
        })))
        .await;
    let context = AsyncContext::full(&runtime).await.unwrap();
    context
        .with(|ctx| {
            assert!(ctx
                .eval::<(), _>("for (;;) { try { for (;;) {} } catch (_) {} }")
                .is_err())
        })
        .await;
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn real_engine_enforces_memory_limit() {
    let runtime = AsyncRuntime::new().unwrap();
    runtime.set_memory_limit(2 * 1024 * 1024).await;
    let context = AsyncContext::full(&runtime).await.unwrap();
    context
        .with(|ctx| {
            assert!(ctx
                .eval::<(), _>("globalThis.a = new Array(10000000).fill('x');")
                .is_err())
        })
        .await;
}
