use chashmap::CHashMap;
use concurr::InsertOutput;
use std::fs::File;
use std::io::{self, StdoutLock, Write};
use std::thread;
use std::time::Duration;

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
    pub outputs: CHashMap<usize, Output>,
}

pub enum Output {
    Outcome(u8, OutputSource),
    Failed,
}

impl Outputs {
    /// Appends a new output onto the queue from an external source
    pub fn push_external(&self, id: usize, status: u8, out: String, err: String) {
        // Simply wrap the inputs as an `External` variant.
        let source = OutputSource::External(out, err);
        let output = Output::Outcome(status, source);
        self.outputs.insert(id, output);
    }

    /// Loops until the next output has been found. For each unsuccessful loop, the thread
    /// will wait 1ms before attempting to lock and grab the output again.
    pub fn get(&self, id: &usize) -> Output {
        loop {
            if let Some(element) = self.outputs.remove(id) {
                return element;
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

impl InsertOutput for Outputs {
    /// Appends a new internal output onto the queue.
    fn insert(&self, id: usize, mut result: Option<(u8, File, File)>) {
        let output = match result.take() {
            Some((sts, out, err)) => {
                Output::Outcome(sts, OutputSource::Internal(out, err))
            }
            None => Output::Failed,
        };
        self.outputs.insert(id, output);
    }
}
