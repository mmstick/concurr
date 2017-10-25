use coco::Stack;
use concurr::InsertJob;

pub struct Inputs {
    pub stack: Stack<(usize, String)>,
}

impl InsertJob for Inputs {
    fn get_job(&self) -> Option<(usize, String)> { self.stack.pop() }

    fn insert_job(&self, id: usize, job: String) { self.stack.push((id, job)); }
}
