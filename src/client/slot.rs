use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, Shutdown};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn spawn(
    inputs: Arc<Mutex<VecDeque<(usize, String)>>>,
    outputs: Arc<Mutex<BTreeMap<usize, (u8, String, String)>>>,
    address: SocketAddr,
    id: usize,
) -> io::Result<()> {
    loop {
        thread::sleep(Duration::from_millis(1));
        let mut inputs = inputs.lock().unwrap();
        let (jid, input) = match inputs.pop_front() {
            Some(input) => {
                drop(inputs);
                input
            }
            None => continue,
        };

        eprintln!("[INFO] sending {}", input);
        let mut stream = TcpStream::connect(address)?;
        stream.write_all(format!("inp {} {} {}\r\n", id, jid, input).as_bytes())?;
        stream.shutdown(Shutdown::Write)?;

        let mut lines = BufReader::new(stream).lines();
        if let Some(status) = lines.next() {
            let status = status?;
            let mut elements = status.split_whitespace();
            let (id, status) = match (elements.next(), elements.next()) {
                (Some(id), Some(status)) => (parse_usize(id)?, parse_u8(status)?),
                _ => return Err(io::Error::new(io::ErrorKind::Other, "invalid status line")),
            };

            if let Some(stdout) = lines.next() {
                if let Some(stderr) = lines.next() {
                    let output = (status, unescape(stdout?), unescape(stderr?));
                    let mut outputs = outputs.lock().unwrap();
                    outputs.insert(id, output);
                    continue;
                }
            }
        }
        return Err(io::Error::new(io::ErrorKind::Other, "invalid response"));
    }
}

fn parse_u8(input: &str) -> io::Result<u8> {
    input.parse::<u8>().map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
}

fn parse_usize(input: &str) -> io::Result<usize> {
    input.parse::<usize>().map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
}

fn unescape(input: String) -> String {
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
