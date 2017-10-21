extern crate app_dirs;
extern crate bytes;
extern crate concurr;
extern crate coco;
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
mod signals;

use app_dirs::{get_app_dir, AppDataType};
use concurr::APP_INFO;
use libc::*;
use native_tls::{Pkcs12, TlsAcceptor};
use service::{Concurr, ConcurrProto};
use std::fs::File;
use std::io::Read;
use std::process::exit;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use tokio_proto::TcpServer;
use tokio_tls::proto::Server as TlsProto;
use std::thread;
use std::time::Duration;
use std::env::args;

pub static PENDING: AtomicUsize = ATOMIC_USIZE_INIT;

extern "C" fn handler(signal: i32) {
    PENDING.fetch_or(1 << signal, Ordering::SeqCst);
}

fn main() {
    unsafe {
        setpgid(0, 0);
        signals::block();
        signals::signal(libc::SIGINT, handler).unwrap();
        signals::signal(libc::SIGTERM, handler).unwrap();
        signals::signal(libc::SIGHUP, handler).unwrap();
    }

    let mut port = 31514;
    let mut args = args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                unimplemented!();
            },
            "-p" | "--port" => match args.next().map(|x| x.parse::<u32>()) {
                Some(Ok(p)) => port = p,
                Some(_) => {
                    eprintln!("concurr [CRITICAL]: invalid port value");
                    exit(1);
                },
                None => {
                    eprintln!("concurr [CRITICAL]: no port value supplied");
                    exit(1);
                }
            }
            _ => ()
        }
    }

    let result = get_app_dir(AppDataType::UserConfig, &APP_INFO, "server.pfx").map(|p| {
        File::open(p).and_then(|mut file| {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map(|_| Pkcs12::from_der(&buf, ""))
        })
    });

    // Attempt to parse the certificate file necessary for encrypting traffic to the server.
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

    let address = format!("0.0.0.0:{}", port);
    let tls_cx = TlsAcceptor::builder(cert).unwrap().build().unwrap();
    let cmds = Arc::new(RwLock::new(Vec::new()));
    let addr = address.parse().unwrap();
    let mut server = TcpServer::new(TlsProto::new(ConcurrProto, tls_cx), addr);
    let ncores = num_cpus::get();
    server.threads(ncores + (ncores / 2));
    eprintln!("Launching service on '{}'.", address);
    thread::spawn(move || server.serve(move || Ok(Concurr::new(cmds.clone()))));

    loop {
        thread::sleep(Duration::from_millis(1000));
        for &signal in &[libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            if PENDING.fetch_and(!(1 << signal), Ordering::SeqCst) & (1 << signal) == 1 << signal {
                exit(1)
            }
        }
    }
}
