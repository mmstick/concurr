
use super::{Inputs, Outputs};
use certificate;
use chashmap::CHashMap;
use concurr::InsertJob;
use connection::{attempt_connection, attempt_write};
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub struct Slot<'a> {
    inputs:  Arc<Inputs>,
    outputs: Arc<Outputs>,
    errors:  Arc<Mutex<VecDeque<(usize, String, u8)>>>,
    failed:  Arc<CHashMap<usize, String>>,
    kill:    Arc<AtomicBool>,
    address: SocketAddr,
    id:      usize,
    domain:  &'a str,
}

impl<'a> Slot<'a> {
    pub fn new(
        inputs: Arc<Inputs>,
        outputs: Arc<Outputs>,
        errors: Arc<Mutex<VecDeque<(usize, String, u8)>>>,
        failed: Arc<CHashMap<usize, String>>,
        kill: Arc<AtomicBool>,
        address: SocketAddr,
        id: usize,
        domain: &'a str,
    ) -> Slot<'a> {
        Slot {
            inputs,
            outputs,
            errors,
            failed,
            address,
            id,
            kill,
            domain,
        }
    }

    fn next_input(&self) -> Option<(usize, String, u8)> {
        match self.inputs.get_job() {
            Some((jid, input)) => Some((jid, input, 0u8)),
            None => {
                let mut errors = self.errors.lock().unwrap();
                errors.pop_back()
            }
        }
    }

    /// Listen for inputs, pass the inputs along, and store the outputs, serially.
    ///
    /// This function contains the event loop that will run on each spawned slot, on each node.
    /// All slots share access to the same `inputs` and `outputs` buffer. Inputs are popped
    /// from the
    /// `inputs` buffer, and their results are pushed onto the `outputs` buffer.
    pub fn spawn(&self) {
        loop {
            // Open a TCP stream to the node that will be used to submit inputs.
            let stream = &mut match attempt_connection(
                self.address,
                self.domain,
                certificate::get(self.domain),
            ) {
                Ok(stream) => stream,
                Err(why) => {
                    eprintln!("concurr [CRITICAL]: connection failed: {}", why);
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };

            // A cache for eliminating heap allocations within the slot.
            let mut cache = ResultsCache::new();

            // Attempt to grab inputs from the inputs buffer until a kill signal is given.
            while !self.kill.load(Ordering::Relaxed) {
                // Grab an input from the shared inputs buffer.
                let (jid, input, tries) = match self.next_input() {
                    Some(input) => input,
                    None => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                };

                // Generate the instruction that will be submitted based on the received input,
                // and then write that instruction into the TcpStream.
                let result = cache.write_instruction(stream, self.id, jid, &input)
                    // Then wait for and return the results of the input, if possible.
                    .and_then(|_| read_results(stream, &self.outputs, &mut cache));

                // Clear the cache so that the next input will have a clean slate.
                cache.clear();

                // If an error occured, append it back to the input list for another slot to
                // attempt.
                if let Err(why) = result {
                    eprintln!("concurr [CRITICAL]: slot error: {}", why);
                    if tries == 3 {
                        self.failed.insert(jid, input);
                    } else {
                        let mut errors = self.errors.lock().unwrap();
                        errors.push_back((jid, input, tries + 1));
                    }
                    thread::sleep(Duration::from_secs(1));
                }
            }
            break;
        }
    }
}

/// Results obtained from an input always consist of precisely three lines. The status line, which
/// contains the job ID and exit status; and the stdout and stderr lines, which have their newlines
/// escaped.
fn read_results<STREAM: Read>(
    stream: &mut STREAM,
    outputs: &Arc<Outputs>,
    cache: &mut ResultsCache,
) -> io::Result<()> {
    let buffer = BufReader::new(stream);
    // Read the results that were returned from the node.
    cache.read_from(buffer)?;
    // Attempt to parse the status line that was read.
    let (id, status) = cache.parse_status()?;
    // Push them onto the outputs buffer.
    outputs.push_external(id, status, unescape(&cache.stdout), unescape(&cache.stderr));
    Ok(())
}

struct ResultsCache {
    instruction: Vec<u8>,
    status:      String,
    stdout:      String,
    stderr:      String,
}

impl ResultsCache {
    pub fn new() -> ResultsCache {
        ResultsCache {
            instruction: Vec::new(),
            status:      String::new(),
            stdout:      String::new(),
            stderr:      String::new(),
        }
    }

    pub fn read_from<STREAM: Read>(
        &mut self,
        mut buffer: BufReader<&mut STREAM>,
    ) -> io::Result<()> {
        // The first line to read is the status line, containing the job ID and exit status.
        let _ = buffer.read_line(&mut self.status)?;
        // The second line contains the stdout stream.
        let _ = buffer.read_line(&mut self.stdout)?;
        // The third line contains the stderr stream.
        let _ = buffer.read_line(&mut self.stderr)?;

        // Remove the additional newlines that were also recorded.
        let _ = self.status.pop();
        let _ = self.stdout.pop();
        let _ = self.stderr.pop();

        Ok(())
    }

    pub fn write_instruction<W: Write>(
        &mut self,
        stream: &mut W,
        cid: usize,
        jid: usize,
        input: &str,
    ) -> io::Result<()> {
        // Build the instruction
        self.instruction.extend_from_slice(b"inp ");
        self.instruction.extend_from_slice(&cid.to_string().as_bytes());
        self.instruction.push(b' ');
        self.instruction.extend_from_slice(&jid.to_string().as_bytes());
        self.instruction.push(b' ');
        self.instruction.extend_from_slice(input.as_bytes());
        self.instruction.extend_from_slice(b"\r\n");

        // Pass the instruction to the server. Attempt 3 times before failing.
        attempt_write(stream, &self.instruction)?;

        // Now clear the instruction
        self.instruction.clear();
        Ok(())
    }

    pub fn parse_status(&self) -> io::Result<(usize, u8)> {
        // Find the space, as we are going to split the results of the status line.
        let pos = self.status
            .find(' ')
            .ok_or(io::Error::new(io::ErrorKind::Other, "invalid status line"))?;
        // Split the status line in two, as there should be a whitespace to separate the results.
        let (id, status) = self.status.split_at(pos);
        // Then attempt to parse each value as their corresponding integer types.
        Ok((parse_usize(id)?, parse_u8(&status[1..])?))
    }

    pub fn clear(&mut self) {
        self.status.clear();
        self.stdout.clear();
        self.stderr.clear();
    }
}

fn parse_u8(input: &str) -> io::Result<u8> {
    input.parse::<u8>().map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
}

fn parse_usize(input: &str) -> io::Result<usize> {
    input.parse::<usize>().map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
}

fn unescape(input: &str) -> String {
    let mut start = 0;
    let mut string = String::with_capacity(input.len());
    let mut chars = input.char_indices();
    while let Some((id, character)) = chars.next() {
        if character == '\\' {
            if let Some((_, nchar)) = chars.next() {
                match nchar {
                    '\\' => {
                        string.push_str(&input[start..id + 1]);
                        start = id + 2;
                    }
                    'n' => {
                        string.push_str(&input[start..id]);
                        string.push('\n');
                        start = id + 2;
                    }
                    _ => (),
                }
            }
        }
    }

    if start != input.len() {
        string.push_str(&input[start..]);
    }
    string
}
