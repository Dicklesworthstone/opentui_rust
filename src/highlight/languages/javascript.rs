use crate::highlight::token::{Token, TokenKind};
use crate::highlight::tokenizer::{CommentKind, LineState, StringKind, Tokenizer};

pub struct JavaScriptTokenizer {
    typescript_mode: bool,
}

impl JavaScriptTokenizer {
    #[must_use]
    pub fn javascript() -> Self {
        Self {
            typescript_mode: false,
        }
    }

    #[must_use]
    pub fn typescript() -> Self {
        Self {
            typescript_mode: true,
        }
    }

    fn is_typescript_keyword(word: &str) -> Option<TokenKind> {
        match word {
            "interface" | "type" | "enum" | "namespace" | "module" | "keyof" | "infer"
            | "never" | "unknown" | "any" => Some(TokenKind::KeywordType),
            "declare" | "readonly" | "abstract" | "private" | "protected" | "public" | "static"
            | "override" => Some(TokenKind::KeywordModifier),
            "implements" | "is" => Some(TokenKind::Keyword),
            _ => None,
        }
    }

    fn is_keyword(word: &str, typescript: bool) -> Option<TokenKind> {
        let kind = match word {
            // Control flow
            "if" | "else" | "switch" | "case" | "default" | "for" | "while" | "do" | "break"
            | "continue" | "return" | "throw" | "try" | "catch" | "finally" => {
                Some(TokenKind::KeywordControl)
            }

            // Definitions
            "function" | "var" | "let" | "const" | "class" | "extends" | "import" | "export"
            | "from" | "as" | "this" | "super" => Some(TokenKind::Keyword),

            // Operator keywords
            "typeof" | "instanceof" | "in" | "of" | "new" | "delete" | "void" => {
                Some(TokenKind::Keyword)
            }

            // Modifiers
            "async" | "await" => Some(TokenKind::KeywordModifier),

            // Values
            "true" | "false" => Some(TokenKind::Boolean),
            "null" | "undefined" | "NaN" | "Infinity" => Some(TokenKind::Constant),

            _ => None,
        };

        if kind.is_some() {
            return kind;
        }

        if typescript {
            return Self::is_typescript_keyword(word);
        }

        None
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
    fn scan_string(
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
                return (idx + ch.len_utf8(), true);
            }
        }

        (line_len, false)
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_regex(
        line: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    ) -> (usize, bool) {
        let mut escaped = false;
        let mut in_class = false;

        while let Some((idx, ch)) = chars.next() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '[' {
                in_class = true;
                continue;
            }
            if ch == ']' && in_class {
                in_class = false;
                continue;
            }
            if ch == '/' && !in_class {
                let mut end_idx = idx + ch.len_utf8();
                while let Some(&(flag_idx, flag)) = chars.peek() {
                    if flag.is_ascii_alphabetic() {
                        chars.next();
                        end_idx = flag_idx + 1;
                    } else {
                        break;
                    }
                }
                return (end_idx, true);
            }
        }

