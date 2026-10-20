use super::*;
use crate::plugin::api::event::PluginEvent;
use std::cell::Cell;
use std::rc::Rc;

// Test-only engine double proves the port accepts !Send runtimes/futures.
struct LocalRuntime {
    calls: Rc<Cell<u32>>,
}

#[async_trait(?Send)]
impl PluginRuntime for LocalRuntime {
    async fn activate(
        &mut self,
        _source: &str,
        _api: Arc<dyn PluginApi>,
        _config: &serde_json::Value,
        _allowed: Vec<String>,
    ) -> PluginResult<()> {
        Ok(())
    }
    async fn deliver_event(&mut self, _event: &PluginEventEnvelope) -> PluginResult<()> {
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
    async fn shutdown(&mut self) -> PluginResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn non_send_runtime_contract_can_be_used_on_local_executor() {
    let calls = Rc::new(Cell::new(0));
    let mut runtime: Box<dyn PluginRuntime> = Box::new(LocalRuntime {
        calls: Rc::clone(&calls),
    });
    runtime
        .deliver_event(&PluginEventEnvelope {
            version: 2,
            sequence: 1,
            occurred_at: 1,
            event: PluginEvent::ResyncRequired { lost: 0 },
        })
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
    runtime.shutdown().await.unwrap();
    assert_eq!(calls.get(), 1);
}
