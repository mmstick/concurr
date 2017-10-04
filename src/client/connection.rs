use certificate;
use native_tls::{Certificate, TlsConnector, TlsStream};
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead, BufReader, Read, Write};
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
    pub connection: TlsStream<TcpStream>,
    pub domain:     String,
    pub address:    SocketAddr,
    pub command:    usize,
    pub cores:      usize,
}

impl Drop for Connection {
    fn drop(&mut self) {
        let result = self.connection
            .write_all(["del ", &(self.command).to_string(), "\r\n"].concat().as_bytes());
        if let Err(_) = result {
            eprintln!(
                "concurr [CRITICAL]: you will need to manually delete the job from the server"
            );
        }
    }
}

impl Connection {
    pub fn new(address: SocketAddr, domain: String) -> Result<Connection, ConnectionError> {
        let mut connection = attempt_connection(address, &domain, certificate::get(&domain))?;
        let cores = get_cores(&mut connection)?;

        Ok(Connection {
            connection,
            domain,
            address,
            command: 0,
            cores,
        })
    }

    pub fn send_command(&mut self, command: &str) -> io::Result<usize> {
        let mut string = String::new();
        let instruction = ["com ", command, "\r\n"].concat();
        attempt_write(&mut self.connection, instruction)?;
        BufReader::new(&mut self.connection).read_line(&mut string)?;
        let id = string[..string.len() - 1]
            .parse::<usize>()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))?;
        self.command = id;
        Ok(id)
    }
}

pub fn attempt_connection<DOMAIN: AsRef<str>>(
    addr: SocketAddr,
    domain: DOMAIN,
    certificate: Certificate,
) -> io::Result<TlsStream<TcpStream>> {
    // Keep track of how many failed attempts have been made to connect to the node.
    let (mut ctries, mut etries) = (0, 0);

    // The connector will be used to upgrade an unencrypted `TcpStream` into a `TlsStream`.
    let mut tls_builder = TlsConnector::builder().unwrap();
    let _ = tls_builder.add_root_certificate(certificate).unwrap();
    let connector = tls_builder.build().unwrap();

    // Attempt to obtain a `TlsStream<TcpStream>`.
    let encrypted_stream = loop {
        // First initialize an unencrypted connection to the server.
        let unencrypted_stream = loop {
            match TcpStream::connect(addr) {
                Ok(conn) => break conn,
                Err(why) => {
                    if ctries == 3 {
                        return Err(io::Error::new(io::ErrorKind::Other, "unable to connect"));
                    }
                    ctries += 1;
                    eprintln!("concurr [CRITICAL]: {}", why);
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            }
        };

        // Then upgrade that to an encrypted connection
        match connector.connect(domain.as_ref(), unencrypted_stream) {
            Ok(conn) => break conn,
            Err(why) => {
                if etries == 3 {
                    return Err(io::Error::new(io::ErrorKind::Other, "TLS connection failed"));
                }
                etries += 1;
                eprintln!("concurr [CRITICAL]: {}", why);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        }
    };

    Ok(encrypted_stream)
}

pub fn attempt_write<STREAM: Write, INSTRUCTION: AsRef<[u8]>>(
    stream: &mut STREAM,
    instruction: INSTRUCTION,
) -> io::Result<()> {
    let mut tries = 0;
    loop {
        if let Err(why) = stream.write_all(instruction.as_ref()) {
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

fn get_cores<STREAM: Read + Write>(stream: &mut STREAM) -> io::Result<usize> {
    attempt_write(stream, b"get cores\r\n")?;
    let mut string = String::new();
    BufReader::new(stream).read_line(&mut string)?;
    string[..string.len() - 1]
        .parse::<usize>()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "cores value is NaN"))
}
