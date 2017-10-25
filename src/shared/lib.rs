extern crate app_dirs;
extern crate coco;
#[macro_use]
extern crate lazy_static;
extern crate libc;

mod tokenizer;
mod jobs;

pub use self::jobs::{slot_event, Job};
pub use self::tokenizer::{Token, Tokens};
use app_dirs::AppInfo;
use std::fs::File;

pub trait InsertJob {
    fn get_job(&self) -> Option<(usize, String)>;
    fn insert_job(&self, usize, String);
}

pub const APP_INFO: AppInfo = AppInfo {
    name:   "concurr",
    author: "Michael Aaron Murphy",
};

pub trait InsertOutput {
    fn insert(&self, id: usize, result: Option<(u8, File, File)>);
}
