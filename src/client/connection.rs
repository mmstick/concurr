use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::str;

#[derive(Debug)]
pub enum ConnectionError {
    IO(io::Error),
}

impl From<io::Error> for ConnectionError {
    fn from(err: io::Error) -> ConnectionError { ConnectionError::IO(err) }
}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            ConnectionError::IO(ref err) => write!(f, "I/O error: {}", err),
        }
    }
}

pub struct Connection {
    pub address: SocketAddr,
    pub command: usize,
    pub cores:   usize,
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Ok(mut stream) = TcpStream::connect(self.address) {
            let _ =
                stream.write_all(["del ", &(self.command).to_string(), "\r\n"].concat().as_bytes());
        }
    }
}

impl Connection {
    pub fn new(address: SocketAddr) -> Result<Connection, ConnectionError> {
        fn get_cores(addr: SocketAddr) -> io::Result<usize> {
            let mut stream = TcpStream::connect(addr)?;
            stream.write_all(b"get cores\r\n")?;
            let mut string = String::new();
            BufReader::new(stream).read_line(&mut string)?;
            string[..string.len() - 1]
                .parse::<usize>()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
        }

        Ok(Connection {
            address,
            command: 0,
            cores: get_cores(address)?,
        })
    }

    pub fn send_command(&mut self, command: &str) -> io::Result<usize> {
        let mut stream = TcpStream::connect(self.address)?;
        stream.write_all(["com ", command, "\r\n"].concat().as_bytes())?;
        let mut string = String::new();
        BufReader::new(stream).read_line(&mut string)?;
        let id = string[..string.len() - 1]
            .parse::<usize>()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))?;
        self.command = id;
        Ok(id)
    }
}
