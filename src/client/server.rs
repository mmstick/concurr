use std::io::{self, BufRead, BufReader, Write};
use std::net::{AddrParseError, SocketAddr, TcpStream};
use std::str;

pub struct Connection {
    address:  SocketAddr,
    commands: Vec<usize>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(mut stream) = TcpStream::connect(self.address) {
            for id in &self.commands {
                let _ = stream.write_all(["del ", &(*id).to_string(), "\r\n"].concat().as_bytes());
            }
        }
    }
}

impl Connection {
    pub fn new(addr: &str) -> Result<Connection, AddrParseError> {
        addr.parse().map(|address| {
            Connection {
                address,
                commands: Vec::new(),
            }
        })
    }

    pub fn get_cores(&self) -> io::Result<usize> {
        let mut stream = TcpStream::connect(self.address)?;
        stream.write_all(b"get cores\r\n")?;
        let mut string = String::new();
        BufReader::new(stream).read_line(&mut string)?;
        string[..string.len() - 1]
            .parse::<usize>()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
    }

    pub fn send_command(&mut self, command: &str) -> io::Result<usize> {
        let mut stream = TcpStream::connect(self.address)?;
        stream.write_all(["com ", command, "\r\n"].concat().as_bytes())?;
        let mut string = String::new();
        BufReader::new(stream).read_line(&mut string)?;
        let id = string[..string.len() - 1]
            .parse::<usize>()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))?;
        self.commands.push(id);
        Ok(id)
    }

    pub fn send_input(
        &self,
        cid: usize,
        jid: usize,
        input: &str,
    ) -> io::Result<(usize, u8, String, String)> {
        let mut stream = TcpStream::connect(self.address)?;
        stream.write_all(format!("inp {} {} {}\r\n", cid, jid, input).as_bytes())?;
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
                    return Ok((id, status, unescape(stdout?), unescape(stderr?)));
                }
            }
        }
        Err(io::Error::new(io::ErrorKind::Other, "invalid response"))
    }
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

fn parse_u8(input: &str) -> io::Result<u8> {
    input.parse::<u8>().map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
}

fn parse_usize(input: &str) -> io::Result<usize> {
    input.parse::<usize>().map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
}
