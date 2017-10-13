extern crate app_dirs;
extern crate concurr;
extern crate native_tls;
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
mod nodes;
mod redirection;
mod slot;

use args::{ArgUnit, ArgsSource, Arguments};
use configure::Config;
use slot::Slot;
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub enum Output {
    Succeeded(String, String),
    Errored(u8, String, String),
    Failed(String),
}

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
    let inputs = Arc::new(Mutex::new(VecDeque::new()));
    let outputs = Arc::new(Mutex::new(BTreeMap::new()));
    let errors = Arc::new(Mutex::new(VecDeque::new()));
    let failed = Arc::new(Mutex::new(BTreeMap::new()));
    let kill = Arc::new(AtomicBool::new(false));
    let parked = Arc::new(AtomicUsize::new(0));

    // Spawn slots for submitting inputs to each connected node.
    for node in &nodes {
        if config.flags & configure::VERBOSE != 0 {
            eprintln!("concurr [INFO]: found {} cores on {:?}", node.cores, node.address);
        }
        let domain = Arc::new(node.domain.clone());
        for _ in 0..node.cores {
            eprintln!("spawned core on node");
            let address = node.address;
            let id = node.command;
            let inputs = inputs.clone();
            let outputs = outputs.clone();
            let errors = errors.clone();
            let failed = failed.clone();
            let kill = kill.clone();
            let parked = parked.clone();
            let domain = domain.clone();
            thread::spawn(move || {
                Slot::new(inputs, outputs, errors, failed, kill, parked, address, id, &domain)
                    .spawn()
            });
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
                let inputs_finished = inputs_finished.clone();
                let ninputs = &mut 0;
                inputs::file(&inputs, &path, ninputs);
                total_inputs.store(*ninputs, Ordering::SeqCst);
                inputs_finished.store(true, Ordering::SeqCst);
            });
        }
        ArgsSource::RedirPipe => {
            let total_inputs = total_inputs.clone();
            let inputs_finished = inputs_finished.clone();
            thread::spawn(move || {
                let ninputs = &mut 0;
                inputs::stdin(&inputs, ninputs);
                total_inputs.store(*ninputs, Ordering::SeqCst);
                inputs_finished.store(true, Ordering::SeqCst);
            });
        }
        ArgsSource::Cli(args) => {
            // This branch will generate permutations of the inputs to use as inputs.
            if args.len() != 1 {
                unimplemented!()
            }

            let total_inputs = total_inputs.clone();
            let inputs_finished = inputs_finished.clone();
            thread::spawn(move || match args[0] {
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
                    let ninputs = &mut 0;
                    for path in vec.iter() {
                        inputs::file(&inputs, &Path::new(path), ninputs);
                    }
                    total_inputs.store(*ninputs, Ordering::SeqCst);
                    inputs_finished.store(true, Ordering::SeqCst);
                }
            });
        }
    }

    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut results = Vec::new();
    let mut counter = 0;
    let start = Instant::now();

    // Wait for inputs to be received, exiting the program once all inputs have been processed.
    while !(inputs_finished.load(Ordering::Relaxed)
        && counter == total_inputs.load(Ordering::Relaxed))
    {
        thread::sleep(Duration::from_millis(1));

        {
            let mut counter = counter;
            loop {
                let mut outputs = outputs.lock().unwrap();
                if let Some((status, out, err)) = outputs.remove(&counter) {
                    results.push(if status == 0 {
                        Output::Succeeded(out, err)
                    } else {
                        Output::Errored(status, out, err)
                    });
                    counter += 1;
                }
                drop(outputs);

                let mut failed = failed.lock().unwrap();
                if let Some(output) = failed.remove(&counter) {
                    results.push(Output::Failed(output));
                    counter += 1;
                }
                drop(failed);

                if !results.is_empty() {
                    break;
                }
                thread::sleep(Duration::from_millis(1));
            }
        }

        for result in results.drain(..) {
            if config.flags & configure::VERBOSE != 0 {
                let _ = stdout.write_all(b"concurr [INFO ");
                let _ = stdout.write_all(counter.to_string().as_bytes());
                let _ = stdout.write_all(b"]\n");
            }
            match result {
                Output::Succeeded(out, err) => {
                    let _ = writeln!(stdout, "concurr [INFO {}]: 0", counter);
                    for line in err.lines() {
                        eprintln!("concurr [ERR {}]: {}", counter, line);
                    }
                    let _ = stdout.write_all(out.as_bytes());
                    let _ = stdout.write(b"\n");
                }
                Output::Errored(status, out, err) => {
                    let _ = writeln!(stdout, "concurr [INFO {}]: {}", counter, status);
                    for line in err.lines() {
                        eprintln!("concurr [ERR {}]: {}", counter, line);
                    }
                    let _ = stdout.write_all(out.as_bytes());
                    let _ = stdout.write(b"\n");
                }
                Output::Failed(input) => {
                    eprintln!("concurr [WARN {}]: failed to execute '{}'", counter, input);
                }
            }
            counter += 1;
        }
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
    let spawned_threads = nodes.into_iter().map(|x| x.cores).sum();
    kill.store(true, Ordering::Relaxed);
    while parked.load(Ordering::Relaxed) != spawned_threads {
        thread::sleep(Duration::from_millis(1));
    }
}
