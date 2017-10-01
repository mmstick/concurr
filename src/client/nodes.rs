use connection::{Connection, ConnectionError};
use std::net::SocketAddr;

pub fn get(nodes: &[SocketAddr], command: &str) -> Result<Vec<Connection>, ConnectionError> {
    let mut output = Vec::with_capacity(nodes.len());
    for node in nodes {
        let mut conn = Connection::new(*node)?;
        eprintln!("[INFO] found {} cores on {:?}", conn.cores, conn.address);
        conn.send_command(command)?;
        output.push(conn);
    }

    Ok(output)
}
