use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::str;
use std::thread;
use std::time::Duration;

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
            let mut stream = attempt_connection(addr)?;
            stream.set_read_timeout(Some(Duration::from_secs(3)))?;
            stream.set_write_timeout(Some(Duration::from_secs(3)))?;
            attempt_write(&mut stream, b"get cores\r\n")?;
            let mut string = String::new();
            BufReader::new(&mut stream).read_line(&mut string)?;
            drop(stream);
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
        let mut string = String::new();
        let instruction = ["com ", command, "\r\n"].concat();
        let mut stream = attempt_connection(self.address)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;
        attempt_write(&mut stream, instruction.as_bytes())?;
        BufReader::new(&mut stream).read_line(&mut string)?;
        drop(stream);
        let id = string[..string.len() - 1]
            .parse::<usize>()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))?;
        self.command = id;
        Ok(id)
    }
}

pub fn attempt_connection(addr: SocketAddr) -> io::Result<TcpStream> {
    let mut tries = 0;
    let stream = loop {
        match TcpStream::connect(addr) {
            Ok(conn) => break conn,
            Err(why) => {
                if tries == 3 {
                    return Err(why);
                }
                tries += 1;
                eprintln!("concurr [CRITICAL]: connection issue: {}", why);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        }
    };
    Ok(stream)
}

pub fn attempt_write(stream: &mut TcpStream, instruction: &[u8]) -> io::Result<()> {
    let mut tries = 0;
    loop {
        if let Err(why) = stream.write_all(instruction) {
            if tries == 3 {
                return Err(why);
            }
            tries += 1;
            eprintln!("concurr [CRITICAL]: connection issue: {}", why);
            thread::sleep(Duration::from_secs(1));
            continue;
        }
        break;
    }
    Ok(())
}
