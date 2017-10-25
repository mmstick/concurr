extern crate app_dirs;
extern crate bytes;
extern crate coco;
extern crate concurr;
extern crate futures;
extern crate libc;
extern crate native_tls;
extern crate num_cpus;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_tls;

mod service;

use app_dirs::{get_app_dir, AppDataType};
use concurr::APP_INFO;
use native_tls::{Pkcs12, TlsAcceptor};
use service::{Concurr, ConcurrProto};
use std::env::args;
use std::fs::File;
use std::io::Read;
use std::process::exit;
use std::sync::{Arc, RwLock};
use std::thread;
use tokio_proto::TcpServer;
use tokio_tls::proto::Server as TlsProto;

fn main() {
    let mut port = 31514;
    let mut args = args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                unimplemented!();
            }
            "-p" | "--port" => match args.next().map(|x| x.parse::<u32>()) {
                Some(Ok(p)) => port = p,
                Some(_) => {
                    eprintln!("concurr [CRITICAL]: invalid port value");
                    exit(1);
                }
                None => {
                    eprintln!("concurr [CRITICAL]: no port value supplied");
                    exit(1);
                }
            },
            _ => (),
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
    let handle = thread::spawn(move || server.serve(move || Ok(Concurr::new(cmds.clone()))));
    handle.join().unwrap();
}
