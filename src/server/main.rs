extern crate app_dirs;
extern crate bytes;
extern crate concurr;
extern crate futures;
extern crate ion_shell;
extern crate libc;
extern crate native_tls;
extern crate num_cpus;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_tls;

mod command;
mod jobs;
mod service;

use app_dirs::{get_app_dir, AppDataType};
use concurr::APP_INFO;
use libc::*;
use native_tls::{Pkcs12, TlsAcceptor};
use service::{Concurr, ConcurrProto};
use std::fs::File;
use std::io::Read;
use std::mem;
use std::process::exit;
use std::ptr;
use std::sync::{Arc, RwLock};
use tokio_proto::TcpServer;
use tokio_tls::proto::Server as TlsProto;

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

    let result = get_app_dir(AppDataType::UserConfig, &APP_INFO, "server.pfx").map(|p| {
        File::open(p).and_then(|mut file| {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map(|_| Pkcs12::from_der(&buf, ""))
        })
    });

    let cert = match result {
        Ok(Ok(Ok(cert))) => cert,
        Ok(Ok(Err(why))) => {
            eprintln!("concurr [CRITICAL]: error parsing cert: {}", why);
            exit(1);
        }
        Ok(Err(why)) => {
            eprintln!("concurr [CRITICAL]: error reading cert file: {}", why);
            exit(1);
        }
        Err(why) => {
            eprintln!("concurr [CRITICAL]: invalid app dir path: {}", why);
            exit(1);
        }
    };

    let tls_cx = TlsAcceptor::builder(cert).unwrap().build().unwrap();

    let cmds = Arc::new(RwLock::new(Vec::new()));
    let addr = "0.0.0.0:31514".parse().unwrap();
    let mut server = TcpServer::new(TlsProto::new(ConcurrProto, tls_cx), addr);
    let ncores = num_cpus::get();
    server.threads(ncores + (ncores / 2));
    eprintln!("Launching service on '0.0.0.0:31514'.");
    server.serve(move || Ok(Concurr::new(cmds.clone())));
}
