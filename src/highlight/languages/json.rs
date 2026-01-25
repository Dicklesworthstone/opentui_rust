use crate::highlight::token::{Token, TokenKind};
use crate::highlight::tokenizer::{CommentKind, LineState, StringKind, Tokenizer};

pub struct JsonTokenizer;

impl Default for JsonTokenizer {
    fn default() -> Self {
        Self
    }
}

impl JsonTokenizer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_block_comment(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        line_len: usize,
    ) -> (usize, bool) {
        while let Some((_idx, ch)) = chars.next() {
            if ch == '*' {
                if let Some(&(next_idx, '/')) = chars.peek() {
                    chars.next();
                    return (next_idx + 1, true);
                }
            }
        }

        (line_len, false)
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_string_tokens(
        line: &str,
        start: usize,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    ) -> (Vec<Token>, usize, bool) {
        let mut tokens = Vec::new();
        let mut segment_start = start;

        while let Some((idx, ch)) = chars.next() {
            match ch {
                '\\' => {
                    if segment_start < idx {
                        tokens.push(Token::new(TokenKind::String, segment_start, idx));
                    }

                    if let Some(&(next_idx, next_ch)) = chars.peek() {
                        if matches!(next_ch, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't') {
                            chars.next();
                            let end_idx = next_idx + next_ch.len_utf8();
                            tokens.push(Token::new(TokenKind::StringEscape, idx, end_idx));
                            segment_start = end_idx;
                            continue;
                        }
                        if next_ch == 'u' {
                            chars.next();
                            let mut end_idx = next_idx + 1;
                            let mut ok = true;
                            for _ in 0..4 {
                                if let Some((hex_idx, hex_ch)) = chars.next() {
                                    if hex_ch.is_ascii_hexdigit() {
                                        end_idx = hex_idx + 1;
                                    } else {
                                        ok = false;
                                    }
                                } else {
                                    ok = false;
                                    break;
                                }
                            }
                            let kind = if ok {
                                TokenKind::StringEscape
                            } else {
                                TokenKind::Error
                            };
                            tokens.push(Token::new(kind, idx, end_idx));
                            segment_start = end_idx;
                            continue;
                        }

                        chars.next();
                        let end_idx = next_idx + next_ch.len_utf8();
                        tokens.push(Token::new(TokenKind::Error, idx, end_idx));
                        segment_start = end_idx;
                        continue;
                    }

                    tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                    return (tokens, line.len(), false);
                }
                '"' => {
                    tokens.push(Token::new(TokenKind::String, segment_start, idx + 1));
                    return (tokens, idx + 1, true);
                }
                _ => {}
            }
        }

        if segment_start < line.len() {
            tokens.push(Token::new(TokenKind::String, segment_start, line.len()));
        }

        (tokens, line.len(), false)
    }

    fn scan_number(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        start: usize,
        first: char,
    ) -> usize {
        let mut end = start + first.len_utf8();

        while let Some(&(i, c)) = chars.peek() {
            if c.is_ascii_digit() {
                chars.next();
                end = i + 1;
            } else {
                break;
            }
        }

        if let Some(&(i, '.')) = chars.peek() {
            let mut temp = chars.clone();
            temp.next();
            if let Some((_, next_c)) = temp.peek() {
                if next_c.is_ascii_digit() {
                    chars.next();
                    end = i + 1;
                    while let Some(&(j, c)) = chars.peek() {
                        if c.is_ascii_digit() {
                            chars.next();
                            end = j + 1;
                        } else {
                            break;
                        }
                    }
                }
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
                    if d.is_ascii_digit() {
                        chars.next();
                        end = k + 1;
                    } else {
                        break;
                    }
                }
            }
        }

        end
    }
}

