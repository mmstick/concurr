use std::fmt::{self, Display, Formatter};

// # Supported Tokens
// - {}: Placeholder
// - {%}: Slot Number
// - {#}: Job Number
// - {##}: Total Job Number

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Placeholder,
    Slot,
    Job,
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedCommand {
    pub tokens: Vec<Token>,
}

impl PreparedCommand {
    pub fn new(input: &str) -> PreparedCommand {
        let mut tokens = Vec::new();
        let mut start = 0;
        // Value will be set to true when a placeholder token is utilized.
        let mut placeholder = false;
        let mut open = false;

        for (id, character) in input.char_indices() {
            match character {
                '{' if !open => {
                    open = true;
                    if start != id {
                        tokens.push(Token::Text(String::from(&input[start..id])));
                        start = id;
                    }
                }
                '}' if open => {
                    open = false;
                    match &input[start..id + 1] {
                        "{}" => {
                            tokens.push(Token::Placeholder);
                            placeholder = true;
                        }
                        "{%}" => tokens.push(Token::Slot),
                        "{#}" => tokens.push(Token::Job),
                        _ => continue,
                    }
                    start = id + 1;
                }
                _ => (),
            }
        }

        // Take care of any stragglers left behind.
        if start != input.len() {
            tokens.push(Token::Text(String::from(&input[start..])));
        }

        // If a placeholder token was not supplied, append one at the end of the command.
        if !placeholder {
            let mut append_text = false;
            match tokens.last_mut() {
                Some(&mut Token::Text(ref mut string)) => string.push(' '),
                Some(_) => append_text = true,
                _ => (),
            };
            if append_text {
                tokens.push(Token::Text(String::from(" ")));
            }
            tokens.push(Token::Placeholder)
        }

        PreparedCommand { tokens }
    }
}

impl Display for PreparedCommand {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for token in &self.tokens {
            match *token {
                Token::Placeholder => f.write_str("{}")?,
                Token::Slot => f.write_str("{%}")?,
                Token::Job => f.write_str("{#}")?,
                Token::Text(ref string) => f.write_str(string)?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PreparedCommand, Token};

    #[test]
    fn tokens() {
        let expected = PreparedCommand {
            tokens: vec![
                Token::Text("echo ".into()),
                Token::Job,
                Token::Text(": ".into()),
                Token::Placeholder,
            ],
        };

        assert_eq!(PreparedCommand::new("echo {#}: {}"), expected);
        assert_eq!(PreparedCommand::new("echo {#}:"), expected);
    }
}
