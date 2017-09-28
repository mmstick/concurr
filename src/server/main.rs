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
use std::sync::{Arc, Mutex};
use tokio_proto::TcpServer;

fn main() {
    let cmds = Arc::new(Mutex::new(Vec::new()));
    let addr = "0.0.0.0:12345".parse().unwrap();
    let server = TcpServer::new(ConcurrProto, addr);
    server.serve(move || Ok(Concurr::new(cmds.clone())));
}
