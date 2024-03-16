use futures::{
    executor::{block_on, ThreadPool},
    future::RemoteHandle,
    prelude::*,
    task::SpawnExt,
};
use once_cell::sync::Lazy;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::Semaphore;

static POOL: Lazy<ThreadPool> = Lazy::new(|| ThreadPool::new().unwrap());

macro_rules! error {
    ($fmt:tt $($tt:tt)*) => {
        PyValueError::new_err(format!($fmt $($tt)*))
    };
}

/// A Python module implemented in Rust.
#[pymodule]
#[pyo3(name = "easyflow")]
fn dataflow(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(load_dataflow, m)?)?;
    m.add_class::<Dataflow>()?;
    m.add_class::<Sender>()?;
    m.add_class::<Listener>()?;
    Ok(())
}

#[pyfunction]
pub fn load_dataflow(path: PathBuf) -> PyResult<Dataflow> {
    let graph = easyflow::Dataflow::open(&path)
        .map_err(|err| error!("Unable to open '{}': {err}", path.display()))?;

    Ok(Dataflow { graph })
}

#[pyclass]
pub struct Dataflow {
    graph: easyflow::Dataflow,
}

#[pymethods]
impl Dataflow {
    pub fn build_sender(&self, node: &str) -> PyResult<Sender> {
        let sender = block_on(async move {
            self.graph
                .build_sender(node)
                .await
                .map_err(|err| error!("Unable to create sender for '{node}' node: {err}"))
        })?;
        Ok(Sender { sender })
    }

    pub fn build_sender_to(&self, node: &str, to: &str) -> PyResult<Sender> {
        let sender = block_on(async move {
            self.graph
                .build_sender_to(node, to)
                .await
                .map_err(|err| error!("Unable to create sender for '{node}' node: {err}"))
        })?;
        Ok(Sender { sender })
    }

    pub fn listen(&self, node: &str, callback: PyObject) -> PyResult<Listener> {
        let receiver = block_on(async move {
            self.graph
                .build_receiver(node)
                .await
                .map_err(|err| error!("Unable to create sender for '{node}' node: {err}"))
        })?;
        build_listener(node.to_string(), receiver, callback)
    }

    pub fn listen_from(&self, node: &str, from: &str, listener: PyObject) -> PyResult<Listener> {
        let receiver = block_on(async move {
            self.graph
                .build_receiver_from(node, from)
                .await
                .map_err(|err| error!("Unable to create sender for '{node}' node: {err}"))
        })?;
        build_listener(node.to_string(), receiver, listener)
    }
}

#[pyclass]
pub struct Sender {
    sender: easyflow_link::Sender,
}

#[pymethods]
impl Sender {
    pub fn send(&mut self, payload: &[u8]) -> PyResult<()> {
        block_on(async move {
            self.sender
                .send(payload)
                .await
                .map_err(|err| error!("{err}"))
        })?;

        Ok(())
    }
}

#[pyclass]
pub struct Listener {
    handle: Option<RemoteHandle<PyResult<()>>>,
    semaphore: Arc<Semaphore>,
    closed: Arc<AtomicBool>,
}

#[pymethods]
impl Listener {
    pub fn terminate(&self) {
        self.semaphore.add_permits(1);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn wait(&mut self) -> PyResult<()> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        block_on(handle)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn build_listener(
    node: String,
    mut receiver: easyflow_link::Receiver,
    callback: PyObject,
) -> PyResult<Listener> {
    let semaphore = Arc::new(Semaphore::new(0));
    let closed = Arc::new(AtomicBool::new(false));

    let task = {
        let semaphore = semaphore.clone();
        let closed = closed.clone();

        async move {
            let result = loop {
                let payload = futures::select! {
                    result = receiver.recv().fuse() => {
                        result.map_err(|err| error!("Fail to receive a message on node '{node}': {err}"))?
                    }
                    _ = semaphore.acquire().fuse() => {
                        break Ok(());
                    }
                };
                let Some(payload) = payload else {
                    break Ok(());
                };

                let result = Python::with_gil(|py| {
                    let payload = PyBytes::new(py, &payload);
                    callback.call1(py, (payload,))
                });

                if let Err(err) = result {
                    break Err(err);
                }
            };

            closed.store(true, Ordering::Release);

            if result.is_err() {
                eprintln!(
                    "Error occurred on node '{node}'. Call listener.wait() to get the error."
                );
            }

            result
        }
    };
    let handle = POOL.spawn_with_handle(task).unwrap();

    Ok(Listener {
        handle: Some(handle),
        semaphore,
        closed,
    })
}
