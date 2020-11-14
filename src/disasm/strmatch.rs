use std::str::Chars;

pub struct Tokenizer<'a> {
    source: Chars<'a>,
    normalize_whitespace: bool,
}

impl<'a> Tokenizer<'a> {
    pub fn new(source: &'a str) -> Tokenizer<'a> {
        Tokenizer {
            source: source.chars(),
            normalize_whitespace: true,
        }
    }

    pub fn no_whitespace_normalize(source: &'a str) -> Tokenizer<'a> {
        Tokenizer {
            source: source.chars(),
            normalize_whitespace: false,
        }
    }

    fn set_normalize_whitespace(&mut self, n: bool) {
        self.normalize_whitespace = n;
    }

    fn next_char(&mut self) -> Option<char> {
        self.source.next()
    }

    // fn peek_char(&self) -> Option<char> {
    //     self.source.as_str().chars().next()
    // }

    fn next_char_if<F>(&mut self, f: F) -> bool
    where
        F: FnOnce(char) -> bool,
    {
        let mut chars = self.source.as_str().chars();

        match chars.next() {
            Some(ch) if f(ch) => {
                self.source = chars;
                true
            }
            _ => false,
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let start_str = self.source.as_str();

        match self.next_char()? {
            // group :: into a single token
            ':' if self.next_char_if(|ch| ch == ':') => {
                // DO NOTHING
            }

            // normalize whitespace
            ch if ch.is_whitespace() && self.normalize_whitespace => {
                loop {
                    if !self.next_char_if(|ch| ch.is_whitespace()) {
                        break;
                    }
                }
                return Some(" ");
            }

            // group identifiers into a single token
            'a'..='z' | 'A'..='Z' | '_' => loop {
                if !self.next_char_if(|ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9')) {
                    break;
                }
            },

            // group numbers (possibly separated by underscores) into a single token
            '0'..='9' => loop {
                if !self.next_char_if(|ch| matches!(ch, '0'..='9' | '_')) {
                    break;
                }
            },

            _ => {}
        }

        let end_str = self.source.as_str();
        Some(unsafe { start_str.get_unchecked(..(start_str.len() - end_str.len())) })
    }
}

pub fn distance<'lhs, 'rhs, Lhs, Rhs>(lhs: Lhs, rhs: Rhs, max_distance: u32) -> Option<u32>
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
                if dist > max_distance {
                    return None;
                }
            }
        }
    }

    Some(dist)
}
