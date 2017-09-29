mod args;
mod server;

use args::{Arguments, ArgUnit};
use server::Connection;
use std::process::exit;

const LOCALHOST: &str = "127.0.0.1:12345";

fn main() {
    let arguments = match Arguments::new() {
        Ok(arguments) => arguments,
        Err(why) => {
            eprintln!("concurr [CRITICAL]: {}", why);
            exit(1);
        }
    };

    eprintln!("[INFO] parsing IP address");
    let mut connection = match Connection::new(LOCALHOST) {
        Ok(connection) => connection,
        Err(why) => {
            eprintln!("[CRITICAL] {}", why);
            exit(1);
        }
    };

    eprintln!("[INFO] obtaining core count from local server");
    let cores = match connection.get_cores() {
        Ok(ncores) => ncores,
        Err(why) => {
            eprintln!("[CRITICAL] {}", why);
            exit(1);
        }
    };

    eprintln!("[INFO] found {} cores on 127.0.0.1", cores);
    let command_id = match connection.send_command(arguments.get_command()) {
        Ok(command_id) => command_id,
        Err(why) => {
            eprintln!("[CRITICAL] {}", why);
            exit(1);
        }
    };

    if arguments.args.len() != 1 {
        unimplemented!()
    }

    match arguments.args[0] {
        ArgUnit::Strings(ref vec) => {
            for (jid, ref input) in vec.iter().enumerate() {
                match connection.send_input(command_id, jid, input) {
                    Ok((id, status, out, err)) => {
                        eprintln!("ID: {}; Status: {}\nSTDOUT: {}\nSTDERR: {}", id, status, out, err);
                    }
                    Err(why) => eprintln!("unable to get output of {}: {}", jid, why),
                }
            }
        },
        ArgUnit::Files(_) => {
            unimplemented!()
        }
    }
}
