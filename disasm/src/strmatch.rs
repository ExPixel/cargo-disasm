use std::iter::Peekable;
use std::str::Chars;

pub struct Tokenizer<'a> {
    source: Chars<'a>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(source: &'a str) -> Tokenizer<'a> {
        Tokenizer {
            source: source.chars(),
        }
    }

    fn next_char(&mut self) -> Option<char> {
        self.source.next()
    }

    fn peek_char(&self) -> Option<char> {
        self.source.as_str().chars().next()
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let start_str = self.source.as_str();

        match self.next_char()? {
            ':' if self.peek_char() == Some(':') => {
                self.next_char().unwrap();
            }

            'a'..='z' | 'A'..='Z' | '_' => {
                while self
                    .peek_char()
                    .filter(|ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9'))
                    .is_some()
                {
                    self.next_char().unwrap();
                }
            }

            _ => {}
        }

        let end_str = self.source.as_str();
        Some(&start_str[..(start_str.len() - end_str.len())])
    }
}

pub fn distance<'lhs, 'rhs, Lhs, Rhs>(lhs: Lhs, rhs: Rhs) -> Option<u32>
where
    Lhs: IntoIterator<Item = &'lhs str>,
    Rhs: IntoIterator<Item = &'rhs str>,
{
    let mut dist = 0;
    let mut rhs = rhs.into_iter();

    for lhs in lhs {
        loop {
            let rhs = rhs.next()?;

            if lhs == rhs {
                break;
            } else {
                dist += 1;
            }
        }
    }

    Some(dist)
}