        (line.len(), false)
    }

    fn scan_number(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        start: usize,
        first: char,
    ) -> usize {
        let mut end = start + first.len_utf8();
        let mut base = 10u8;

        if first == '0' {
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
                        if *next_c == '.' {
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
            if suffix == 'n' {
                chars.next();
                end = i + 1;
            }
        }

        end
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_interpolation(
        line: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        tokens: &mut Vec<Token>,
        expr_start: usize,
    ) -> Option<usize> {
        let mut depth = 1usize;
        let mut in_string: Option<char> = None;
        let mut escaped = false;
        let mut in_regex = false;
        let mut in_class = false;

        while let Some((idx, ch)) = chars.next() {
            if let Some(quote) = in_string {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == quote {
                    in_string = None;
                }
                continue;
            }

            if in_regex {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '[' {
                    in_class = true;
                    continue;
                }
                if ch == ']' && in_class {
                    in_class = false;
                    continue;
                }
                if ch == '/' && !in_class {
                    in_regex = false;
                }
                continue;
            }

            match ch {
                '\'' | '"' | '`' => {
                    in_string = Some(ch);
                }
                '/' => {
                    if let Some(&(_, '/')) = chars.peek() {
                        if expr_start < idx {
                            tokens.push(Token::new(TokenKind::Text, expr_start, idx));
                        }
                        return None;
                    }
                    if let Some(&(_, '*')) = chars.peek() {
                        chars.next();
                        let (_end_idx, found_end) = Self::scan_block_comment(chars, line.len());
                        if !found_end {
                            if expr_start < line.len() {
                                tokens.push(Token::new(TokenKind::Text, expr_start, line.len()));
                            }
                            return None;
                        }
                    } else {
                        in_regex = true;
                    }
                }
                '{' => {
                    depth = depth.saturating_add(1);
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        if expr_start < idx {
                            tokens.push(Token::new(TokenKind::Text, expr_start, idx));
                        }
                        tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                        return Some(idx + 1);
                    }
                }
                _ => {}
            }
        }

        if expr_start < line.len() {
            tokens.push(Token::new(TokenKind::Text, expr_start, line.len()));
        }

        None
    }

    // Peekable scanning is easier to read with while-let loops.
    #[allow(clippy::while_let_on_iterator)]
    fn scan_template_literal(
        line: &str,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        tokens: &mut Vec<Token>,
        mut segment_start: usize,
    ) -> LineState {
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
            if ch == '`' {
                tokens.push(Token::new(TokenKind::String, segment_start, idx + 1));
                return LineState::Normal;
            }
            if ch == '$' {
                if let Some(&(_, '{')) = chars.peek() {
                    if segment_start < idx {
                        tokens.push(Token::new(TokenKind::String, segment_start, idx));
                    }
                    chars.next();
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 2));
                    if let Some(end_idx) = Self::scan_interpolation(line, chars, tokens, idx + 2) {
                        segment_start = end_idx;
                        continue;
                    }
                    return LineState::InString(StringKind::Backtick);
                }
            }
        }

        tokens.push(Token::new(TokenKind::String, segment_start, line.len()));
        LineState::InString(StringKind::Backtick)
    }
}

impl Default for JavaScriptTokenizer {
    fn default() -> Self {
        Self::javascript()
    }
}

impl Tokenizer for JavaScriptTokenizer {
    fn name(&self) -> &'static str {
        if self.typescript_mode {
            "TypeScript"
        } else {
            "JavaScript"
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        if self.typescript_mode {
            &["ts", "tsx", "mts", "cts"]
        } else {
            &["js", "jsx", "mjs", "cjs"]
        }
    }

