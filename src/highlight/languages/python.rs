use crate::highlight::token::{Token, TokenKind};
use crate::highlight::tokenizer::{LineState, StringKind, Tokenizer};

pub struct PythonTokenizer;

impl Default for PythonTokenizer {
    fn default() -> Self {
        Self
    }
}

impl PythonTokenizer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn is_keyword(word: &str) -> Option<TokenKind> {
        match word {
            // Control flow
            "if" | "elif" | "else" | "for" | "while" | "break" | "continue" | "return"
            | "yield" | "pass" | "raise" | "try" | "except" | "finally" | "with" | "as"
            | "assert" => Some(TokenKind::KeywordControl),

            // Definitions and imports
            "def" | "class" | "lambda" | "global" | "nonlocal" | "import" | "from" => {
                Some(TokenKind::Keyword)
            }

            // Operators as keywords + None literal
            "and" | "or" | "not" | "in" | "is" | "None" => Some(TokenKind::Keyword),

            // Async
            "async" | "await" => Some(TokenKind::KeywordModifier),

            // Literals
            "True" | "False" => Some(TokenKind::Boolean),
            _ => None,
        }
    }

    fn parse_prefixed_string_start(line: &str, idx: usize) -> Option<(usize, char, bool, bool)> {
        let bytes = line.as_bytes();
        if idx >= bytes.len() {
            return None;
        }

        let mut pos = idx;
        let mut raw = false;
        let mut saw_prefix = false;

        while pos < bytes.len() {
            let lower = bytes[pos].to_ascii_lowercase();
            if matches!(lower, b'r' | b'u' | b'b' | b'f') {
                saw_prefix = true;
                if lower == b'r' {
                    raw = true;
                }
                pos += 1;
            } else {
                break;
            }
        }

        if !saw_prefix || pos >= bytes.len() {
            return None;
        }

        let quote = bytes[pos];
        if quote == b'\'' || quote == b'"' {
            let triple =
                pos + 2 < bytes.len() && bytes[pos + 1] == quote && bytes[pos + 2] == quote;
            Some((pos - idx, quote as char, triple, raw))
        } else {
            None
        }
    }

    fn scan_string_body(
        line: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        quote: char,
        triple: bool,
        raw: bool,
    ) -> (usize, bool) {
        let mut end_idx = 0;
        let mut found_end = false;
        let mut escaped = false;
        let triple_seq = if quote == '\'' { "'''" } else { "\"\"\"" };

        while let Some((idx, ch)) = chars.next() {
            end_idx = idx + ch.len_utf8();

            if triple {
                if ch == quote && line[idx..].starts_with(triple_seq) {
                    chars.next();
                    chars.next();
                    end_idx = idx + 3;
                    found_end = true;
                    break;
                }
                continue;
            }

            if !raw {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
            }

            if ch == quote {
                found_end = true;
                break;
            }
        }

        if !found_end {
            end_idx = line.len();
        }

        (end_idx, found_end)
    }

    fn scan_any_triple_end(
        line: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    ) -> (usize, bool) {
        let mut end_idx = 0;
        let mut found_end = false;

        while let Some((idx, ch)) = chars.next() {
            end_idx = idx + ch.len_utf8();
            if ch == '\'' && line[idx..].starts_with("'''") {
                chars.next();
                chars.next();
                end_idx = idx + 3;
                found_end = true;
                break;
            }
            if ch == '"' && line[idx..].starts_with("\"\"\"") {
                chars.next();
                chars.next();
                end_idx = idx + 3;
                found_end = true;
                break;
            }
        }

        if !found_end {
            end_idx = line.len();
        }

        (end_idx, found_end)
    }
}

