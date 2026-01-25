use crate::highlight::token::{Token, TokenKind};
use crate::highlight::tokenizer::{LineState, StringKind, Tokenizer};

pub struct MarkdownTokenizer;

impl Default for MarkdownTokenizer {
    fn default() -> Self {
        Self
    }
}

impl MarkdownTokenizer {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn is_hr(trimmed: &str) -> bool {
        let bytes = trimmed.as_bytes();
        if bytes.len() < 3 {
            return false;
        }
        let first = bytes[0];
        if first != b'-' && first != b'*' && first != b'_' {
            return false;
        }
        bytes.iter().all(|&b| b == first)
    }

    fn is_setext_heading(trimmed: &str) -> bool {
        let bytes = trimmed.as_bytes();
        if bytes.len() < 2 {
            return false;
        }
        let first = bytes[0];
        if first != b'=' && first != b'-' {
            return false;
        }
        bytes.iter().all(|&b| b == first)
    }

    fn is_escaped(bytes: &[u8], idx: usize) -> bool {
        idx > 0 && bytes[idx - 1] == b'\\'
    }

    fn in_ranges(idx: usize, ranges: &[(usize, usize)]) -> bool {
        ranges.iter().any(|&(start, end)| idx >= start && idx < end)
    }

    fn scan_code_spans(line: &str, tokens: &mut Vec<Token>) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'`' && !Self::is_escaped(bytes, i) {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'`' && !Self::is_escaped(bytes, i) {
                        let end = i + 1;
                        tokens.push(Token::new(TokenKind::CodeInline, start, end));
                        ranges.push((start, end));
                        i = end;
                        break;
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        ranges
    }

    fn scan_links(line: &str, ranges: &[(usize, usize)], tokens: &mut Vec<Token>) {
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'[' && !Self::is_escaped(bytes, i) && !Self::in_ranges(i, ranges) {
                let start = if i > 0 && bytes[i - 1] == b'!' && !Self::is_escaped(bytes, i - 1) {
                    i - 1
                } else {
                    i
                };
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] != b']' {
                    j += 1;
                }
                if j < bytes.len() {
                    let next = j + 1;
                    if next < bytes.len() && (bytes[next] == b'(' || bytes[next] == b'[') {
                        let closer = if bytes[next] == b'(' { b')' } else { b']' };
                        let mut k = next + 1;
                        while k < bytes.len() && bytes[k] != closer {
                            k += 1;
                        }
                        if k < bytes.len() {
                            let end = k + 1;
                            tokens.push(Token::new(TokenKind::Link, start, end));
                            i = end;
                            continue;
                        }
                    }
                }
            }
            i += 1;
        }
    }

    fn scan_emphasis(line: &str, ranges: &[(usize, usize)], tokens: &mut Vec<Token>) {
        let bytes = line.as_bytes();
        let delims = ["***", "___", "**", "__", "~~", "*", "_"];
        for delim in delims {
            let delim_bytes = delim.as_bytes();
            let mut i = 0usize;
            while i + delim_bytes.len() <= bytes.len() {
                if bytes[i..].starts_with(delim_bytes)
                    && !Self::is_escaped(bytes, i)
                    && !Self::in_ranges(i, ranges)
                {
                    let start = i;
                    i += delim_bytes.len();
                    while i + delim_bytes.len() <= bytes.len() {
                        if bytes[i..].starts_with(delim_bytes)
                            && !Self::is_escaped(bytes, i)
                            && !Self::in_ranges(i, ranges)
                        {
                            let end = i + delim_bytes.len();
                            tokens.push(Token::new(TokenKind::Emphasis, start, end));
                            i = end;
                            break;
                        }
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
}

impl Tokenizer for MarkdownTokenizer {
    fn name(&self) -> &'static str {
        "Markdown"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md", "markdown", "mkd", "mkdn"]
    }

    fn tokenize_line(&self, line: &str, state: LineState) -> (Vec<Token>, LineState) {
        let mut tokens = Vec::new();
        let trimmed = line.trim_start();
        let trim_offset = line.len() - trimmed.len();

        if matches!(state, LineState::InString(StringKind::Backtick)) {
            tokens.push(Token::new(TokenKind::CodeBlock, 0, line.len()));
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                return (tokens, LineState::Normal);
            }
            return (tokens, LineState::InString(StringKind::Backtick));
        }

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            tokens.push(Token::new(TokenKind::CodeBlock, trim_offset, line.len()));
            return (tokens, LineState::InString(StringKind::Backtick));
        }

        if line.starts_with("    ") || line.starts_with('\t') {
            tokens.push(Token::new(TokenKind::CodeBlock, 0, line.len()));
            return (tokens, LineState::Normal);
        }

        if trimmed.starts_with('>') {
            tokens.push(Token::new(TokenKind::Comment, trim_offset, line.len()));
            return (tokens, LineState::Normal);
        }

        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
            if (1..=6).contains(&hash_count) && trimmed[hash_count..].starts_with(' ') {
                tokens.push(Token::new(TokenKind::Heading, trim_offset, line.len()));
                return (tokens, LineState::Normal);
            }
        }

        if Self::is_hr(trimmed) {
            tokens.push(Token::new(TokenKind::Punctuation, trim_offset, line.len()));
            return (tokens, LineState::Normal);
        }

        if Self::is_setext_heading(trimmed) {
            tokens.push(Token::new(TokenKind::Heading, trim_offset, line.len()));
            return (tokens, LineState::Normal);
        }

        if trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ")
            || trimmed.chars().take_while(char::is_ascii_digit).count() > 0
                && trimmed.contains(". ")
        {
            tokens.push(Token::new(
                TokenKind::Punctuation,
                trim_offset,
                trim_offset + 1,
            ));
        }

        let code_ranges = Self::scan_code_spans(line, &mut tokens);
        Self::scan_links(line, &code_ranges, &mut tokens);
        Self::scan_emphasis(line, &code_ranges, &mut tokens);

        (tokens, LineState::Normal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md_headings() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("# Heading", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::Heading);

        let (tokens, _) = tokenizer.tokenize_line("Heading\n", LineState::Normal);
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_md_emphasis() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("**bold** and *italic*", LineState::Normal);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Emphasis));
    }

    #[test]
    fn test_md_code_inline() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("Use `code` here", LineState::Normal);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::CodeInline));
    }

    #[test]
    fn test_md_code_blocks() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, state) = tokenizer.tokenize_line("```rust", LineState::Normal);
        assert_eq!(tokens[0].kind, TokenKind::CodeBlock);
        assert_eq!(state, LineState::InString(StringKind::Backtick));

        let (tokens, state) = tokenizer.tokenize_line("fn main() {}", state);
        assert_eq!(tokens[0].kind, TokenKind::CodeBlock);
        assert_eq!(state, LineState::InString(StringKind::Backtick));

        let (tokens, state) = tokenizer.tokenize_line("```", state);
        assert_eq!(tokens[0].kind, TokenKind::CodeBlock);
        assert_eq!(state, LineState::Normal);
    }

    #[test]
    fn test_md_links() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("[link](url)", LineState::Normal);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Link));
    }

    #[test]
    fn test_md_lists() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("- item", LineState::Normal);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Punctuation));
    }

    #[test]
    fn test_md_blockquotes() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("> quote", LineState::Normal);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Comment));
    }

    #[test]
    fn test_md_escaping() {
        let tokenizer = MarkdownTokenizer::new();
        let (tokens, _) = tokenizer.tokenize_line("\\*not italic\\*", LineState::Normal);
        assert!(!tokens.iter().any(|t| t.kind == TokenKind::Emphasis));
    }
}
