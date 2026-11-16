//! The shapes the real core API will need across the C# boundary: an object with
//! sync methods, an async method that takes a while (a stop that folds a CHD),
//! a callback interface the core calls from its own thread (frame ready, state
//! changes), an error enum and a record.

uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CoreError {
    #[error("machine {id} is not running")]
    NotRunning { id: u32 },
}

#[derive(uniffi::Record)]
pub struct MachineInfo {
    pub id: u32,
    pub name: String,
    pub running: bool,
}

#[uniffi::export(with_foreign)]
pub trait Observer: Send + Sync {
    fn frame_ready(&self, id: u32, seq: u64);
    fn state_changed(&self, info: MachineInfo);
}

#[derive(uniffi::Object)]
pub struct Core {
    observer: std::sync::Mutex<Option<std::sync::Arc<dyn Observer>>>,
}

#[uniffi::export(async_runtime = "tokio")]
impl Core {
    #[uniffi::constructor]
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self { observer: Default::default() })
    }

    pub fn machines(&self) -> Vec<MachineInfo> {
        vec![
            MachineInfo { id: 1, name: "Octane2".into(), running: false },
            MachineInfo { id: 2, name: "O2".into(), running: false },
        ]
    }

    pub fn set_observer(&self, o: std::sync::Arc<dyn Observer>) {
        *self.observer.lock().unwrap() = Some(o);
    }

    /// Calls back from a thread the core owns, like a QEMU display thread would.
    pub fn start(&self, id: u32) -> Result<(), CoreError> {
        let o = self.observer.lock().unwrap().clone().ok_or(CoreError::NotRunning { id })?;
        std::thread::spawn(move || {
            o.state_changed(MachineInfo { id, name: "x".into(), running: true });
            for seq in 1..=3 {
                o.frame_ready(id, seq);
            }
        });
        Ok(())
    }

    /// A long operation. In the app this is a stop that can take minutes.
    pub async fn stop(&self, id: u32, ms: u64) -> Result<u64, CoreError> {
        if id == 0 {
            return Err(CoreError::NotRunning { id });
        }
        tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
        Ok(ms)
    }
}
