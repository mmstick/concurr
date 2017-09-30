mod slot;

pub use self::slot::*;
use super::command::PreparedCommand;
use std::collections::{BTreeMap, VecDeque};
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize};

#[derive(Clone)]
pub struct Job {
    pub slots:   usize,
    pub command: PreparedCommand,
    pub inputs:  Arc<Mutex<VecDeque<(usize, String)>>>,
    pub outputs: Arc<Mutex<BTreeMap<usize, Option<(u8, RawFd, RawFd)>>>>,
    pub kill:    Arc<AtomicBool>,
    pub parked:  Arc<AtomicUsize>,
}
