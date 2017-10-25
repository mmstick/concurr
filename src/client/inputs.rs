use concurr::InsertJob;
use std::collections::VecDeque;
use std::sync::Mutex;

pub struct Inputs {
    pub inputs: Mutex<VecDeque<(usize, String)>>,
}

impl InsertJob for Inputs {
    fn get_job(&self) -> Option<(usize, String)> {
        let mut lock = self.inputs.lock().unwrap();
        lock.pop_front()
    }

    fn insert_job(&self, id: usize, job: String) {
        let mut lock = self.inputs.lock().unwrap();
        lock.push_back((id, job));
    }
}
