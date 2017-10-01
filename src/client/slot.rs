use connection::{attempt_connection, attempt_write};
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufRead, BufReader};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Listen for inputs, pass the inputs along, and store the outputs, serially.
///
/// This function contains the event loop that will run on each spawned slot, on each node.
/// All slots share access to the same `inputs` and `outputs` buffer. Inputs are popped from the
/// `inputs` buffer, and their results are pushed onto the `outputs` buffer.
pub fn spawn(
    inputs: Arc<Mutex<VecDeque<(usize, String)>>>,
    outputs: Arc<Mutex<BTreeMap<usize, (u8, String, String)>>>,
    address: SocketAddr,
    id: usize,
) -> io::Result<()> {
    // Open a TCP stream to the node that will be used to submit inputs.
    let mut stream = attempt_connection(address)?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;

    loop {
        // Grab an input from the shared inputs buffer.
        let (jid, input) = {
            let mut inputs = inputs.lock().unwrap();
            match inputs.pop_front() {
                Some(input) => input,
                None => {
                    thread::sleep(Duration::from_millis(1));
                    continue
                },
            }
        };

        // Generate the instruction that will be submitted based on the received input.
        let instruction = format!("inp {} {} {}\r\n", id, jid, input);
        // Pass the instruction to the server. Attempt 3 times before failing.
        attempt_write(&mut stream, instruction.as_bytes())?;
        // Wait for and read the results that are returned, and place them onto the output
        // buffer after parsing the results.
        if !read_results(BufReader::new(&mut stream), &outputs)? {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid response"));
        }
    }
}

/// Results obtained from an input always consist of precisely three lines. The status line, which
/// contains the job ID and exit status; and the stdout and stderr lines, which have their newlines
/// escaped.
fn read_results(
    buffer: BufReader<&mut TcpStream>,
    outputs: &Arc<Mutex<BTreeMap<usize, (u8, String, String)>>>,
) -> io::Result<bool> {
    // Create a `Lines` iterator that will we will call exactly three times in the future.
    let mut lines = buffer.lines();
    // The first line to read is the status line, containing the job ID and exit status.
    if let Some(status) = lines.next() {
        // Ensure that the status line contained valid UTF-8.
        let status = status?;
        // Find the space, as we are going to split the line at that space.
        let pos = status.find(' ')
            .ok_or(io::Error::new(io::ErrorKind::Other, "invalid status line"))?;
        // Split the status line in two, as there should be a whitespace to separate the results.
        let (id, status) = status.split_at(pos);
        // Then attempt to parse each value as their corresponding integer types.
        let (id, status) = (parse_usize(id)?, parse_u8(&status[1..])?);
        // Now obtain the stdout line, followed by the stderr line.
        if let (Some(stdout), Some(stderr)) = (lines.next(), lines.next()) {
            // Escape the stdout and stderr values, and push them onto the outputs buffer.
            let output = (status, unescape(&stdout?), unescape(&stderr?));
            let mut outputs = outputs.lock().unwrap();
            outputs.insert(id, output);
            // True indicates that we successfully placed a value.
            return Ok(true);
        }
    }

    // False indicates that there was an issue with the results.
    Ok(false)
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
