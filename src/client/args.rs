use std::env::args;
use std::fmt::{Display, Formatter, self};

#[derive(Debug, PartialEq)]
pub enum ArgUnit {
    Strings(Vec<String>),
    Files(Vec<String>)
}

#[derive(Debug, PartialEq)]
pub enum ArgumentError {
    NoCommand,
    NoInputs,
    Invalid(String)
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Action {
    ParseStrings,
    ParseFiles,
    Stop
}

#[derive(Debug, PartialEq)]
pub struct Arguments {
    command: String,
    pub args: Vec<ArgUnit>,
}

impl Display for ArgumentError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            ArgumentError::NoCommand => write!(f, "no command was given"),
            ArgumentError::NoInputs => write!(f, "no inputs were supplied"),
            ArgumentError::Invalid(ref op) => write!(f, "invalid argument operator: '{}'", op)
        }
    }
}

impl Arguments {
    pub fn new() -> Result<Arguments, ArgumentError> {
        let mut args = args().skip(1);
        let mut store = Arguments {
            command: args.next().ok_or(ArgumentError::NoCommand)?,
            args: Vec::new()
        };

        let mut action;

        if let Some(arg) = args.next() {
            action = match arg.as_str() {
                ":"  => store.parse(&mut args, true),
                "::" => store.parse(&mut args, false),
                _    => return Err(ArgumentError::Invalid(arg))
            };
            loop {
                action = match action {
                    Action::ParseStrings => store.parse(&mut args, true),
                    Action::ParseFiles => store.parse(&mut args, false),
                    Action::Stop => break
                }
            }
            Ok(store)
        } else {
            Err(ArgumentError::NoInputs)
        }
    }

    pub fn get_command<'a>(&'a self) -> &'a str {
        self.command.as_str()
    }

    fn parse<I: Iterator<Item = String>>(&mut self, iter: &mut I, is_string: bool) -> Action {
        let mut args = Vec::new();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                ":" => {
                    self.args.push(if is_string { ArgUnit::Strings(args) } else { ArgUnit::Files(args) });
                    return Action::ParseStrings;
                },
                "::" => {
                    self.args.push(if is_string { ArgUnit::Strings(args) } else { ArgUnit::Files(args) });
                    return Action::ParseFiles;
                },
                _ => args.push(arg),
            }
        }

        self.args.push(if is_string { ArgUnit::Strings(args) } else { ArgUnit::Files(args) });
        Action::Stop
    }
}
