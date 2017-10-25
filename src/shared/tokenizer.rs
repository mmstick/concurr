use std::fmt::{self, Display, Formatter};

// # Supported Tokens
// - {}: Placeholder
// - {%}: Slot Number
// - {#}: Job Number

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Placeholder,
    Slot,
    Job,
    Text(String),
}

const PLACE: u8 = 1;
const OPEN: u8 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub tokens: Vec<Token>,
}

impl Tokens {
    pub fn new(input: &str) -> Tokens {
        let mut tokens = Vec::new();
        let mut start = 0;
        let mut flags = 0;
        let mut chars = input.char_indices();

        while let Some((id, character)) = chars.next() {
            match character {
                '\\' => {
                    let _ = chars.next();
                }
                '{' if flags & OPEN == 0 => {
                    flags |= OPEN;
                    if start != id {
                        tokens.push(Token::Text(String::from(&input[start..id])));
                        start = id;
                    }
                }
                '}' if flags & OPEN != 0 => {
                    flags ^= OPEN;
                    match &input[start..id + 1] {
                        "{}" => {
                            tokens.push(Token::Placeholder);
                            flags |= PLACE;
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
        if flags & PLACE == 0 {
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

        Tokens { tokens }
    }
}

impl Display for Tokens {
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
    use super::{Token, Tokens};

    #[test]
    fn tokens() {
        let expected = Tokens {
            tokens: vec![
                Token::Text("echo ".into()),
                Token::Job,
                Token::Text(": ".into()),
                Token::Placeholder,
            ],
        };

        assert_eq!(Tokens::new("echo {#}: {}"), expected);
        assert_eq!(Tokens::new("echo {#}:"), expected);
    }
}
