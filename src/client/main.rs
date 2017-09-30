extern crate app_dirs;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

mod args;
mod configure;
mod connection;
mod nodes;
mod slot;

use args::{ArgUnit, Arguments};
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

fn main() {
    // Read the configuration file to get a list of nodes to connect to.
    let config = configure::get();

    // Then parse the arguments supplied to the client.
    let arguments = match Arguments::new() {
        Ok(arguments) => arguments,
        Err(why) => {
            eprintln!("concurr [CRITICAL]: {}", why);
            exit(1);
        }
    };

    // Collect a vector of nodes that we will send inputs to, and initialize them with a command.
    let nodes = match nodes::get(&config.nodes, arguments.get_command()) {
        Ok(nodes) => nodes,
        Err(why) => {
            eprintln!("concurr [CRITICAL]: connection error: {}", why);
            exit(1);
        }
    };

    // Input and output queues that will be concurrently accessed across threads.
    let inputs = Arc::new(Mutex::new(VecDeque::new()));
    let outputs = Arc::new(Mutex::new(BTreeMap::new()));

    // Spawn slots for submitting inputs to each connected node.
    for node in &nodes {
        for _ in 0..node.cores {
            let inputs = inputs.clone();
            let outputs = outputs.clone();
            let address = node.address;
            let id = node.command;
            thread::spawn(move || slot::spawn(inputs, outputs, address, id));
        }
    }

    // This branch will generate permutations of the inputs to use as inputs.
    if arguments.args.len() != 1 {
        unimplemented!()
    }

    // Useful for signaling the total number of inputs that are to be expected.
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

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut counter = 0;

    while !(inputs_finished.load(Ordering::Relaxed) &&
        counter == total_inputs.load(Ordering::Relaxed))
    {
        thread::sleep(Duration::from_millis(1));
        let mut outputs = outputs.lock().unwrap();
        let (status, out, err) = match outputs.remove(&counter) {
            Some(output) => {
                drop(outputs);
                output
            }
            None => continue,
        };
        let _ = write!(
            stdout,
            "Job: {}; Status: {}\nSTDOUT: {}\nSTDERR: {}",
            counter,
            status,
            out,
            err
        );
        counter += 1;
    }
}
