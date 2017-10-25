use concurr::InsertOutput;
use std::collections::BTreeMap;
use std::fs::File;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub struct Outputs {
    pub outputs: Mutex<BTreeMap<usize, Option<(u8, File, File)>>>,
}

impl Outputs {
    pub fn remove(&self, id: &usize) -> Option<(u8, File, File)> {
        loop {
            let mut lock = self.outputs.lock().unwrap();
            match lock.remove(id) {
                Some(element) => {
                    drop(lock);
                    return element;
                }
                None => {
                    drop(lock);
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }
}

impl InsertOutput for Outputs {
    fn insert(&self, id: usize, result: Option<(u8, File, File)>) {
        let mut lock = self.outputs.lock().unwrap();
        lock.insert(id, result);
    }
}
