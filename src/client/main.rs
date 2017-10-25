extern crate app_dirs;
extern crate concurr;
extern crate libc;
extern crate native_tls;
extern crate num_cpus;
#[allow(unused_extern_crates)]
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate toml;

mod args;
mod certificate;
mod configure;
mod connection;
mod inputs;
mod outputs;
mod nodes;
mod redirection;
mod slot;
mod source;

use self::inputs::Inputs;
use self::outputs::{Output, Outputs};
use args::{ArgUnit, ArgsSource, Arguments};
use concurr::{slot_event, InsertJob, Tokens};
use configure::Config;
use slot::Slot;
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Instant;

fn main() {
    // Read the configuration file to get a list of nodes to connect to.
    let config = match Config::get() {
        Ok(config) => config,
        Err(why) => {
            eprintln!("concurr [CRITICAL]: {}", why);
            exit(1);
        }
    };

    // Then parse the arguments supplied to the client.
    let arguments = match Arguments::new() {
        Ok(arguments) => arguments,
        Err(why) => {
            eprintln!("concurr [CRITICAL]: {}", why);
            exit(1);
        }
    };

    // Collect a vector of nodes that we will send inputs to, and initialize them with a command.
    let nodes = match nodes::get(config.nodes.into_iter(), arguments.get_command()) {
        Ok(nodes) => nodes,
        Err(why) => {
            eprintln!("concurr [CRITICAL]: connection error: {}", why);
            exit(1);
        }
    };

    // Input and output queues that will be concurrently accessed across threads.
    let slot_id = Arc::new(AtomicUsize::new(0));
    let inputs = Arc::new(Inputs {
        inputs: Mutex::new(VecDeque::new()),
    });
    let outputs = Arc::new(Outputs {
        outputs: Mutex::new(BTreeMap::new()),
    });
    let errors = Arc::new(Mutex::new(VecDeque::new()));
    let failed = Arc::new(Mutex::new(BTreeMap::new()));
    let kill = Arc::new(AtomicBool::new(false));

    // Useful for knowing when to exit the program
    let mut handles = Vec::new();

    // If enabled, the client will also act as a node.
    if config.flags & configure::LOCHOST != 0 {
        let command = Tokens::new(arguments.get_command());
        let parked = Arc::new(AtomicUsize::new(0));
        let cores = num_cpus::get();

        if config.flags & configure::VERBOSE != 0 {
            eprintln!("concurr [INFO]: spawning {} slots in client", cores);
        }

        for _ in 0..cores {
            let command = command.clone();
            let inputs = inputs.clone();
            let outputs = outputs.clone();
            let kill = kill.clone();
            let parked = parked.clone();
            let slot_id = slot_id.clone();
            let handle = thread::spawn(move || {
                slot_event(
                    slot_id.fetch_add(1, Ordering::SeqCst),
                    command,
                    inputs,
                    outputs,
                    kill,
                    parked,
                )
            });
            handles.push(handle);
        }
    }

    // Spawn slots for submitting inputs to each external node.
    for node in &nodes {
        if config.flags & configure::VERBOSE != 0 {
            eprintln!("concurr [INFO]: spawning {} slots on {:?}", node.cores, node.address);
        }
        let domain = Arc::new(node.domain.clone());
        for _ in 0..node.cores {
            let address = node.address;
            let id = node.command;
            let inputs = inputs.clone();
            let outputs = outputs.clone();
            let errors = errors.clone();
            let failed = failed.clone();
            let kill = kill.clone();
            let domain = domain.clone();
            let handle = thread::spawn(move || {
                Slot::new(inputs, outputs, errors, failed, kill, address, id, &domain).spawn()
            });
            handles.push(handle);
        }
    }

    // Useful for signaling the total number of inputs that are to be expected.
    let total_inputs = Arc::new(AtomicUsize::new(0));
    let inputs_finished = Arc::new(AtomicBool::new(false));

    // Pass arguments into the spawned threads, according to the type of arguments that are
    // have been supplied, and where the arguments originate from.
    match arguments.args {
        ArgsSource::RedirFile(path) => {
            let total_inputs = total_inputs.clone();
            let inputs_finished = inputs_finished.clone();
            thread::spawn(move || {
                let ninputs = &mut 0;
                source::file(&inputs, &path, ninputs);
                total_inputs.store(*ninputs, Ordering::SeqCst);
                inputs_finished.store(true, Ordering::SeqCst);
            });
        }
        ArgsSource::RedirPipe => {
            let total_inputs = total_inputs.clone();
            let inputs_finished = inputs_finished.clone();
            thread::spawn(move || {
                let ninputs = &mut 0;
                source::stdin(&inputs, ninputs);
                total_inputs.store(*ninputs, Ordering::SeqCst);
                inputs_finished.store(true, Ordering::SeqCst);
            });
        }
        ArgsSource::Cli(args) => {
            // This branch will generate permutations of the inputs to use as inputs.
            if args.len() != 1 {
                unimplemented!("permutations currently unsupported")
            }

            let total_inputs = total_inputs.clone();
            let inputs_finished = inputs_finished.clone();
            thread::spawn(move || match args[0] {
                ArgUnit::Strings(ref vec) => {
                    let mut ninputs = 0;
                    for ref input in vec.iter() {
                        inputs.insert_job(ninputs, String::from(input.as_str()));
                        ninputs += 1;
                    }
                    total_inputs.store(ninputs, Ordering::SeqCst);
                    inputs_finished.store(true, Ordering::SeqCst);
                }
                ArgUnit::Files(ref vec) => {
                    let ninputs = &mut 0;
                    for path in vec.iter() {
                        source::file(&inputs, &Path::new(path), ninputs);
                    }
                    total_inputs.store(*ninputs, Ordering::SeqCst);
                    inputs_finished.store(true, Ordering::SeqCst);
                }
            });
        }
    }

    let stdout = io::stdout();
    let stdout = &mut stdout.lock();
    let mut counter = 0;
    let start = Instant::now();

    // Wait for inputs to be received, exiting the program once all inputs have been processed.
    while !(inputs_finished.load(Ordering::Relaxed)
        && counter == total_inputs.load(Ordering::Relaxed))
    {
        // The get method blocks until the next output is found.
        match outputs.get(&counter) {
            Output::Outcome(status, mut source) => {
                if config.flags & configure::VERBOSE != 0 {
                    let _ = writeln!(stdout, "\nconcurr [INFO] Job {}: {}", counter, status);
                }
                source.write(stdout);
            }
            Output::Failed => {
                eprintln!("Job {} failed to execute", counter);
            }
        }
        counter += 1;
    }

    let time = Instant::now() - start;

    if config.flags & configure::VERBOSE != 0 {
        eprintln!(
            "concurr [INFO]: processed {} inputs within {}.{}s",
            total_inputs.load(Ordering::Relaxed),
            time.as_secs(),
            time.subsec_nanos() / 1_000_000
        );
    }

    // Stop the threads that are running in the background.
    kill.store(true, Ordering::Relaxed);
    handles.into_iter().for_each(|h| h.join().unwrap());
}
