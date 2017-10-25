use super::{InsertJob, InsertOutput, Token, Tokens};
use libc::{self, close, dup2};
use std::env;
use std::fs::File;
use std::os::unix::io::FromRawFd;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

const STDOUT_FILENO: i32 = libc::STDOUT_FILENO;
const STDERR_FILENO: i32 = libc::STDERR_FILENO;

lazy_static! {
    /// On non-Windows systems, the `SHELL` environment variable will be used to determine the
    /// preferred shell of choice for execution. Windows will simply use `cmd`.
    static ref COMMAND: (String, &'static str) = if cfg!(target_os = "windows") {
        ("cmd".into(), "/C")
    } else {
        (env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()), "-c")
    };
}

#[derive(Clone)]
pub struct Job<INPUTS: InsertJob, OUTPUTS: InsertOutput> {
    pub slots:   usize,
    pub command: Tokens,
    pub inputs:  Arc<INPUTS>,
    pub outputs: Arc<OUTPUTS>,
    pub kill:    Arc<AtomicBool>,
    pub parked:  Arc<AtomicUsize>,
}

pub fn slot_event<INPUTS: InsertJob, OUTPUTS: InsertOutput>(
    sid: usize,
    command: Tokens,
    inputs: Arc<INPUTS>,
    outputs: Arc<OUTPUTS>,
    kill: Arc<AtomicBool>,
    parked: Arc<AtomicUsize>,
) {
    let mut buffer = String::new();

    while kill.load(Ordering::Relaxed) != true {
        buffer.clear();
        thread::sleep(Duration::from_millis(1));
        if let Some((jid, input)) = inputs.get_job() {
            for token in &command.tokens {
                match *token {
                    Token::Placeholder => buffer.push_str(&input),
                    Token::Slot => buffer.push_str(&sid.to_string()),
                    Token::Job => buffer.push_str(&jid.to_string()),
                    Token::Text(ref text) => buffer.push_str(text),
                }
            }

            let mut stdout_fds = [0; 2];
            let mut stderr_fds = [0; 2];

            unsafe {
                let _ = libc::pipe(stdout_fds.as_mut_ptr());
                let _ = libc::pipe(stderr_fds.as_mut_ptr());
            }

            // Spawn a shell with the supplied command.
            let cmd = Command::new(COMMAND.0.as_str())
                .arg(COMMAND.1)
                .arg(&buffer)
                // Configure the pipes accordingly in the child.
                .before_exec(move || unsafe {
                    // Redirect the child's std{out,err} to the write ends of our pipe.
                    dup2(stdout_fds[1], STDOUT_FILENO);
                    dup2(stderr_fds[1], STDERR_FILENO);

                    // Close all the fds we created here, so EOF will be sent when the program exits.
                    close(stdout_fds[0]);
                    close(stdout_fds[1]);
                    close(stderr_fds[0]);
                    close(stderr_fds[1]);

                    Ok(())
                })
                .spawn();

            let (mut pout, mut perr) = unsafe {
                // Close the write ends of the pipes in the parent
                libc::close(stdout_fds[1]);
                libc::close(stderr_fds[1]);
                (
                    // But create files from the read ends.
                    File::from_raw_fd(stdout_fds[0]),
                    File::from_raw_fd(stderr_fds[0]),
                )
            };

            match cmd {
                Ok(mut child) => {
                    let status = child.wait().ok().map_or(1, |e| e.code().unwrap_or(1)) as u8;
                    outputs.insert(jid, Some((status, pout, perr)));
                }
                Err(why) => {
                    eprintln!("[CRITICAL] {}", why);
                    outputs.insert(jid, None);
                }
            }
        }
    }
    parked.fetch_add(1, Ordering::Relaxed);
}