    #[allow(clippy::too_many_lines)]
    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        let mut tokens = Vec::new();
        let mut chars = line.char_indices().peekable();
        let mut can_start_regex = true;
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
                let (end_idx, found_end) = Self::scan_string(&mut chars, line.len(), '"');
                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, end_idx));
                } else {
                    tokens.push(Token::new(TokenKind::String, 0, line.len()));
                    return (tokens, LineState::InString(StringKind::Double));
                }
            }
            LineState::InString(StringKind::Single) => {
                let (end_idx, found_end) = Self::scan_string(&mut chars, line.len(), '\'');
                if found_end {
                    tokens.push(Token::new(TokenKind::String, 0, end_idx));
                } else {
                    tokens.push(Token::new(TokenKind::String, 0, line.len()));
                    return (tokens, LineState::InString(StringKind::Single));
                }
            }
            LineState::InString(StringKind::Backtick) => {
                let next_state = Self::scan_template_literal(line, &mut chars, &mut tokens, 0);
                if next_state != LineState::Normal {
                    return (tokens, next_state);
                }
            }
            _ => {}
        }

        if !tokens.is_empty() {
            can_start_regex = false;
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
                        let is_doc = line[idx..].starts_with("/**");
                        chars.next();
                        let (end_idx, found_end) = Self::scan_block_comment(&mut chars, line.len());
                        let token_kind = if is_doc {
                            TokenKind::CommentDoc
                        } else {
                            TokenKind::CommentBlock
                        };

                        if found_end {
                            tokens.push(Token::new(token_kind, idx, end_idx));
                            can_start_regex = true;
                        } else {
                            tokens.push(Token::new(token_kind, idx, line.len()));
                            let next_kind = if is_doc {
                                CommentKind::Doc
                            } else {
                                CommentKind::Block
                            };
                            return (tokens, LineState::InComment(next_kind));
                        }
                        continue;
                    }
                    if can_start_regex {
                        let (end_idx, found_end) = Self::scan_regex(line, &mut chars);
                        if found_end {
                            tokens.push(Token::new(TokenKind::String, idx, end_idx));
                            can_start_regex = false;
                        } else {
                            tokens.push(Token::new(TokenKind::String, idx, line.len()));
                        }
                        continue;
                    }
                    if let Some(&(next_idx, '=')) = chars.peek() {
                        chars.next();
                        tokens.push(Token::new(TokenKind::Operator, idx, next_idx + 1));
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                    }
                    can_start_regex = true;
                }

                '\'' | '"' => {
                    let start = idx;
                    let (end_idx, found_end) = Self::scan_string(&mut chars, line.len(), ch);
                    if found_end {
                        tokens.push(Token::new(TokenKind::String, start, end_idx));
                        can_start_regex = false;
                    } else {
                        tokens.push(Token::new(TokenKind::String, start, line.len()));
                        let next_state = if ch == '"' {
                            LineState::InString(StringKind::Double)
                        } else {
                            LineState::InString(StringKind::Single)
                        };
                        return (tokens, next_state);
                    }
                }

                '`' => {
                    let next_state =
                        Self::scan_template_literal(line, &mut chars, &mut tokens, idx);
                    can_start_regex = false;
                    if next_state != LineState::Normal {
                        return (tokens, next_state);
                    }
                }

                '@' => {
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
                    can_start_regex = false;
                }

                '#' => {
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
                    tokens.push(Token::new(TokenKind::Identifier, start, end));
                    can_start_regex = false;
                }

                c if c.is_ascii_digit() => {
                    let start = idx;
                    let end = Self::scan_number(&mut chars, start, c);
                    tokens.push(Token::new(TokenKind::Number, start, end));
                    can_start_regex = false;
                }

                '.' => {
                    if line[idx..].starts_with("...") {
                        chars.next();
                        chars.next();
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 3));
                        can_start_regex = true;
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
                            can_start_regex = false;
                        } else {
                            tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                            can_start_regex = true;
                        }
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                        can_start_regex = true;
                    }
                }

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
                    if let Some(kind) = Self::is_keyword(word, self.typescript_mode) {
                        tokens.push(Token::new(kind, start, end));
                        can_start_regex = true;
                    } else if word.chars().next().is_some_and(char::is_uppercase) {
                        tokens.push(Token::new(TokenKind::Type, start, end));
                        can_start_regex = false;
                    } else if let Some(&(_, '(')) = chars.peek() {
                        tokens.push(Token::new(TokenKind::Function, start, end));
                        can_start_regex = false;
                    } else {
                        tokens.push(Token::new(TokenKind::Identifier, start, end));
                        can_start_regex = false;
                    }
                }

                '(' | '[' | '{' | ',' | ';' | ':' => {
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                    can_start_regex = true;
                }
                ')' | ']' | '}' => {
                    tokens.push(Token::new(TokenKind::Punctuation, idx, idx + 1));
                    can_start_regex = false;
                }
                '?' => {
                    if line[idx..].starts_with("??") || line[idx..].starts_with("?.") {
                        chars.next();
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 2));
                    } else {
                        tokens.push(Token::new(TokenKind::Operator, idx, idx + 1));
                    }
                    can_start_regex = true;
                }
                '=' => {
                    let mut end = idx + 1;
                    if line[idx..].starts_with("===") || line[idx..].starts_with("!==") {
                        chars.next();
                        chars.next();
                        end = idx + 3;
                    } else if line[idx..].starts_with("=>") {
                        chars.next();
                        end = idx + 2;
                    } else if let Some(&(_, next)) = chars.peek() {
                        if next == '=' {
                            chars.next();
                            end = idx + 2;
                        }
                    }
                    tokens.push(Token::new(TokenKind::Operator, idx, end));
                    can_start_regex = true;
                }
                '+' | '-' | '*' | '%' | '!' | '<' | '>' | '&' | '|' | '^' | '~' => {
                    let mut end = idx + 1;
                    if let Some(&(_, next)) = chars.peek() {
                        if next == '=' || next == ch {
                            chars.next();
                            end = idx + 2;
                        }
                    }
                    tokens.push(Token::new(TokenKind::Operator, idx, end));
                    can_start_regex = true;
                }

                _ => {
                    tokens.push(Token::new(TokenKind::Text, idx, idx + 1));
                    can_start_regex = false;
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
    fn test_js_keywords() {
        let tokenizer = JavaScriptTokenizer::javascript();
        let (tokens, _) =
            tokenizer.tokenize_line("if else return function const class", LineState::Normal);

        assert_eq!(tokens.len(), 6);
        assert_eq!(tokens[0].kind, TokenKind::KeywordControl);
        assert_eq!(tokens[1].kind, TokenKind::KeywordControl);
        assert_eq!(tokens[2].kind, TokenKind::KeywordControl);
        assert_eq!(tokens[3].kind, TokenKind::Keyword);
        assert_eq!(tokens[4].kind, TokenKind::Keyword);
        assert_eq!(tokens[5].kind, TokenKind::Keyword);
    }

    #[test]
    fn test_js_strings() {
        let tokenizer = JavaScriptTokenizer::javascript();
        let (tokens, _) = tokenizer.tokenize_line("'a' \"b\"", LineState::Normal);

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, TokenKind::String);
        assert_eq!(tokens[1].kind, TokenKind::String);
    }

    #[test]
    fn test_js_template_literals() {
        let tokenizer = JavaScriptTokenizer::javascript();
        let line = "`hi ${name}`";
        let (tokens, state) = tokenizer.tokenize_line(line, LineState::Normal);

        assert_eq!(state, LineState::Normal);

        let strings: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::String)
            .collect();
        assert_eq!(strings.len(), 2);

        let punct: Vec<_> = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Punctuation)
            .collect();
        assert_eq!(punct.len(), 2);
        assert_eq!(&line[punct[0].range()], "${");
        assert_eq!(&line[punct[1].range()], "}");
    }

    #[test]
    fn test_js_regex() {
        let tokenizer = JavaScriptTokenizer::javascript();
        let line = "const r = /ab+c/i;";
        let (tokens, _) = tokenizer.tokenize_line(line, LineState::Normal);

        let regex = tokens
            .iter()
            .find(|token| token.kind == TokenKind::String)
            .expect("regex token");
        assert_eq!(&line[regex.range()], "/ab+c/i");
    }

    #[test]
    fn test_ts_keywords() {
        let tokenizer = JavaScriptTokenizer::typescript();
        let (tokens, _) = tokenizer.tokenize_line("interface readonly public", LineState::Normal);

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::KeywordType);
        assert_eq!(tokens[1].kind, TokenKind::KeywordModifier);
        assert_eq!(tokens[2].kind, TokenKind::KeywordModifier);
    }

    #[test]
    fn test_js_comments() {
        let tokenizer = JavaScriptTokenizer::javascript();
        let (tokens, _) = tokenizer.tokenize_line("x = 1 // comment", LineState::Normal);

        assert_eq!(tokens.last().unwrap().kind, TokenKind::Comment);
    }
}