impl Tokenizer for JsonTokenizer {
    fn name(&self) -> &'static str {
        "JSON"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["json", "jsonc", "json5"]
    }

    #[allow(clippy::too_many_lines)]
    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        let mut tokens = Vec::new();
        let mut chars = line.char_indices().peekable();

        match state {
            LineState::InComment(kind) => {
                let token_kind = if kind == CommentKind::Doc {
                    TokenKind::CommentDoc
                } else {
                    TokenKind::CommentBlock
                };
                let (end_idx, found_end) = Self::scan_block_comment(&mut chars, line.len());
                if found_end {
                    tokens.push(Token::new(token_kind, 0, end_idx));
                } else {
                    tokens.push(Token::new(token_kind, 0, line.len()));
                    return (tokens, LineState::InComment(kind));
                }
            }
            LineState::InString(StringKind::Double) => {
                let (mut string_tokens, end_idx, found_end) =
                    Self::scan_string_tokens(line, 0, &mut chars);
                if found_end {
                    tokens.append(&mut string_tokens);
                    if end_idx < line.len() {
                        // continue scanning remainder
                    } else {
                        return (tokens, LineState::Normal);
                    }
                } else {
                    tokens.append(&mut string_tokens);
                    return (tokens, LineState::InString(StringKind::Double));
                }
            }
            _ => {}
        }

        while let Some((idx, ch)) = chars.next() {
            if ch.is_whitespace() {
                continue;
            }

            match ch {
                '/' => {
                    if let Some(&(_, '/')) = chars.peek() {
                        tokens.push(Token::new(TokenKind::Comment, idx, line.len()));
                        break;
                    }
                    if let Some(&(_, '*')) = chars.peek() {
                        chars.next();
                        let (end_idx, found_end) = Self::scan_block_comment(&mut chars, line.len());
                        let token_kind = if line[idx..].starts_with("/**") {
                            TokenKind::CommentDoc
                        } else {
                            TokenKind::CommentBlock
                        };
                        if found_end {
                            tokens.push(Token::new(token_kind, idx, end_idx));
                        } else {
                            tokens.push(Token::new(token_kind, idx, line.len()));
                            let next_kind = if token_kind == TokenKind::CommentDoc {
                                CommentKind::Doc
                            } else {
                                CommentKind::Block
                            };
                            return (tokens, LineState::InComment(next_kind));
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                    }
                }

                '"' => {
                    let (mut string_tokens, end_idx, found_end) =
                        Self::scan_string_tokens(line, idx, &mut chars);
                    if found_end {
                        let is_key = line[end_idx..]
                            .chars()
                            .find(|c| !c.is_whitespace())
                            .is_some_and(|c| c == ':');
                        if is_key {
                            for token in &mut string_tokens {
                                if token.kind == TokenKind::String {
                                    token.kind = TokenKind::Identifier;
                                }
                            }
                        }
                        tokens.append(&mut string_tokens);
                    } else {
                        tokens.append(&mut string_tokens);
                        return (tokens, LineState::InString(StringKind::Double));
                    }
                }

                '-' => {
                    if let Some(&(_, next)) = chars.peek() {
                        if next.is_ascii_digit() {
                            chars.next();
                            let end = Self::scan_number(&mut chars, idx, next);
                            tokens.push(Token::new(TokenKind::Number, idx, end));
                        } else {
                            tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                    }
                }

                c if c.is_ascii_digit() => {
                    let end = Self::scan_number(&mut chars, idx, c);
                    tokens.push(Token::new(TokenKind::Number, idx, end));
                }

                '{' | '}' | '[' | ']' => {
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                }
                ':' | ',' => {
                    tokens.push(Token::new(TokenKind::Delimiter, idx, idx + 1));
                }

                't' => {
                    if line[idx..].starts_with("true") {
                        tokens.push(Token::new(TokenKind::Boolean, idx, idx + 4));
                        for _ in 0..3 {
                            chars.next();
                        }
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
                    } else {
                        tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                    }
                }
                'n' => {
                    if line[idx..].starts_with("null") {
                        tokens.push(Token::new(TokenKind::Constant, idx, idx + 4));
                        for _ in 0..3 {
                            chars.next();
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
                    }
                }

                _ => {
                    tokens.push(Token::new(TokenKind::Error, idx, idx + 1));
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
    fn test_json_strings_and_keys() {
        let tokenizer = JsonTokenizer::new();
        let line = "\"name\": \"bob\"";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        let key = tokens
            .iter()
            .find(|token| token.kind == TokenKind::Identifier)
            .expect("key token");
        assert_eq!(&line[key.range()], "\"name\"");

        let value = tokens
            .iter()
            .find(|token| token.kind == TokenKind::String)
            .expect("value token");
        assert_eq!(&line[value.range()], "\"bob\"");
    }

    #[test]
    fn test_json_numbers() {
        let tokenizer = JsonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("0 -1 3.14 1e10 1E-2", LineState::Normal);

        let numbers: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Number)
            .collect();
        assert_eq!(numbers.len(), 5);
    }

    #[test]
    fn test_json_booleans_null() {
        let tokenizer = JsonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("true false null", LineState::Normal);

        assert_eq!(tokens[0].kind, TokenKind::Boolean);
        assert_eq!(tokens[1].kind, TokenKind::Boolean);
        assert_eq!(tokens[2].kind, TokenKind::Constant);
    }

    #[test]
    fn test_json_structure() {
        let tokenizer = JsonTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("{ } [ ] : ,", LineState::Normal);

        let punct: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Punctuation)
            .collect();
        let delim: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Delimiter)
            .collect();

        assert_eq!(punct.len(), 4);
        assert_eq!(delim.len(), 2);
    }

    #[test]
    fn test_json_escapes() {
        let tokenizer = JsonTokenizer::new();
        let line = "\"a\\n\\u0041\"";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        let escapes: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::StringEscape)
            .collect();
        assert_eq!(escapes.len(), 2);
    }

    #[test]
    fn test_jsonc_comments() {
        let tokenizer = JsonTokenizer::new();

        let (tokens, _) = tokenizer.tokenize_line("// comment", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::Comment);

        let (tokens, _) = tokenizer.tokenize_line("/* block */", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::CommentBlock);
    }

    #[test]
    fn test_json_invalid_escape() {
        let tokenizer = JsonTokenizer::new();
        let line = "\"bad\\q\"";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        assert!(tokens.iter().any(|token| token.kind == TokenKind::Error));
    }
}
