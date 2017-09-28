use super::obtain;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::str;

#[derive(Debug, PartialEq)]
pub enum JobEvent {
    /// Create a new command to store in the job server.
    Command(String),
    /// Execute an input, using the ID of the command to execute it with, and the ID of the job
    /// being executed.
    Input(usize, usize, String),
    /// Return a list of commands currently stored in the job server.
    GetCommands,
    /// Return the number of cores on the machine that the job server is running on.
    GetCores,
    /// Delete a command from the command list.
    StopJob(usize),
}

impl JobEvent {
    // Obtain the `Command` event from the input.
    pub fn get_command(input: &[u8]) -> io::Result<Option<JobEvent>> {
        Ok(Some(JobEvent::Command(obtain(input)?)))
    }

    /// Parses the input and returns one of the `Get` variants.
    pub fn get_option(input: &[u8]) -> io::Result<Option<JobEvent>> {
        match input {
            b"comms" => Ok(Some(JobEvent::GetCommands)),
            b"cores" => Ok(Some(JobEvent::GetCores)),
            _ => Err(io::Error::new(io::ErrorKind::Other, "unsupported value")),
        }
    }

    /// Attempts to parse a number from the input and uses that as the job to stop.
    pub fn del_command(input: &[u8]) -> io::Result<Option<JobEvent>> {
        Ok(Some(JobEvent::StopJob(parse_usize(input)?)))
    }

    /// Attempts to parse the `Input` event from a given byte slice.
    pub fn get_input(input: &[u8]) -> io::Result<Option<JobEvent>> {
        // Find the first space to get the value of the command ID to execute.
        if let Some(index) = input.iter().position(|&b| b == b' ') {
            // Obtain the ID of the command to execute.
            let cid = parse_usize(&input[..index])?;
            // Adjust the region of the slice for future searching.
            let input = &input[index + 1..];
            // Find the first space to get the value of the job ID to execute.
            if let Some(index) = input.iter().position(|&b| b == b' ') {
                // Obtain the ID of the job to execute.
                let id = parse_usize(&input[..index])?;
                // Then return an `Input` event that contains the input to process.
                return Ok(Some(JobEvent::Input(cid, id, obtain(&input[index + 1..])?)));
            }
        }

        // Indicates that the supplied input didn't provide enough arguments
        Err(io::Error::new(io::ErrorKind::Other, "not enough arguments"))
    }
}

pub enum ResponseEvent {
    Finished(usize, String),
    Output(usize, u8, String, String),
    Info(String),
}

impl Display for ResponseEvent {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            ResponseEvent::Finished(id, ref out) => write!(f, "job {} finished: {}", id, out),
            ResponseEvent::Output(jid, status, ref stdout, ref stderr) => {
                write!(f, "{} {}\n{}\n{}", jid, status, escape(stdout), escape(stderr))
            }
            ResponseEvent::Info(ref info) => write!(f, "{}", info),
        }
    }
}

fn parse_usize(input: &[u8]) -> io::Result<usize> {
    str::from_utf8(&input)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid UTF-8"))?
        .parse::<usize>()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "ID is NaN"))
}

fn escape(input: &str) -> String {
    let mut start = 0;
    let mut output = String::with_capacity(input.len());
    for (id, character) in input.char_indices() {
        match character {
            '\n' => {
                output.push_str(&input[start..id]);
                output.push_str("\\n");
                start = id + 1;
            }
            '\\' => {
                output.push_str(&input[start..id]);
                start = id;
            }
            _ => (),
        }
    }
    if start != input.len() {
        output.push_str(&input[start..]);
    }
    output
}
