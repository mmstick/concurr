use super::super::command::{PreparedCommand, Token};
use ion_shell::{Builtin, Shell};
use ion_shell::shell::library::IonLibrary;
use libc;
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::os::unix::io::RawFd;
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

const STDOUT_FILENO: i32 = libc::STDOUT_FILENO;
const STDERR_FILENO: i32 = libc::STDERR_FILENO;

pub fn slot_event(
    sid: usize,
    command: PreparedCommand,
    inputs: Arc<Mutex<VecDeque<(usize, String)>>>,
    outputs: Arc<Mutex<BTreeMap<usize, Option<(u8, RawFd, RawFd)>>>>,
    kill: Arc<AtomicBool>,
    parked: Arc<AtomicUsize>,
) {
    eprintln!("[INFO] spawning slot {} for {}", sid, command);

    let builtins = Builtin::map();

    let mut shell = Shell::new(&builtins);
    let mut buffer = String::new();

    while kill.load(Ordering::Relaxed) != true {
        buffer.clear();
        thread::sleep(Duration::from_millis(1));
        let mut lock = inputs.lock().unwrap();
        if let Some((jid, input)) = lock.pop_front() {
            drop(lock);
            for token in &command.tokens {
                match *token {
                    Token::Placeholder => buffer.push_str(&input),
                    Token::Slot => buffer.push_str(&sid.to_string()),
                    Token::Job => buffer.push_str(&jid.to_string()),
                    Token::Text(ref text) => buffer.push_str(text),
                }
            }
            eprintln!("[INFO] slot {}: {}", sid, buffer);

            let mut stdout_fds = [0; 2];
            let mut stderr_fds = [0; 2];

            unsafe {
                let _ = libc::pipe(stdout_fds.as_mut_ptr());
                let _ = libc::pipe(stderr_fds.as_mut_ptr());
            }

            match unsafe { libc::fork() } {
                -1 => eprintln!("[CRITICAL] could not execute fork"),
                0 => {
                    unsafe {
                        let _ = libc::setpgid(0, 0);
                        let _ = libc::dup2(stdout_fds[1], STDOUT_FILENO);
                        let _ = libc::dup2(stderr_fds[1], STDERR_FILENO);
                        let _ = libc::close(stdout_fds[0]);
                        let _ = libc::close(stderr_fds[0]);
                        let _ = libc::close(stdout_fds[1]);
                        let _ = libc::close(stderr_fds[1]);
                    }
                    exit(shell.execute_command(&buffer));
                }
                pid @ _ => {
                    unsafe {
                        let _ = libc::close(stdout_fds[1]);
                        let _ = libc::close(stderr_fds[1]);
                    }
                    match wait(pid as libc::pid_t) {
                        Ok(exit_status) => {
                            let mut outputs = outputs.lock().unwrap();
                            outputs.insert(jid, Some((exit_status, stdout_fds[0], stderr_fds[0])));
                            drop(outputs);
                            continue;
                        }
                        Err(err) => {
                            eprintln!("[CRITICAL] {}", err);
                        }
                    }
                }
            }

            let mut outputs = outputs.lock().unwrap();
            outputs.insert(jid, None);
        }
    }
    parked.fetch_add(1, Ordering::Relaxed);
    eprintln!("[INFO] slot {} is dimissed for {}", sid, command);
}

fn wait(pid: libc::pid_t) -> io::Result<u8> {
    unsafe {
        let mut status = 0;
        if libc::waitpid(pid, &mut status, 0) == -1 {
            return Err(io::Error::new(io::ErrorKind::Other, "could not wait on task process"));
        }

        Ok(libc::WEXITSTATUS(status) as u8)
    }
}
