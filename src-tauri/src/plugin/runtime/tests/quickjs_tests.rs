use super::*;

#[test]
fn engine_errors_preserve_resource_and_script_categories() {
    assert_eq!(
        engine_error(rquickjs::Error::Allocation),
        PluginError::LimitExceeded
    );
    assert_eq!(
        engine_error(rquickjs::Error::Exception),
        PluginError::ScriptError
    );
}

#[tokio::test]
async fn rejection_tracker_observes_unhandled_promises_and_removes_handled_ones() {
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>("globalThis.pending = Promise.reject('private credential');")
                .unwrap();
        })
        .await;
    assert_eq!(runtime.rejections.lock().unwrap().pending.len(), 1);
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>("pending.catch(() => {});").unwrap();
        })
        .await;
    assert!(runtime.rejections.lock().unwrap().pending.is_empty());
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>("Promise.reject('another secret').catch(() => {});")
                .unwrap();
        })
        .await;
    assert!(runtime.rejections.lock().unwrap().pending.is_empty());
    runtime.shutdown().await;
}

#[tokio::test]
async fn rejection_tracking_has_a_fixed_memory_budget() {
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    runtime
        .context
        .with(|ctx| {
            ctx.eval::<(), _>(
                "globalThis.promises = Array.from({length: 300}, () => Promise.reject('secret'));",
            )
            .unwrap();
        })
        .await;
    {
        let state = runtime.rejections.lock().unwrap();
        assert_eq!(state.pending.len(), 256);
        assert!(state.overflow);
    }
    runtime.report_rejections();
    assert!(runtime.rejections.lock().unwrap().pending.is_empty());
    runtime.shutdown().await;
}

#[tokio::test]
async fn unhandled_rejection_diagnostics_never_format_js_reasons() {
    use std::io::Write;
    #[derive(Clone)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let _sink = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(std::io::sink)
            .finish(),
    );
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = Capture(bytes.clone());
    let dispatch = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish(),
    );
    let mut runtime = QuickJsRuntime::new(CancellationToken::new()).await.unwrap();
    runtime.context.with(|ctx| {
        ctx.eval::<(), _>("Promise.reject({secret: 'PRIVATE_CREDENTIAL', toString() { throw Error('DO_NOT_FORMAT'); }});").unwrap();
    }).await;
    tracing::dispatcher::with_default(&dispatch, || runtime.report_rejections());
    let log = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(log.contains("promise.rejection"));
    assert!(log.contains("unhandled_rejection"));
    assert!(!log.contains("PRIVATE_CREDENTIAL"));
    assert!(!log.contains("DO_NOT_FORMAT"));
    runtime.shutdown().await;
}
