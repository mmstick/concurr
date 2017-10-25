use concurr::InsertOutput;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, StdoutLock, Write};
use std::sync::Mutex;

/// Enables efficiently handling outputs based on it's source.
pub enum OutputSource {
    /// Outputs created from the internal node are stored within anonymous files.
    Internal(File, File),
    /// Outputs from external nodes, on the other hand, must be buffered to a string.
    External(String, String),
}

impl OutputSource {
    /// Handles writing the input streams to their corresponding output streams.
    pub fn write(&mut self, stdout: &mut StdoutLock) {
        match *self {
            OutputSource::Internal(ref mut out, ref mut err) => {
                let mut stderr = io::stderr();
                let _ = io::copy(out, stdout);
                let _ = io::copy(err, &mut stderr.lock());
            }
            OutputSource::External(ref mut out, ref mut err) => {
                let mut stderr = io::stderr();
                let _ = stdout.write_all(out.as_bytes());
                let _ = stderr.lock().write_all(err.as_bytes());
            }
        }
    }
}

pub struct Outputs {
    pub outputs: Mutex<BTreeMap<usize, Output>>,
}

pub enum Output {
    Succeeded(OutputSource),
    Errored(u8, OutputSource),
    Failed,
}

impl Outputs {
    pub fn push_external(&self, id: usize, status: u8, out: String, err: String) {
        let source = OutputSource::External(out, err);
        let mut lock = self.outputs.lock().unwrap();
        let output =
            if status == 0 { Output::Succeeded(source) } else { Output::Errored(status, source) };
        lock.insert(id, output);
    }

    pub fn remove(&self, id: &usize) -> Output {
        loop {
            let mut lock = self.outputs.lock().unwrap();
            if let Some(element) = lock.remove(id) {
                return element;
            }
        }
    }
}

impl InsertOutput for Outputs {
    fn insert(&self, id: usize, mut result: Option<(u8, File, File)>) {
        let mut lock = self.outputs.lock().unwrap();
        let output = match result.take() {
            Some(result) => {
                let source = OutputSource::Internal(result.1, result.2);
                if result.0 == 0 {
                    Output::Succeeded(source)
                } else {
                    Output::Errored(result.0, source)
                }
            }
            None => Output::Failed,
        };
        lock.insert(id, output);
    }
}
