use chashmap::CHashMap;
use concurr::InsertOutput;
use std::fs::File;
use std::thread;
use std::time::Duration;

pub struct Outputs {
    pub outputs: CHashMap<usize, Option<(u8, File, File)>>,
}

impl Outputs {
    pub fn remove(&self, id: &usize) -> Option<(u8, File, File)> {
        loop {
            match self.outputs.remove(id) {
                Some(element) => {
                    return element;
                }
                None => {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }
}

impl InsertOutput for Outputs {
    fn insert(&self, id: usize, result: Option<(u8, File, File)>) {
        self.outputs.insert(id, result);
    }
}
