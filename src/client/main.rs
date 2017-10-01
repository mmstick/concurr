extern crate app_dirs;
#[allow(unused_extern_crates)]
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
use std::io::{self, Write, BufRead, BufReader};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use std::fs::File;

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
            thread::spawn(move || if let Err(why) = slot::spawn(inputs, outputs, address, id) {
                eprintln!("concurr [CRITICAL]: slot error: {}", why);
            });
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
                for ref input in vec.iter() {
                    let mut inputs = inputs.lock().unwrap();
                    inputs.push_back((ninputs, String::from(input.as_str())));
                    ninputs += 1;
                }
                total_inputs.store(ninputs, Ordering::SeqCst);
                inputs_finished.store(true, Ordering::SeqCst);
            }
            ArgUnit::Files(ref vec) => {
                let mut ninputs = 0;
                for path in vec.iter() {
                    let file = match File::open(path.as_str()) {
                        Ok(file) => file,
                        Err(why) => {
                            eprintln!("concurr [CRITICAL]: unable to read inputs from '{}': {}", path, why);
                            continue
                        }
                    };

                    for line in BufReader::new(file).lines() {
                        match line {
                            Ok(input) => {
                                let mut inputs = inputs.lock().unwrap();
                                inputs.push_back((ninputs, String::from(input.as_str())));
                                ninputs += 1;
                            },
                            Err(why) => {
                                eprintln!("concurr [CRITICAL]: unable to read line from '{}': {}", path, why);
                                break
                            }
                        }
                    }
                }
                total_inputs.store(ninputs, Ordering::SeqCst);
                inputs_finished.store(true, Ordering::SeqCst);
            },
        });
    }

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut results = Vec::new();
    let mut counter = 0;

    // Wait for inputs to be received, exiting the program once all inputs have been processed.
    while !(inputs_finished.load(Ordering::Relaxed) &&
        counter == total_inputs.load(Ordering::Relaxed))
    {
        thread::sleep(Duration::from_millis(1));

        {
            let mut counter = counter;
            let mut outputs = outputs.lock().unwrap();
            while let Some(output) = outputs.remove(&counter) {
                results.push(output);
                counter += 1;
            }
        }

        for (status, out, err) in results.drain(..) {
            let _ = writeln!(
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

    // Ensure that the nodes vector lives until the end of the program.
    // This is because dropped nodes will delete their commands.
    drop(nodes);
}
