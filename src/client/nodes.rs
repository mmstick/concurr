use connection::{Connection, ConnectionError};
use std::net::SocketAddr;

pub fn get<NODES: Iterator<Item = (SocketAddr, String)>>(
    nodes: NODES,
    command: &str,
) -> Result<Vec<Connection>, ConnectionError> {
    let mut output = Vec::new();
    for (addr, domain) in nodes {
        let mut conn = Connection::new(addr, domain)?;
        eprintln!("concurr [INFO]: found {} cores on {:?}", conn.cores, conn.address);
        conn.send_command(command)?;
        output.push(conn);
    }

    Ok(output)
}
