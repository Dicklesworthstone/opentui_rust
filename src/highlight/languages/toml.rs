use crate::highlight::token::{Token, TokenKind};
use crate::highlight::tokenizer::{LineState, StringKind, Tokenizer};

pub struct TomlTokenizer;

impl Default for TomlTokenizer {
    fn default() -> Self {
        Self
    }
}

impl TomlTokenizer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_basic_string(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        line_len: usize,
        quote: char,
    ) -> (usize, bool) {
        let mut escaped = false;
        while let Some((idx, ch)) = chars.next() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                return (idx + 1, true);
            }
        }
        (line_len, false)
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_literal_string(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        line_len: usize,
        quote: char,
    ) -> (usize, bool) {
        while let Some((idx, ch)) = chars.next() {
            if ch == quote {
                return (idx + 1, true);
            }
        }
        (line_len, false)
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_multiline_string(
        line: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        quote: char,
    ) -> (usize, bool) {
        let triple = if quote == '"' { "\"\"\"" } else { "'''" };
        while let Some((idx, ch)) = chars.next() {
            if ch == quote && line[idx..].starts_with(triple) {
                chars.next();
                chars.next();
                return (idx + 3, true);
            }
        }
        (line.len(), false)
    }

    fn scan_number_like(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        start: usize,
        first: char,
    ) -> usize {
        let mut end = start + first.len_utf8();
        while let Some(&(i, c)) = chars.peek() {
            if c.is_ascii_digit()
                || c.is_ascii_hexdigit()
                || matches!(
                    c,
                    '_' | '.' | ':' | '+' | '-' | 'T' | 'Z' | 'e' | 'E' | 'x' | 'o' | 'b'
                )
            {
                chars.next();
                end = i + 1;
            } else {
                break;
            }
        }
        end
    }
}

