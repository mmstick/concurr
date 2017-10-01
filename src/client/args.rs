use redirection::{self, RedirectionSource};
use std::env::args;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum ArgsSource {
    Cli(Vec<ArgUnit>),
    RedirFile(PathBuf),
    RedirPipe,
}

#[derive(Debug, PartialEq)]
pub enum ArgUnit {
    Strings(Vec<String>),
    Files(Vec<String>),
}

#[derive(Debug, PartialEq)]
pub enum ArgumentError {
    NoCommand,
    NoInputs,
    Invalid(String),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    ParseStrings,
    ParseFiles,
    Stop,
}

#[derive(Debug, PartialEq)]
pub struct Arguments {
    command:  String,
    pub args: ArgsSource,
}

impl Display for ArgumentError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            ArgumentError::NoCommand => write!(f, "no command was given"),
            ArgumentError::NoInputs => write!(f, "no inputs were supplied"),
            ArgumentError::Invalid(ref op) => write!(f, "invalid argument operator: '{}'", op),
        }
    }
}

impl Arguments {
    pub fn new() -> Result<Arguments, ArgumentError> {
        let mut args = args().skip(1);

        // THe first argument should be the command.
        let command = args.next().ok_or(ArgumentError::NoCommand)?;

        // Check if any redirections happened, and if so, this will notify the program to
        // obtain pipes from the source of the redirection directly.
        match redirection::source() {
            Some(RedirectionSource::Pipe) => {
                return Ok(Arguments {
                    command,
                    args: ArgsSource::RedirPipe,
                });
            }
            Some(RedirectionSource::File(path)) => {
                return Ok(Arguments {
                    command,
                    args: ArgsSource::RedirFile(path),
                });
            }
            None => (),
        }

        // Otherwise, we will attempt to parse arguments supplied to the command line.
        let mut store = Vec::new();
        // If the user specified permutated inputs, we will need to a place to store actions.
        // The value indicates what types of inputs are being read. Files? Strings? Appends?
        let mut action;

        if let Some(arg) = args.next() {
            action = match arg.as_str() {
                ":" => parse(&mut store, &mut args, true),
                "::" => parse(&mut store, &mut args, false),
                _ => return Err(ArgumentError::Invalid(arg)),
            };
            loop {
                action = match action {
                    Action::ParseStrings => parse(&mut store, &mut args, true),
                    Action::ParseFiles => parse(&mut store, &mut args, false),
                    Action::Stop => break,
                }
            }
            Ok(Arguments {
                command,
                args: ArgsSource::Cli(store),
            })
        } else {
            Err(ArgumentError::NoInputs)
        }
    }

    pub fn get_command<'a>(&'a self) -> &'a str { self.command.as_str() }
}

fn parse<I: Iterator<Item = String>>(
    vec: &mut Vec<ArgUnit>,
    iter: &mut I,
    is_string: bool,
) -> Action {
    let mut args = Vec::new();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            ":" => {
                vec.push(if is_string { ArgUnit::Strings(args) } else { ArgUnit::Files(args) });
                return Action::ParseStrings;
            }
            "::" => {
                vec.push(if is_string { ArgUnit::Strings(args) } else { ArgUnit::Files(args) });
                return Action::ParseFiles;
            }
            _ => args.push(arg),
        }
    }

    vec.push(if is_string { ArgUnit::Strings(args) } else { ArgUnit::Files(args) });
    Action::Stop
}
