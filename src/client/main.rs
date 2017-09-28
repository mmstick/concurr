mod server;

use server::Connection;
use std::env;
use std::process::exit;

const LOCALHOST: &str = "127.0.0.1:12345";
const INPUTS: [&str; 10] = [
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
];

fn main() {
    let mut args = env::args().skip(1);

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
    let command_id = match args.next().map(|ref x| connection.send_command(x)) {
        Some(Ok(command_id)) => command_id,
        Some(Err(why)) => {
            eprintln!("[CRITICAL] {}", why);
            exit(1);
        }
        None => {
            eprintln!("no command supplied");
            exit(1);
        }
    };

    for (jid, input) in INPUTS.iter().enumerate() {
        match connection.send_input(command_id, jid, *input) {
            Ok((id, status, out, err)) => {
                eprintln!("ID: {}; Status: {}\nSTDOUT: {}\nSTDERR: {}", id, status, out, err);
            }
            Err(why) => eprintln!("unable to get output of {}", jid),
        }
    }
}
