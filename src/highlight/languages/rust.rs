use crate::highlight::token::{Token, TokenKind};
use crate::highlight::tokenizer::{CommentKind, LineState, StringKind, Tokenizer};

pub struct RustTokenizer;

impl Default for RustTokenizer {
    fn default() -> Self {
        Self
    }
}

impl RustTokenizer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn is_keyword(word: &str) -> Option<TokenKind> {
        match word {
            // Control
            "if" | "else" | "match" | "loop" | "while" | "for" | "break" | "continue"
            | "return" => Some(TokenKind::KeywordControl),

            // Definitions and other keywords
            "fn" | "let" | "const" | "static" | "struct" | "enum" | "trait" | "impl" | "type"
            | "mod" | "use" | "crate" | "self" | "Self" | "super" | "where" | "as" | "in" => {
                Some(TokenKind::Keyword)
            }

            // Modifiers
            "pub" | "mut" | "ref" | "move" | "async" | "await" | "unsafe" | "extern" | "dyn" => {
                Some(TokenKind::KeywordModifier)
            }

            // Types (primitive)
            "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32"
            | "i64" | "i128" | "isize" | "f32" | "f64" | "char" | "str" | "String" | "Vec"
            | "Option" | "Result" => Some(TokenKind::KeywordType),

            // Values
            "true" | "false" => Some(TokenKind::Boolean),

            _ => None,
        }
    }

    fn scan_block_comment(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        mut depth: u8,
        track_nested: bool,
    ) -> (usize, u8, bool) {
        let mut end_idx = 0;
        let mut found_end = false;

        while let Some((idx, ch)) = chars.next() {
            end_idx = idx + ch.len_utf8();

            if ch == '/' && track_nested {
                if let Some(&(next_idx, '*')) = chars.peek() {
                    chars.next();
                    depth = depth.saturating_add(1);
                    end_idx = next_idx + 1;
                    continue;
                }
            }

            if ch == '*' {
                if let Some(&(next_idx, '/')) = chars.peek() {
                    chars.next();
                    end_idx = next_idx + 1;
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        found_end = true;
                        break;
                    }
                }
            }
        }

        (end_idx, depth, found_end)
    }

    fn scan_raw_string_end(
        line: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        hashes: u8,
    ) -> (usize, bool) {
        let mut end_idx = 0;
        let mut found_end = false;

        while let Some((idx, ch)) = chars.next() {
            end_idx = idx + ch.len_utf8();
            if ch == '"' {
                let remaining = &line[idx + 1..];
                let mut match_hashes = true;
                if remaining.len() >= hashes as usize {
                    for h in remaining.chars().take(hashes as usize) {
                        if h != '#' {
                            match_hashes = false;
                            break;
                        }
                    }
                    if match_hashes {
                        for _ in 0..hashes {
                            chars.next();
                        }
                        end_idx = idx + 1 + hashes as usize;
                        found_end = true;
                        break;
                    }
                }
            }
        }

        (end_idx, found_end)
    }
}

impl Tokenizer for RustTokenizer {
    fn name(&self) -> &'static str {
        "Rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    #[allow(clippy::too_many_lines, clippy::while_let_on_iterator)]
    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        let mut tokens = Vec::new();
        let mut chars = line.char_indices().peekable();
        let mut in_attribute = false;