impl Tokenizer for PythonTokenizer {
    fn name(&self) -> &'static str {
        "Python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py", "pyw", "pyi"]
    }

    #[allow(clippy::too_many_lines)]
    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        let mut tokens = Vec::new();
        let mut chars = line.char_indices().peekable();
        let mut line_start = true;
        let mut resumed = false;

        match state {
            LineState::InString(StringKind::Double) => {
                let (end_idx, found_end) =
                    Self::scan_string_body(line, &mut chars, '"', false, false);
                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, end_idx));
                    resumed = true;
                } else {
                    tokens.push(Token::new(TokenKind::String, 0, line.len()));
                    return (tokens, LineState::InString(StringKind::Double));
                }
            }
            LineState::InString(StringKind::Single) => {
                let (end_idx, found_end) =
                    Self::scan_string_body(line, &mut chars, '\'', false, false);
                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, end_idx));
                    resumed = true;
                } else {
                    tokens.push(Token::new(TokenKind::String, 0, line.len()));
                    return (tokens, LineState::InString(StringKind::Single));
                }
            }
            LineState::InString(StringKind::Triple) => {
                let (end_idx, found_end) = Self::scan_any_triple_end(line, &mut chars);
                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, end_idx));
                    resumed = true;
                } else {
                    tokens.push(Token::new(TokenKind::String, 0, line.len()));
                    return (tokens, LineState::InString(StringKind::Triple));
                }
            }
            _ => {}
        }

        if resumed {
            line_start = false;
        }

        while let Some((idx, ch)) = chars.next() {
            if ch.is_whitespace() {
                continue;
            }

            if line_start && ch == '@' {
                let start = idx;
                let mut end = idx + 1;
                while let Some(&(i, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '.' {
                        chars.next();
                        end = i + 1;
                    } else {
                        break;
                    }
                }
                tokens.push(Token::new(TokenKind::Attribute, start, end));
                line_start = false;
                continue;
            }

            line_start = false;

            match ch {
                '#' => {
                    tokens.push(Token::new(TokenKind::Comment, idx, line.len()));
                    break;
                }

                '\'' | '"' => {
                    let start = idx;
                    let quote = ch;
                    let triple = if quote == '\'' {
                        line[idx..].starts_with("'''")
                    } else {
                        line[idx..].starts_with("\"\"\"")
                    };

                    if triple {
                        chars.next();
                        chars.next();
                    }

                    let (end_idx, found_end) =
                        Self::scan_string_body(line, &mut chars, quote, triple, false);
                    if found_end {
                        tokens.push(Token::new(TokenKind::String, start, end_idx));
                    } else {
                        tokens.push(Token::new(TokenKind::String, start, line.len()));
                        let next_state = if triple {
                            LineState::InString(StringKind::Triple)
                        } else if quote == '"' {
                            LineState::InString(StringKind::Double)
                        } else {
                            LineState::InString(StringKind::Single)
                        };
                        return (tokens, next_state);
                    }
                }

                c if c.is_alphabetic() || c == '_' => {
                    if matches!(c, 'r' | 'R' | 'u' | 'U' | 'b' | 'B' | 'f' | 'F') {
                        if let Some((prefix_len, quote, triple, raw)) =
                            Self::parse_prefixed_string_start(line, idx)
                        {
                            let start = idx;
                            for _ in 0..prefix_len.saturating_sub(1) {
                                chars.next();
                            }

                            let _ = chars.next();
                            if triple {
                                chars.next();
                                chars.next();
                            }

                            let (end_idx, found_end) =
                                Self::scan_string_body(line, &mut chars, quote, triple, raw);
                            if found_end {
                                tokens.push(Token::new(TokenKind::String, start, end_idx));
                            } else {
                                tokens.push(Token::new(TokenKind::String, start, line.len()));
                                let next_state = if triple {
                                    LineState::InString(StringKind::Triple)
                                } else if quote == '"' {
                                    LineState::InString(StringKind::Double)
                                } else {
                                    LineState::InString(StringKind::Single)
                                };
                                return (tokens, next_state);
                            }
                            continue;
                        }
                    }

                    let start = idx;
                    let mut end = idx + 1;
                    while let Some(&(i, c)) = chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            chars.next();
                            end = i + 1;
                        } else {
                            break;
                        }
                    }
                    let word = &line[start..end];

                    if let Some(kind) = Self::is_keyword(word) {
                        tokens.push(Token::new(kind, start, end));
                    } else if word.chars().next().is_some_and(char::is_uppercase) {
                        tokens.push(Token::new(TokenKind::Type, start, end));
                    } else if let Some(&(_, '(')) = chars.peek() {
                        tokens.push(Token::new(TokenKind::Function, start, end));
                    } else {
                        tokens.push(Token::new(TokenKind::Identifier, start, end));
                    }
                }

                c if c.is_ascii_digit() => {
                    let start = idx;
                    let mut end = idx + 1;
                    let mut base = 10u8;

                    if c == '0' {
                        if let Some(&(_, prefix)) = chars.peek() {
                            match prefix {
                                'x' | 'X' => {
                                    chars.next();
                                    end += 1;
                                    base = 16;
                                }
                                'b' | 'B' => {
                                    chars.next();
                                    end += 1;
                                    base = 2;
                                }
                                'o' | 'O' => {
                                    chars.next();
                                    end += 1;
                                    base = 8;
                                }
                                _ => {}
                            }
                        }
                    }

                    let is_valid_digit = |ch: char, base: u8| match base {
                        2 => ch == '0' || ch == '1',
                        8 => ch.is_digit(8),
                        16 => ch.is_ascii_hexdigit(),
                        _ => ch.is_ascii_digit(),
                    };

                    while let Some(&(i, c)) = chars.peek() {
                        if c == '_' || is_valid_digit(c, base) || (base == 10 && c == '.') {
                            if c == '.' {
                                let mut temp = chars.clone();
                                temp.next();
                                if let Some((_, next_c)) = temp.peek() {
                                    if *next_c == '.' || !next_c.is_ascii_digit() {
                                        break;
                                    }
                                }
                            }
                            chars.next();
                            end = i + 1;
                        } else {
                            break;
                        }
                    }

                    if base == 10 {
                        if let Some(&(i, c)) = chars.peek() {
                            if c == 'e' || c == 'E' {
                                chars.next();
                                end = i + 1;
                                if let Some(&(j, sign)) = chars.peek() {
                                    if sign == '+' || sign == '-' {
                                        chars.next();
                                        end = j + 1;
                                    }
                                }
                                while let Some(&(k, d)) = chars.peek() {
                                    if d.is_ascii_digit() || d == '_' {
                                        chars.next();
                                        end = k + 1;
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if let Some(&(i, suffix)) = chars.peek() {
                        if suffix == 'j' || suffix == 'J' {
                            chars.next();
                            end = i + 1;
                        }
                    }

                    tokens.push(Token::new(TokenKind::Number, start, end));
                }

                '.' => {
                    if line[idx..].starts_with("...") {
                        chars.next();
                        chars.next();
                        tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 3));
                        continue;
                    }

                    if let Some(&(_, next)) = chars.peek() {
                        if next.is_ascii_digit() {
                            let start = idx;
                            let mut end = idx + 1;
                            chars.next();
                            end += 1;
                            while let Some(&(i, c)) = chars.peek() {
                                if c.is_ascii_digit() || c == '_' {
                                    chars.next();
                                    end = i + 1;
                                } else {
                                    break;
                                }
                            }
                            if let Some(&(i, c)) = chars.peek() {
                                if c == 'e' || c == 'E' {
                                    chars.next();
                                    end = i + 1;
                                    if let Some(&(j, sign)) = chars.peek() {
                                        if sign == '+' || sign == '-' {
                                            chars.next();
                                            end = j + 1;
                                        }
                                    }
                                    while let Some(&(k, d)) = chars.peek() {
                                        if d.is_ascii_digit() || d == '_' {
                                            chars.next();
                                            end = k + 1;
                                        } else {
                                            break;
                                        }
                                    }
                                }
                            }
                            tokens.push(Token::new(TokenKind::Number, start, end));
                        } else {
                            tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                    }
                }

                ':' => {
                    if let Some(&(_, '=')) = chars.peek() {
                        chars.next();
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 2));
                    } else {
                        tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                    }
                }

                '-' => {
                    if let Some(&(_, '>')) = chars.peek() {
                        chars.next();
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 2));
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                    }
                }

                '*' | '/' | '=' | '!' | '<' | '>' => {
                    let mut end = idx + 1;
                    if let Some(&(i, next)) = chars.peek() {
                        if next == '=' || next == ch {
                            chars.next();
                            end = i + 1;
                        }
                    }
                    tokens.push(Token::new(TokenKind::Operator, idx, end));
                }

                '+' | '%' | '^' | '&' | '|' | '~' => {
                    tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                }

                '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' => {
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
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
    fn test_definition_keywords() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("def class import from", LineState::Normal);

        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].kind, TokenKind::Keyword);
        assert_eq!(tokens[1].kind, TokenKind::Keyword);
        assert_eq!(tokens[2].kind, TokenKind::Keyword);
        assert_eq!(tokens[3].kind, TokenKind::Keyword);
    }

    #[test]
    fn test_control_keywords() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line(
            "if elif else for while break continue return yield pass",
            LineState::Normal,
        );

        for token in tokens {
            assert_eq!(token.kind, TokenKind::KeywordControl);
        }
    }

    #[test]
    fn test_async_keywords() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("async await", LineState::Normal);

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::KeywordModifier);
        assert_eq!(tokens[1].kind, TokenKind::KeywordModifier);
    }

    #[test]
    fn test_boolean_and_none() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("True False None", LineState::Normal);

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Boolean);
        assert_eq!(tokens[1].kind, TokenKind::Boolean);
        assert_eq!(tokens[2].kind, TokenKind::Keyword);
    }

    #[test]
    fn test_strings_and_prefixes() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line(
            "name = 'hi' \"there\" f\"f{a}\" r'raw' b\"bytes\"",
            LineState::Normal,
        );

        let strings: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::String)
            .collect();
        assert_eq!(strings.len(), 5);
    }

    #[test]
    fn test_triple_strings() {
        let tokenizer = PythonTokenizer::new();

        let (tokens, state) = tokenizer.tokenize_line("text = \"\"\"start", LineState::Normal);
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
    fn test_decorators() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("@decorator", LineState::Normal);

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Attribute);
    }

    #[test]
    fn test_numbers_and_complex() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line(
            "42 3.14 1_000 0xFF 0o77 0b1010 1e10 3+4j",
            LineState::Normal,
        );

        let numbers: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Number)
            .collect();
        assert_eq!(numbers.len(), 9);
    }

    #[test]
    fn test_comments() {
        let tokenizer = PythonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("x = 1 # comment", LineState::Normal);

        assert_eq!(tokens.last().unwrap().kind, TokenKind::Comment);
    }
}
