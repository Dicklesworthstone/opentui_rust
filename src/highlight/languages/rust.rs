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

        // Resume state from previous line
        match state {
            LineState::InComment(CommentKind::Block) => {
                // Find end of block comment */
                let mut last_idx = 0;
                let mut found_end = false;
                while let Some((idx, ch)) = chars.next() {
                    last_idx = idx;
                    if ch == '*' {
                        if let Some(&(_, '/')) = chars.peek() {
                            chars.next(); // consume '/'
                            last_idx += 1;
                            found_end = true;
                            break;
                        }
                    }
                }
                if found_end {
                    tokens.push(Token::new(TokenKind::CommentBlock, 0, last_idx + 1));
                } else {
                    tokens.push(Token::new(TokenKind::CommentBlock, 0, line.len()));
                    return (tokens, LineState::InComment(CommentKind::Block));
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
                let mut last_idx = 0;
                let mut found_end = false;
                
                // We need to find '"' followed by `hashes` '#' characters
                while let Some((idx, ch)) = chars.next() {
                    last_idx = idx;
                    if ch == '"' {
                        // Check if followed by `hashes` '#'
                        let mut match_hashes = true;
                        let remaining = &line[idx+1..];
                        if remaining.len() >= hashes as usize {
                            for h in remaining.chars().take(hashes as usize) {
                                if h != '#' {
                                    match_hashes = false;
                                    break;
                                }
                            }
                            if match_hashes {
                                // Found it, consume hashes
                                for _ in 0..hashes {
                                    chars.next();
                                }
                                last_idx += hashes as usize;
                                found_end = true;
                                break;
                            }
                        }
                    }
                }

                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, last_idx + 1));
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
                        let kind = if line[idx..].starts_with("///") || line[idx..].starts_with("//!") {
                            TokenKind::CommentDoc
                        } else {
                            TokenKind::Comment
                        };
                        tokens.push(Token::new(kind, idx, line.len()));
                        break; // Rest of line is comment
                    } else if let Some(&(_, '*')) = chars.peek() {
                        // Block comment /*
                        chars.next(); // consume '*'
                        // Look for ending */
                        let mut end_idx = idx + 2;
                        let mut found_end = false;
                        
                        let mut star_seen = false;
                        while let Some((i, c)) = chars.next() {
                            end_idx = i + 1;
                            if star_seen && c == '/' {
                                found_end = true;
                                break;
                            }
                            star_seen = c == '*';
                        }

                        if found_end {
                            tokens.push(Token::new(TokenKind::CommentBlock, idx, end_idx));
                        } else {
                            tokens.push(Token::new(TokenKind::CommentBlock, idx, line.len()));
                            return (tokens, LineState::InComment(CommentKind::Block));
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                    }
                }

                // Raw string r" or r#"
                'r' => {
                    // Check if followed by " or #
                    if let Some(&(i, next_c)) = chars.peek() {
                        if next_c == '"' {
                            // r"..."
                            chars.next(); // consume "
                            let start = idx;
                            let mut end = i + 1;
                            let mut complete = false;
                            
                            while let Some((j, c)) = chars.next() {
                                end = j + 1;
                                if c == '"' {
                                    complete = true;
                                    break;
                                }
                            }
                            if complete {
                                tokens.push(Token::new(TokenKind::String, start, end));
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
                                
                                // Now inside raw string
                                let mut end = idx + 1 + temp_hashes + 1; // r + hashes + "
                                let mut found_end = false;
                                
                                // Look for " followed by temp_hashes #
                                while let Some((k, c)) = chars.next() {
                                    end = k + 1;
                                    if c == '"' {
                                        let mut match_hashes = true;
                                        // Need to check hashes
                                        let remaining = &line[k+1..];
                                        if remaining.len() >= temp_hashes {
                                            for h in remaining.chars().take(temp_hashes) {
                                                if h != '#' {
                                                    match_hashes = false;
                                                    break;
                                                }
                                            }
                                            if match_hashes {
                                                // Found end
                                                for _ in 0..temp_hashes {
                                                    chars.next();
                                                }
                                                end += temp_hashes;
                                                found_end = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                                
                                if found_end {
                                    tokens.push(Token::new(TokenKind::String, start, end));
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
                    let mut is_hex = false;
                    if c == '0' {
                        if let Some(&(_, 'x' | 'b' | 'o')) = chars.peek() {
                            chars.next();
                            end += 1;
                            is_hex = true;
                        }
                    }

                    while let Some(&(i, c)) = chars.peek() {
                        if c.is_ascii_digit() || c == '_' || (is_hex && c.is_ascii_hexdigit()) || c == '.' {
                            if c == '.' {
                                let mut temp = chars.clone();
                                temp.next(); // skip .
                                if let Some((_, next_c)) = temp.peek() {
                                    if *next_c == '.' {
                                        break;
                                    }
                                    if !next_c.is_ascii_digit() && !is_hex {
                                        break;
                                    }
                                }
                            }
                            chars.next();
                            end = i + 1;
                        } else {
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
                            break;
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
                    } else {
                        tokens.push(Token::new(TokenKind::Lifetime, start, end));
                    }
                }

                // Operators and Punctuation
                ';' | ',' | '.' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '{' | '}' => {
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                }
                '+' | '-' | '*' | '%' | '^' | '&' | '|' | '=' | '<' | '>' => {
                    tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                }
                
                // Attributes #
                '#' => {
                    let start = idx;
                    if let Some(&(_, c)) = chars.peek() {
                        if c == '[' {
                            tokens.push(Token::new(TokenKind::Attribute, start, start + 1));
                        } else if c == '!' {
                            chars.next(); // consume !
                            if let Some(&(_, '[')) = chars.peek() {
                                tokens.push(Token::new(TokenKind::Attribute, start, start + 2));
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

                _ => {
                    tokens.push(Token::new(TokenKind::Text, idx, idx + 1));
                }
            }
        }

        (tokens, LineState::Normal)
    }
}