impl Tokenizer for TomlTokenizer {
    fn name(&self) -> &'static str {
        "TOML"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }

    #[allow(clippy::too_many_lines)]
    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        let mut tokens = Vec::new();
        let mut chars = line.char_indices().peekable();
        let mut parsing_key = true;
        let mut inline_depth = 0usize;
        let mut at_line_start = true;

        if matches!(state, LineState::InString(StringKind::Triple)) {
            let (end_idx, found_end) = Self::scan_multiline_string(line, &mut chars, '"');
            if found_end {
                tokens.push(Token::new(TokenKind::String, 0, end_idx));
            } else {
                tokens.push(Token::new(TokenKind::String, 0, line.len()));
                return (tokens, LineState::InString(StringKind::Triple));
            }
        }

        while let Some((idx, ch)) = chars.next() {
            if ch.is_whitespace() {
                continue;
            }

            if at_line_start {
                at_line_start = false;
                if ch == '[' {
                    let mut end_idx = idx + 1;
                    let mut needed = 1usize;
                    if let Some(&(_, '[')) = chars.peek() {
                        chars.next();
                        needed = 2;
                        end_idx = idx + 2;
                    }
                    for (i, c) in chars.by_ref() {
                        end_idx = i + 1;
                        if c == ']' {
                            needed = needed.saturating_sub(1);
                            if needed == 0 {
                                break;
                            }
                        }
                    }
                    tokens.push(Token::new(TokenKind::Type, idx, end_idx));
                    parsing_key = false;
                    continue;
                }
            }

            match ch {
                '#' => {
                    tokens.push(Token::new(TokenKind::Comment, idx, line.len()));
                    break;
                }

                '"' => {
                    if line[idx..].starts_with("\"\"\"") {
                        chars.next();
                        chars.next();
                        let (end_idx, found_end) =
                            Self::scan_multiline_string(line, &mut chars, '"');
                        if found_end {
                            let kind = if parsing_key {
                                TokenKind::Identifier
                            } else {
                                TokenKind::String
                            };
                            tokens.push(Token::new(kind, idx, end_idx));
                        } else {
                            tokens.push(Token::new(
                                if parsing_key {
                                    TokenKind::Identifier
                                } else {
                                    TokenKind::String
                                },
                                idx,
                                line.len(),
                            ));
                            return (tokens, LineState::InString(StringKind::Triple));
                        }
                    } else {
                        let (end_idx, found_end) =
                            Self::scan_basic_string(&mut chars, line.len(), '"');
                        if found_end {
                            let kind = if parsing_key {
                                TokenKind::Identifier
                            } else {
                                TokenKind::String
                            };
                            tokens.push(Token::new(kind, idx, end_idx));
                        } else {
                            let kind = if parsing_key {
                                TokenKind::Identifier
                            } else {
                                TokenKind::String
                            };
                            tokens.push(Token::new(kind, idx, line.len()));
                            return (tokens, LineState::InString(StringKind::Double));
                        }
                    }
                }

                '\'' => {
                    if line[idx..].starts_with("'''") {
                        chars.next();
                        chars.next();
                        let (end_idx, found_end) =
                            Self::scan_multiline_string(line, &mut chars, '\'');
                        if found_end {
                            let kind = if parsing_key {
                                TokenKind::Identifier
                            } else {
                                TokenKind::String
                            };
                            tokens.push(Token::new(kind, idx, end_idx));
                        } else {
                            tokens.push(Token::new(
                                if parsing_key {
                                    TokenKind::Identifier
                                } else {
                                    TokenKind::String
                                },
                                idx,
                                line.len(),
                            ));
                            return (tokens, LineState::InString(StringKind::Triple));
                        }
                    } else {
                        let (end_idx, found_end) =
                            Self::scan_literal_string(&mut chars, line.len(), '\'');
                        if found_end {
                            let kind = if parsing_key {
                                TokenKind::Identifier
                            } else {
                                TokenKind::String
                            };
                            tokens.push(Token::new(kind, idx, end_idx));
                        } else {
                            let kind = if parsing_key {
                                TokenKind::Identifier
                            } else {
                                TokenKind::String
                            };
                            tokens.push(Token::new(kind, idx, line.len()));
                            return (tokens, LineState::InString(StringKind::Single));
                        }
                    }
                }

                '=' => {
                    tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                    parsing_key = false;
                }

                ',' => {
                    tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                    if inline_depth > 0 {
                        parsing_key = true;
                    }
                }

                '{' | '}' | '[' | ']' => {
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                    if ch == '{' {
                        inline_depth = inline_depth.saturating_add(1);
                        parsing_key = true;
                    } else if ch == '}' {
                        inline_depth = inline_depth.saturating_sub(1);
                    }
                }

                c if c.is_ascii_digit()
                    || ((c == '-' || c == '+')
                        && chars.peek().is_some_and(|&(_, next)| next.is_ascii_digit())) =>
                {
                    let end = Self::scan_number_like(&mut chars, idx, c);
                    tokens.push(Token::new(TokenKind::Number, idx, end));
                }

                't' => {
                    if line[idx..].starts_with("true") {
                        tokens.push(Token::new(TokenKind::Boolean, idx, idx + 4));
                        for _ in 0..3 {
                            chars.next();
                        }
                    } else if parsing_key {
                        let mut end = idx + 1;
                        while let Some(&(i, c)) = chars.peek() {
                            if c.is_alphanumeric() || c == '_' || c == '-' {
                                chars.next();
                                end = i + 1;
                            } else {
                                break;
                            }
                        }
                        tokens.push(Token::new(TokenKind::Identifier, idx, end));
                    } else {
                        tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                    }
                }

                'f' => {
                    if line[idx..].starts_with("false") {
                        tokens.push(Token::new(TokenKind::Boolean, idx, idx + 5));
                        for _ in 0..4 {
                            chars.next();
                        }
                    } else if parsing_key {
                        let mut end = idx + 1;
                        while let Some(&(i, c)) = chars.peek() {
                            if c.is_alphanumeric() || c == '_' || c == '-' {
                                chars.next();
                                end = i + 1;
                            } else {
                                break;
                            }
                        }
                        tokens.push(Token::new(TokenKind::Identifier, idx, end));
                    } else {
                        tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                    }
                }

                c if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' => {
                    let mut end = idx + 1;
                    while let Some(&(i, c)) = chars.peek() {
                        if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                            chars.next();
                            end = i + 1;
                        } else {
                            break;
                        }
                    }
                    if parsing_key {
                        tokens.push(Token::new(TokenKind::Identifier, idx, end));
                    } else {
                        tokens.push(Token::new(TokenKind::Error, idx, end));
                    }
                }

                _ => {
                    tokens.push(Token::new(TokenKind::Text, idx, idx + 1));
                }
            }
        }

        (tokens, LineState::Normal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_sections() {
        let tokenizer = TomlTokenizer::new();
        let line = "[section.sub]";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        let header = tokens
            .iter()
            .find(|token| token.kind == TokenKind::Type)
            .expect("section token");
        assert_eq!(&line[header.range()], "[section.sub]");
    }

    #[test]
    fn test_toml_key_values() {
        let tokenizer = TomlTokenizer::new();
        let line = "key = \"value\"";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Identifier)
        );
        assert!(tokens.iter().any(|token| token.kind == TokenKind::String));
    }

    #[test]
    fn test_toml_strings() {
        let tokenizer = TomlTokenizer::new();
        let line = "literal = 'no escapes'";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        assert!(tokens.iter().any(|token| token.kind == TokenKind::String));
    }

    #[test]
    fn test_toml_multiline_strings() {
        let tokenizer = TomlTokenizer::new();

        let (tokens, state) = tokenizer.tokenize_line("multiline = \"\"\"start", LineState::Normal);
        assert_eq!(tokens.last().unwrap().kind, TokenKind::String);
        assert_eq!(state, LineState::InString(StringKind::Triple));

        let (tokens, state) = tokenizer.tokenize_line("middle", state);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(state, LineState::InString(StringKind::Triple));

        let (tokens, state) = tokenizer.tokenize_line("end\"\"\"", state);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(state, LineState::Normal);
    }

    #[test]
    fn test_toml_numbers_and_dates() {
        let tokenizer = TomlTokenizer::new();
        let line = "int = 42 hex = 0xDEAD date = 2024-01-15";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        let numbers: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Number)
            .collect();
        assert!(numbers.len() >= 3);
    }

    #[test]
    fn test_toml_arrays() {
        let tokenizer = TomlTokenizer::new();
        let line = "array = [1, 2]";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Punctuation)
        );
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Number));
    }

    #[test]
    fn test_toml_inline_tables() {
        let tokenizer = TomlTokenizer::new();
        let line = "inline = { key = \"value\", other = 42 }";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        let identifiers: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Identifier)
            .collect();
        assert!(identifiers.len() >= 2);
    }
}
