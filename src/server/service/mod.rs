mod codec;
mod events;
mod proto;

pub use self::codec::*;
pub use self::events::*;
pub use self::proto::*;

use command::PreparedCommand;
use futures::{future, Future};
use jobs::{slot_event, Job};
use num_cpus;
use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::io::FromRawFd;
use std::str;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;
use tokio_service::Service;

fn obtain(input: &[u8]) -> io::Result<String> {
    str::from_utf8(input)
        .map(String::from)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid UTF-8"))
}

type Jobs = Arc<RwLock<Vec<Option<Job>>>>;

pub struct Concurr {
    commands: Jobs,
}

impl Concurr {
    pub fn new(commands: Jobs) -> Concurr { Concurr { commands } }
}

impl Service for Concurr {
    // These types must match the corresponding protocol types:
    type Request = JobEvent;
    type Response = ResponseEvent;

    // For non-streaming protocols, service errors are always io::Error
    type Error = io::Error;

    // The future for computing the response; box it for simplicity.
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    // Produce a future for computing a response from a request.
    fn call(&self, req: Self::Request) -> Self::Future {
        let event = match req {
            JobEvent::Command(cmd) => {
                let command = PreparedCommand::new(&cmd);
                let inputs = Arc::new(Mutex::new(VecDeque::new()));
                let outputs = Arc::new(Mutex::new(BTreeMap::new()));
                let kill = Arc::new(AtomicBool::new(false));
                let parked = Arc::new(AtomicUsize::new(0));
                let slots = num_cpus::get();

                for slot in 0..slots {
                    let inputs = inputs.clone();
                    let outputs = outputs.clone();
                    let command = command.clone();
                    let kill = kill.clone();
                    let parked = parked.clone();
                    let _ = thread::spawn(
                        move || slot_event(slot, command, inputs, outputs, kill, parked),
                    );
                }

                let mut id = 0;
                let mut commands = self.commands.write().unwrap();
                for cmd in commands.iter_mut() {
                    if cmd.is_none() {
                        *cmd = Some(Job {
                            slots,
                            command,
                            inputs,
                            outputs,
                            kill,
                            parked,
                        });
                        return Box::new(future::ok(ResponseEvent::Info(id.to_string())));
                    }
                    id += 1;
                }

                commands.push(Some(Job {
                    slots,
                    command,
                    inputs,
                    outputs,
                    kill,
                    parked,
                }));
                ResponseEvent::Info(id.to_string())
            }
            JobEvent::Input(cid, jid, input) => {
                {
                    let commands = self.commands.read().unwrap();
                    match commands.get(cid) {
                        Some(&Some(ref unit)) => {
                            let mut inputs = unit.inputs.lock().unwrap();
                            inputs.push_back((jid, input.clone()));
                            drop(inputs);

                            let result = loop {
                                thread::sleep(Duration::from_millis(1));
                                if let Ok(ref mut outputs) = unit.outputs.try_lock() {
                                    if let Some(result) = outputs.remove(&jid) {
                                        break result;
                                    }
                                }
                            };

                            match result {
                                Some((status, stdout, stderr)) => {
                                    let mut stdout = unsafe { File::from_raw_fd(stdout) };
                                    let mut stderr = unsafe { File::from_raw_fd(stderr) };
                                    let mut outbuf = String::new();
                                    let mut errbuf = String::new();
                                    let _ = stdout.read_to_string(&mut outbuf);
                                    let _ = stderr.read_to_string(&mut errbuf);
                                    return Box::new(future::ok(
                                        ResponseEvent::Output(jid, status, outbuf, errbuf),
                                    ));
                                }
                                None => {
                                    eprintln!(
                                        "[CRITICAL] job {} errored with a critical issue",
                                        cid
                                    );
                                }
                            }
                        }
                        _ => eprintln!("[WARN] command ID {} not found", cid),
                    }
                }

                ResponseEvent::Finished(jid, input)
            }
            JobEvent::GetCores => {
                eprintln!("[INFO] received GetCores request");
                ResponseEvent::Info(num_cpus::get().to_string())
            }
            JobEvent::GetCommands => {
                let commands = self.commands.read().unwrap();
                let mut output;
                let mut commands = commands.iter().enumerate();

                loop {
                    match commands.next() {
                        Some((id, &Some(ref cmd))) => {
                            output = format!("{}: {}", id, cmd.command);
                            break;
                        }
                        None => {
                            return Box::new(
                                future::ok(ResponseEvent::Info("no jobs available".into())),
                            )
                        }
                        _ => (),
                    }
                }

                for (id, cmd) in commands {
                    if let Some(ref cmd) = *cmd {
                        output.push_str(&format!("\n{}: {}", id, cmd.command));
                    }
                }

                ResponseEvent::Info(output)
            }
            JobEvent::StopJob(id) => {
                let mut commands = self.commands.write().unwrap();
                // Obtain the corresponding job from the given ID.
                if let Some(command) = commands.get_mut(id) {
                    // We shall signal the threads to quite, and then set this command to None.
                    if let Some(ref unit) = *command {
                        eprintln!("[INFO] removing job {}", id);
                        // Signal to the threads that it's time to come home.
                        unit.kill.store(true, Ordering::Relaxed);
                        // Wait for them to park before resetting the command.
                        while unit.parked.load(Ordering::Relaxed) != unit.slots {
                            thread::sleep(Duration::from_millis(1));
                        }
                    }
                    // Reset the command
                    *command = None;
                }
                ResponseEvent::Info("deleted job".into())
            }
        };

        Box::new(future::ok(event))
    }
}
