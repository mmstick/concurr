extern crate bytes;
extern crate futures;
extern crate ion_shell;
extern crate libc;
extern crate num_cpus;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;

mod command;
mod jobs;
mod service;

use service::{Concurr, ConcurrProto};
use std::sync::{Arc, RwLock};
use tokio_proto::TcpServer;
use libc::*;
use std::mem;
use std::ptr;

fn main() {
    unsafe {
        setpgid(0, 0);
        let mut sigset = mem::uninitialized::<sigset_t>();
        sigemptyset(&mut sigset as *mut sigset_t);
        sigaddset(&mut sigset as *mut sigset_t, SIGTSTP);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTOU);
        sigaddset(&mut sigset as *mut sigset_t, SIGTTIN);
        sigaddset(&mut sigset as *mut sigset_t, SIGCHLD);
        sigprocmask(SIG_BLOCK, &sigset as *const sigset_t, ptr::null_mut() as *mut sigset_t);
    }
    let cmds = Arc::new(RwLock::new(Vec::new()));
    let addr = "0.0.0.0:12345".parse().unwrap();
    let mut server = TcpServer::new(ConcurrProto, addr);
    server.threads(num_cpus::get() * 2);
    server.serve(move || Ok(Concurr::new(cmds.clone())));
}
