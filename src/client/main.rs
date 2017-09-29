mod args;
mod connection;
mod slot;

use args::{ArgUnit, Arguments};
use connection::Connection;
use std::collections::{BTreeMap, VecDeque};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

const LOCALHOST: &str = "127.0.0.1:12345";

fn main() {
    let arguments = match Arguments::new() {
        Ok(arguments) => arguments,
        Err(why) => {
            eprintln!("concurr [CRITICAL]: {}", why);
            exit(1);
        }
    };

    eprintln!("[INFO] parsing IP address");
    let mut connection = match Connection::new(LOCALHOST) {
        Ok(connection) => connection,
        Err(why) => {
            eprintln!("[CRITICAL] {}", why);
            exit(1);
        }
    };

    eprintln!("[INFO] found {} cores on {:?}", connection.cores, connection.address);
    eprintln!("[INFO] sending '{}' to {:?}", arguments.get_command(), connection.address);
    let command_id = match connection.send_command(arguments.get_command()) {
        Ok(command_id) => command_id,
        Err(why) => {
            eprintln!("[CRITICAL] {}", why);
            exit(1);
        }
    };

    let inputs = Arc::new(Mutex::new(VecDeque::new()));
    let outputs = Arc::new(Mutex::new(BTreeMap::new()));

    for _ in 0..connection.cores {
        let inputs = inputs.clone();
        let outputs = outputs.clone();
        let address = connection.address;
        thread::spawn(move || slot::spawn(inputs, outputs, address, command_id));
    }

    if arguments.args.len() != 1 {
        unimplemented!()
    }

    let total_inputs = Arc::new(AtomicUsize::new(0));
    let inputs_finished = Arc::new(AtomicBool::new(false));

    {
        let total_inputs = total_inputs.clone();
        let inputs_finished = inputs_finished.clone();
        thread::spawn(move || match arguments.args[0] {
            ArgUnit::Strings(ref vec) => {
                let mut ninputs = 0;
                for (jid, ref input) in vec.iter().enumerate() {
                    let mut inputs = inputs.lock().unwrap();
                    inputs.push_back((jid, String::from(input.as_str())));
                    ninputs += 1;
                }
                total_inputs.store(ninputs, Ordering::SeqCst);
                inputs_finished.store(true, Ordering::SeqCst);
            }
            ArgUnit::Files(_) => unimplemented!(),
        });
    }

    let mut counter = 0;
    while !(inputs_finished.load(Ordering::Relaxed) &&
        counter == total_inputs.load(Ordering::Relaxed))
    {
        thread::sleep(Duration::from_millis(1));
        let mut outputs = outputs.lock().unwrap();
        let (status, stdout, stderr) = match outputs.remove(&counter) {
            Some(output) => {
                drop(outputs);
                output
            }
            None => continue,
        };
        eprintln!("Job: {}; Status: {}\nSTDOUT: {}\nSTDERR: {}", counter, status, stdout, stderr);
        counter += 1;
    }
}