        // Resume state from previous line
        match state {
            LineState::InComment(kind) => {
                let (doc, depth) = match kind {
                    CommentKind::Doc => (true, 1),
                    CommentKind::Nested(depth) => (false, depth.max(1)),
                    CommentKind::Block => (false, 1),
                };

                let (end_idx, depth, found_end) = Self::scan_block_comment(&mut chars, depth, true);
                let token_kind = if doc {
                    TokenKind::CommentDoc
                } else {
                    TokenKind::CommentBlock
                };

                if found_end {
                    tokens.push(Token::new(token_kind, 0, end_idx));
                } else {
                    tokens.push(Token::new(token_kind, 0, line.len()));
                    let next_kind = if doc && depth == 1 {
                        CommentKind::Doc
                    } else if depth > 1 {
                        CommentKind::Nested(depth)
                    } else {
                        CommentKind::Block
                    };
                    return (tokens, LineState::InComment(next_kind));
                }
            }
            LineState::InString(StringKind::Double) => {
                // Resume normal string
                let mut last_idx = 0;
                let mut escaped = false;
                let mut found_end = false;
                while let Some((idx, ch)) = chars.next() {
                    last_idx = idx;
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        found_end = true;
                        break;
                    }
                }
                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, last_idx + 1));
                } else {
                    tokens.push(Token::new(TokenKind::String, 0, line.len()));
                    return (tokens, LineState::InString(StringKind::Double));
                }
            }
            LineState::InRawString(hashes) => {
                let (end_idx, found_end) = Self::scan_raw_string_end(line, &mut chars, hashes);
                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, end_idx));
                } else {
                    tokens.push(Token::new(TokenKind::String, 0, line.len()));
                    return (tokens, LineState::InRawString(hashes));
                }
            }
            _ => {}
        }

        while let Some((idx, ch)) = chars.next() {
            match ch {
                // Whitespace
                ch if ch.is_whitespace() => {
                    // Skip
                }

                // Comments
                '/' => {
                    if let Some(&(_, '/')) = chars.peek() {
                        // Line comment //
                        let kind =
                            if line[idx..].starts_with("///") || line[idx..].starts_with("//!") {
                                TokenKind::CommentDoc
                            } else {
                                TokenKind::Comment
                            };
                        tokens.push(Token::new(kind, idx, line.len()));
                        break; // Rest of line is comment
                    } else if let Some(&(_, '*')) = chars.peek() {
                        // Block comment /*
                        let is_doc =
                            line[idx..].starts_with("/**") || line[idx..].starts_with("/*!");
                        chars.next(); // consume '*'
                        let (end_idx, depth, found_end) =
                            Self::scan_block_comment(&mut chars, 1, true);
                        let token_kind = if is_doc {
                            TokenKind::CommentDoc
                        } else {
                            TokenKind::CommentBlock
                        };

                        if found_end {
                            tokens.push(Token::new(token_kind, idx, end_idx));
                        } else {
                            tokens.push(Token::new(token_kind, idx, line.len()));
                            let next_kind = if is_doc && depth == 1 {
                                CommentKind::Doc
                            } else if depth > 1 {
                                CommentKind::Nested(depth)
                            } else {
                                CommentKind::Block
                            };
                            return (tokens, LineState::InComment(next_kind));
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                    }
                }

                // Byte strings/bytes/byte raw strings
                'b' => {
                    if let Some(&(i, next_c)) = chars.peek() {
                        if next_c == '"' {
                            // b"..."
                            chars.next(); // consume "
                            let start = idx;
                            let mut end = i + 1;
                            let mut escaped = false;
                            let mut complete = false;

                            while let Some((j, c)) = chars.next() {
                                end = j + 1;
                                if escaped {
                                    escaped = false;
                                } else if c == '\\' {
                                    escaped = true;
                                } else if c == '"' {
                                    complete = true;
                                    break;
                                }
                            }

                            if complete {
                                tokens.push(Token::new(TokenKind::String, start, end));
                            } else {
                                tokens.push(Token::new(TokenKind::String, start, line.len()));
                                return (tokens, LineState::InString(StringKind::Double));
                            }
                            continue;
                        } else if next_c == '\'' {
                            // b'a'
                            chars.next(); // consume '
                            let start = idx;
                            let mut end = i + 1;
                            let mut escaped = false;
                            let mut complete = false;

                            while let Some((j, c)) = chars.next() {
                                end = j + 1;
                                if escaped {
                                    escaped = false;
                                } else if c == '\\' {
                                    escaped = true;
                                } else if c == '\'' {
                                    complete = true;
                                    break;
                                }
                            }

                            if complete {
                                tokens.push(Token::new(TokenKind::String, start, end));
                            } else {
                                tokens.push(Token::new(TokenKind::String, start, line.len()));
                            }
                            continue;
                        } else if next_c == 'r' {
                            let mut temp_cursor = line[i + 1..].chars();
                            let mut hashes = 0usize;
                            let mut is_raw = false;

                            while let Some(c) = temp_cursor.next() {
                                if c == '#' {
                                    hashes += 1;
                                } else if c == '"' {
                                    is_raw = true;
                                    break;
                                } else {
                                    break;
                                }
                            }

                            if is_raw {
                                let start = idx;
                                chars.next(); // consume 'r'
                                for _ in 0..hashes {
                                    chars.next();
                                }
                                chars.next(); // consume opening "
                                let (end_idx, found_end) =
                                    Self::scan_raw_string_end(line, &mut chars, hashes as u8);
                                if found_end {
                                    tokens.push(Token::new(TokenKind::String, start, end_idx));
                                } else {
                                    tokens.push(Token::new(TokenKind::String, start, line.len()));
                                    return (tokens, LineState::InRawString(hashes as u8));
                                }
                                continue;
                            }
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
                    } else if in_attribute {
                        tokens.push(Token::new(TokenKind::Identifier, start, end));
                    } else if let Some(&(_, '(')) = chars.peek() {
                        tokens.push(Token::new(TokenKind::Function, start, end));
                    } else if let Some(&(_, '!')) = chars.peek() {
                        tokens.push(Token::new(TokenKind::Macro, start, end));
                    } else {
                        tokens.push(Token::new(TokenKind::Identifier, start, end));
                    }
                }

                // Raw string r" or r#"
                'r' => {
                    // Check if followed by " or #
                    if let Some(&(_, next_c)) = chars.peek() {
                        if next_c == '"' {
                            // r"..."
                            chars.next(); // consume "
                            let start = idx;
                            let (end_idx, found_end) =
                                Self::scan_raw_string_end(line, &mut chars, 0);
                            if found_end {
                                tokens.push(Token::new(TokenKind::String, start, end_idx));
                            } else {
                                tokens.push(Token::new(TokenKind::String, start, line.len()));
                                return (tokens, LineState::InRawString(0));
                            }
                        } else if next_c == '#' {
                            // r#... or r###...
                            let start = idx;

                            // Peek ahead to count hashes and find quote
                            #[allow(clippy::unused_peekable)]
                            let mut temp_cursor = chars.clone();
                            let mut temp_hashes = 0;
                            let mut is_raw_string = false;

                            while let Some((_, c)) = temp_cursor.next() {
                                if c == '#' {
                                    temp_hashes += 1;
                                } else if c == '"' {
                                    is_raw_string = true;
                                    break;
                                } else {
                                    // Not a raw string (e.g. r#ident)
                                    break;
                                }
                            }

                            if is_raw_string {
                                // Consume the hashes and quote
                                for _ in 0..temp_hashes {
                                    chars.next();
                                }
                                chars.next(); // consume "

                                let (end_idx, found_end) =
                                    Self::scan_raw_string_end(line, &mut chars, temp_hashes as u8);

                                if found_end {
                                    tokens.push(Token::new(TokenKind::String, start, end_idx));
                                } else {
                                    tokens.push(Token::new(TokenKind::String, start, line.len()));
                                    return (tokens, LineState::InRawString(temp_hashes as u8));
                                }
                            } else {
                                // Raw identifier r#ident
                                // Consume #
                                chars.next();
                                // Now consume identifier part
                                let mut end = idx + 2;
                                while let Some(&(i, c)) = chars.peek() {
                                    if c.is_alphanumeric() || c == '_' {
                                        chars.next();
                                        end = i + 1;
                                    } else {
                                        break;
                                    }
                                }
                                // It is an identifier (keyword check?)
                                tokens.push(Token::new(TokenKind::Identifier, idx, end));
                            }
                        } else {
                            // Just 'r' followed by something else (e.g. r_ident)
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
                            } else {
                                tokens.push(Token::new(TokenKind::Identifier, start, end));
                            }
                        }
                    } else {
                        // End of line, just 'r'
                        tokens.push(Token::new(TokenKind::Identifier, idx, idx + 1));
                    }
                }

                // Identifiers and Keywords
                c if c.is_alphabetic() || c == '_' => {
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
                    } else if in_attribute {
                        tokens.push(Token::new(TokenKind::Identifier, start, end));
                    } else if let Some(&(_, '(')) = chars.peek() {
                        tokens.push(Token::new(TokenKind::Function, start, end));
                    } else if let Some(&(_, '!')) = chars.peek() {
                        tokens.push(Token::new(TokenKind::Macro, start, end));
                    } else {
                        tokens.push(Token::new(TokenKind::Identifier, start, end));
                    }
                }

                // Numeric Literals
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
                                temp.next(); // skip .
                                if let Some((_, next_c)) = temp.peek() {
                                    if *next_c == '.' {
                                        break;
                                    }
                                    if !next_c.is_ascii_digit() {
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

                    if let Some(&(i, c)) = chars.peek() {
                        if c.is_ascii_alphabetic() {
                            chars.next();
                            end = i + 1;
                            while let Some(&(j, s)) = chars.peek() {
                                if s.is_alphanumeric() {
                                    chars.next();
                                    end = j + 1;
                                } else {
                                    break;
                                }
                            }
                        }
                    }

                    tokens.push(Token::new(TokenKind::Number, start, end));
                }

                // Strings
                '"' => {
                    let start = idx;
                    let mut end = idx + 1;
                    let mut escaped = false;
                    let mut complete = false;

                    while let Some((i, c)) = chars.next() {
                        end = i + 1;
                        if escaped {
                            escaped = false;
                        } else if c == '\\' {
                            escaped = true;
                        } else if c == '"' {
                            complete = true;
                            break;
                        }
                    }

                    if complete {
                        tokens.push(Token::new(TokenKind::String, start, end));
                    } else {
                        tokens.push(Token::new(TokenKind::String, start, line.len()));
                        return (tokens, LineState::InString(StringKind::Double));
                    }
                }

                // Char literals 'a' or lifetimes 'a
                '\'' => {
                    let start = idx;
                    let mut content_len = 0;
                    let mut end = idx + 1;
                    let mut terminated = false;

                    while let Some(&(i, c)) = chars.peek() {
                        if c == '\'' && content_len > 0 {
                            chars.next();
                            end = i + 1;
                            terminated = true;
                            break;
                        }
                        if !c.is_alphanumeric() && c != '_' && c != '\\' {
                            break;
                        }
                        chars.next();
                        content_len += 1;
                        end = i + 1;
                    }

                    if terminated {
                        tokens.push(Token::new(TokenKind::String, start, end));
                    } else if matches!(chars.peek(), Some((_, ':'))) {
                        tokens.push(Token::new(TokenKind::Label, start, end));
                    } else {
                        tokens.push(Token::new(TokenKind::Lifetime, start, end));
                    }
                }

                // Operators and Punctuation
                ';' | ',' | '.' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '{' | '}' => {
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                    if ch == ']' && in_attribute {
                        in_attribute = false;
                    }
                }
                '+' | '-' | '*' | '%' | '^' | '&' | '|' | '=' | '<' | '>' => {
                    tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                }

                // Attributes #
                '#' => {
                    let start = idx;
                    if let Some(&(_, c)) = chars.peek() {
                        if c == '[' {
                            chars.next(); // consume '['
                            tokens.push(Token::new(TokenKind::Attribute, start, start + 2));
                            in_attribute = true;
                        } else if c == '!' {
                            chars.next(); // consume !
                            if let Some(&(_, '[')) = chars.peek() {
                                chars.next(); // consume '['
                                tokens.push(Token::new(TokenKind::Attribute, start, start + 3));
                                in_attribute = true;
                            } else {
                                tokens.push(Token::new(TokenKind::Operator, start, start + 2));
                            }
                        } else {
                            tokens.push(Token::new(TokenKind::Operator, start, start + 1));
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, start, start + 1));
                    }
                }

                // Macro variables ($ident)
                '$' => {
                    let start = idx;
                    let mut end = idx + 1;
                    if let Some(&(i, c)) = chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            chars.next();
                            end = i + 1;
                            while let Some(&(j, s)) = chars.peek() {
                                if s.is_alphanumeric() || s == '_' {
                                    chars.next();
                                    end = j + 1;
                                } else {
                                    break;
                                }
                            }
                            tokens.push(Token::new(TokenKind::Macro, start, end));
                        } else {
                            tokens.push(Token::new(TokenKind::Operator, start, end));
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, start, end));
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
    fn test_keywords() {
        let tokenizer = RustTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("fn let if else match loop", LineState::Normal);

        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::Keyword); // fn
        assert_eq!(tokens[1].kind, TokenKind::Keyword); // let
        assert_eq!(tokens[2].kind, TokenKind::KeywordControl); // if
        assert_eq!(tokens[3].kind, TokenKind::KeywordControl); // else
        assert_eq!(tokens[4].kind, TokenKind::KeywordControl); // match
        assert_eq!(tokens[5].kind, TokenKind::KeywordControl); // loop
    }

    #[test]
    fn test_types() {
        let tokenizer = RustTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("bool u8 String Option", LineState::Normal);

        assert_eq!(tokens[0].kind, TokenKind::KeywordType); // bool
        assert_eq!(tokens[1].kind, TokenKind::KeywordType); // u8
        assert_eq!(tokens[2].kind, TokenKind::KeywordType); // String
        assert_eq!(tokens[3].kind, TokenKind::KeywordType); // Option
    }

    #[test]
    fn test_literals() {
        let tokenizer = RustTokenizer::new();
        let (tokens, _) =
            tokenizer.tokenize_line("123 0xFF 0b1010 1e-3 'a' \"hello\"", LineState::Normal);

        assert_eq!(tokens[0].kind, TokenKind::Number);
        assert_eq!(tokens[1].kind, TokenKind::Number);
        assert_eq!(tokens[2].kind, TokenKind::Number);
        assert_eq!(tokens[3].kind, TokenKind::Number);
        assert_eq!(tokens[4].kind, TokenKind::String); // 'a' char
        assert_eq!(tokens[5].kind, TokenKind::String); // "hello"
    }

    #[test]
    fn test_comments() {
        let tokenizer = RustTokenizer::new();

        let (tokens, _) = tokenizer.tokenize_line("// line comment", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::Comment);

        let (tokens, _) = tokenizer.tokenize_line("/// doc comment", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::CommentDoc);

        let (tokens, _) = tokenizer.tokenize_line("/* block */", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::CommentBlock);

        let (tokens, _) = tokenizer.tokenize_line("/** doc block */", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::CommentDoc);
    }

    #[test]
    fn test_nested_block_comments() {
        let tokenizer = RustTokenizer::new();

        let (tokens, state) = tokenizer.tokenize_line("/* outer /* inner", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::CommentBlock);
        assert_eq!(state, LineState::InComment(CommentKind::Nested(2)));

        let (tokens, state) = tokenizer.tokenize_line("still */ end */", state);
        assert_eq!(tokens[0].kind, TokenKind::CommentBlock);
        assert_eq!(state, LineState::Normal);
    }

    #[test]
    fn test_attributes() {
        let tokenizer = RustTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("#[derive(Debug)]", LineState::Normal);

        assert_eq!(tokens[0].kind, TokenKind::Attribute); // #[
        assert_eq!(tokens[1].kind, TokenKind::Identifier); // derive
    }

    #[test]
    fn test_lifetimes() {
        let tokenizer = RustTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("'a 'static", LineState::Normal);

        assert_eq!(tokens[0].kind, TokenKind::Lifetime);
        assert_eq!(tokens[1].kind, TokenKind::Lifetime);
    }

    #[test]
    fn test_labels_and_macro_vars() {
        let tokenizer = RustTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("'label: $var", LineState::Normal);

        assert_eq!(tokens[0].kind, TokenKind::Label);
        assert_eq!(tokens[2].kind, TokenKind::Macro);
    }

    #[test]
    fn test_byte_strings() {
        let tokenizer = RustTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("b\"hi\" b'a' br#\"raw\"#", LineState::Normal);

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(tokens[1].kind, TokenKind::String);
        assert_eq!(tokens[2].kind, TokenKind::String);
    }

    #[test]
    fn test_multi_line_string() {
        let tokenizer = RustTokenizer::new();

        let (tokens, state) = tokenizer.tokenize_line("let x = \"start", LineState::Normal);
        assert_eq!(tokens.last().unwrap().kind, TokenKind::String);
        assert_eq!(state, LineState::InString(StringKind::Double));

        let (tokens, state) = tokenizer.tokenize_line("middle", state);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(state, LineState::InString(StringKind::Double));

        let (tokens, state) = tokenizer.tokenize_line("end\";", state);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(state, LineState::Normal);
    }

    #[test]
    fn test_raw_string() {
        let tokenizer = RustTokenizer::new();

        let (tokens, state) = tokenizer.tokenize_line("r#\"raw string\"#", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(state, LineState::Normal);

        let (_tokens, state) = tokenizer.tokenize_line("r##\"multi", LineState::Normal);
        assert_eq!(state, LineState::InRawString(2));

        let (_tokens, state) = tokenizer.tokenize_line("line\"##", state);
        assert_eq!(state, LineState::Normal);
    }
}
