use std::num::{NonZeroU8, NonZeroUsize};
use crate::lex::identifier_lexer::*;
use crate::lex::number_lexer::{is_digit, scan_number_or_dot};
use crate::lex::string_lexer::{scan_string_literal, scan_verbatim_string_literal};
use crate::lex::token::{Token, TokenIndex, TokenKind};
use crate::lex::{Comment, Line, LineIndex, TokenizedText};
use crate::source_text::{SourceText, TextSize};
use crate::user_symbols::UserSymbols;
use phf_macros::phf_map;
use crate::fnv::fnv1a_32;

static KEYWORDS: phf::Map<&'static [u8], TokenKind> = phf_map! {
    b"and" => TokenKind::And,
    b"or" => TokenKind::Or,
    b"xor" => TokenKind::Xor,
    b"not" => TokenKind::Not,
    b"mod" => TokenKind::Modulo,
    b"div" => TokenKind::IntegerDivide,
    b"begin" => TokenKind::OpenBrace,
    b"end" => TokenKind::CloseBrace,
    b"true" => TokenKind::BooleanLiteral,
    b"false" => TokenKind::BooleanLiteral,
    b"break" => TokenKind::Break,
    b"exit" => TokenKind::Exit,
    b"do" => TokenKind::Do,
    b"until" => TokenKind::Until,
    b"case" => TokenKind::Case,
    b"else" => TokenKind::Else,
    b"new" => TokenKind::New,
    b"var" => TokenKind::Var,
    b"globalvar" => TokenKind::GlobalVar,
    b"try" => TokenKind::Try,
    b"catch" => TokenKind::Catch,
    b"finally" => TokenKind::Finally,
    b"return" => TokenKind::Return,
    b"continue" => TokenKind::Continue,
    b"for" => TokenKind::For,
    b"switch" => TokenKind::Switch,
    b"while" => TokenKind::While,
    b"repeat" => TokenKind::Repeat,
    b"function" => TokenKind::Function,
    b"with" => TokenKind::With,
    b"default" => TokenKind::Default,
    b"if" => TokenKind::If,
    b"then" => TokenKind::Then,
    b"throw" => TokenKind::Throw,
    b"delete" => TokenKind::Delete,
    b"enum" => TokenKind::Enum,
    b"constructor" => TokenKind::Constructor,
};

#[derive(Copy, Clone)]
#[repr(u8)]
enum Dispatch {
    IdentifierStart,
    CommonSymbolStart,
    UniqueSymbolStart,
    NumberOrDot,
    Quote,
    At,
    Dollar,
    HorizontalWhitespace,
    CommentOrDivide,
    Newline,
    Cr,
    Error,
}

const CHAR_COUNT: usize = u8::MAX as usize + 1;

static DISPATCH_TABLE: [Dispatch; CHAR_COUNT] = {
    let mut result: [Dispatch; CHAR_COUNT] = [Dispatch::Error; CHAR_COUNT];
    let mut i = 0;
    while i < CHAR_COUNT {
        let c = i as u8;
        result[i] = match c {
            b'/' => Dispatch::CommentOrDivide,
            b'\n' => Dispatch::Newline,
            b'\r' => Dispatch::Cr,
            b'.' => Dispatch::NumberOrDot,
            b'"' => Dispatch::Quote,
            b'@' => Dispatch::At,
            b'$' => Dispatch::Dollar,
            c if is_digit(c) => Dispatch::NumberOrDot,
            c if is_identifier_start(c) => Dispatch::IdentifierStart,
            c if is_common_symbol_start(c) => Dispatch::CommonSymbolStart,
            c if is_unique_symbol_start(c) => Dispatch::UniqueSymbolStart,
            c if is_horizontal_whitespace(c) => Dispatch::HorizontalWhitespace,
            _ => Dispatch::Error,
        };
        i += 1;
    }
    result
};

const fn is_common_symbol_start(c: u8) -> bool {
    matches!(
        c,
        b'>' | b'<'
            | b'&'
            | b'|'
            | b'^'
            | b'~'
            | b'+'
            | b'*'
            | b'/'
            | b'%'
            | b'='
            | b'!'
            | b'-'
            | b'?'
            | b'['
    )
}

const fn is_unique_symbol_start(c: u8) -> bool {
    matches!(
        c,
        b';' | b':' | b',' | b'.' | b'{' | b'}' | b'(' | b')' | b']'
    )
}

const fn is_horizontal_whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\t')
}

const fn is_open_delimiter(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::OpenParen
            | TokenKind::OpenBrace
            | TokenKind::OpenBracket
            | TokenKind::ArrayAccessor
            | TokenKind::ListAccessor
            | TokenKind::GridAccessor
            | TokenKind::MapAccessor
            | TokenKind::StructAccessor
    )
}

const fn is_close_delimiter(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::CloseParen | TokenKind::CloseBrace | TokenKind::CloseBracket
    )
}

fn is_matching_delimiter(open_kind: TokenKind, close_kind: TokenKind) -> bool {
    debug_assert!(is_open_delimiter(open_kind));
    debug_assert!(is_close_delimiter(close_kind));
    match open_kind {
        TokenKind::OpenParen => close_kind == TokenKind::CloseParen,
        TokenKind::OpenBrace => close_kind == TokenKind::CloseBrace,
        TokenKind::OpenBracket
        | TokenKind::ArrayAccessor
        | TokenKind::ListAccessor
        | TokenKind::GridAccessor
        | TokenKind::MapAccessor
        | TokenKind::StructAccessor => close_kind == TokenKind::CloseBracket,
        _ => panic!("expected an open delimiter"),
    }
}

pub fn lex(text: &SourceText) -> TokenizedText {
    let mut lexer = Lexer::new(text);
    lexer.lex();
    lexer.output
}

struct Lexer<'a> {
    output: TokenizedText,
    text: &'a SourceText,
    cursor: TextSize,
    line_index: LineIndex,
    open_brackets: Vec<TokenIndex>,
    has_leading_space: bool,
    has_mismatched_brackets: bool,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a SourceText) -> Self {
        Self {
            output: TokenizedText::new(),
            text,
            cursor: TextSize::from(0),
            line_index: LineIndex::from(0),
            open_brackets: Vec::new(),
            has_leading_space: false,
            has_mismatched_brackets: false,
        }
    }

    fn lex(&mut self) {
        self.make_lines();
        self.lex_file_start();

        while self.cursor < self.text.len() {
            // dispatch table covers all possible u8 values
            let byte_kind = unsafe {
                *DISPATCH_TABLE.get_unchecked(self.text.get_byte_unchecked(self.cursor) as usize)
            };
            match byte_kind {
                Dispatch::IdentifierStart => self.lex_keyword_or_identifier(),
                Dispatch::CommonSymbolStart => self.lex_common_start_symbol(),
                Dispatch::UniqueSymbolStart => self.lex_unique_start_symbol(),
                Dispatch::NumberOrDot => self.lex_number_literal_or_dot(),
                Dispatch::Quote => self.lex_string_literal(),
                Dispatch::At => self.lex_verbatim_string_literal(),
                Dispatch::Dollar => self.lex_template_string_or_hex_literal(),
                Dispatch::HorizontalWhitespace => self.lex_horizontal_whitespace(),
                Dispatch::Newline => self.lex_vertical_whitespace(),
                Dispatch::Cr => self.lex_cr(),
                Dispatch::CommentOrDivide => self.lex_comment_or_divide(),
                Dispatch::Error => self.lex_error(),
            };
        }

        self.lex_file_end();

        if self.output.tokens.len() >= Token::MAX_INDEX {
            todo!("report too many tokens");
        }
    }

    #[inline(always)]
    fn add_token(&mut self, kind: TokenKind, start: TextSize) -> TokenIndex {
        Self::add_token_with_payload(self, kind, 0, start)
    }

    #[inline(always)]
    fn add_token_with_payload(
        &mut self,
        kind: TokenKind,
        payload: u32,
        start: TextSize,
    ) -> TokenIndex {
        let token = Token::new(kind, self.has_leading_space, payload, start);
        self.has_leading_space = false;
        self.output.tokens.push(token)
    }

    #[inline(always)]
    fn peek(&self) -> u8 {
        if self.cursor + 1 < self.text.len() {
            self.text.get_byte(self.cursor + 1)
        } else {
            0
        }
    }

    #[inline(always)]
    fn get_byte(&self, pos: TextSize) -> u8 {
        self.text.get_byte(pos)
    }

    #[inline(always)]
    fn current(&self) -> u8 {
        self.text.get_byte(self.cursor)
    }

    fn lex_file_start(&mut self) {
        debug_assert!(self.cursor == 0);
        self.add_token(TokenKind::FileStart, TextSize::from(0));
        self.has_leading_space = true;

        let current_line = self.output.lines.get(self.line_index);
        debug_assert!(current_line.start() == 0);

        self.advance_to_line(LineIndex::from(0));
    }

    fn lex_file_end(&mut self) {
        debug_assert!(self.cursor == self.text.len());
        self.has_leading_space = true;
        self.add_token(TokenKind::FileEnd, self.cursor);
    }

    fn make_lines(&mut self) {
        let Lexer {
            text,
            output: tokens,
            ..
        } = self;

        if text.len() == 0 {
            tokens.lines.push(Line::new(TextSize::from(0)));
            return;
        }

        let mut start: TextSize = 0.into();

        loop {
            match text.find_next(b'\n', start) {
                Some(new_line_start) => {
                    tokens.lines.push(Line::new(start));
                    start = new_line_start + 1;
                }
                None => {
                    break;
                }
            };
        }

        // The last line ends at the end of the file
        tokens.lines.push(Line::new(start));

        // If the last line wasn't empty, insert a fake blank line
        if start != text.len() {
            tokens.lines.push(Line::new(text.len()));
            tokens.last_line_is_inserted = true;
        }
    }

    fn advance_to_line(&mut self, to_line: LineIndex) {
        debug_assert!(to_line > self.line_index || (to_line == 0 && self.line_index == 0));
        self.line_index = to_line;
        self.cursor = self.output.lines.get(to_line).start();
        self.skip_horizontal_whitespace();
        let line_info = self.output.lines.get_mut(self.line_index);
        line_info.set_indent((self.cursor - line_info.start()).value());
    }

    fn advance_to_next_line(&mut self) {
        self.advance_to_line(self.line_index + 1);
    }

    fn skip_horizontal_whitespace(&mut self) {
        while self.cursor < self.text.len() {
            let c = self.current();
            if is_horizontal_whitespace(c) {
                self.cursor += 1;
            } else {
                break;
            }
        }
    }

    fn lex_horizontal_whitespace(&mut self) {
        self.has_leading_space = true;
        self.skip_horizontal_whitespace();
    }

    fn lex_vertical_whitespace(&mut self) {
        self.has_leading_space = true;
        self.advance_to_next_line();
    }

    fn lex_cr(&mut self) {
        // normalize CR+LF
        if self.peek() == b'\n' {
            self.lex_vertical_whitespace();
            return;
        }

        let is_lfcr = self.cursor.value() > 0 && self.get_byte(self.cursor - 1) == b'\n';

        if is_lfcr {
            self.output
                .diagnostics
                .push("the LF+CR line ending is not supported, only LF and CR+LF are supported");
        } else {
            self.output
                .diagnostics
                .push("a raw CR line ending is not supported, only LF and CR+LF are supported");
        }

        // treat unexpected CR as horizontal whitespace
        self.has_leading_space = true;
        self.cursor += 1;
    }

    fn lex_common_start_symbol(&mut self) {
        debug_assert!(is_common_symbol_start(self.current()));

        let start = self.cursor;
        let slice = self.text.get_slice(start..);

        let (kind, len) = match slice[0] {
            b'>' => {
                if slice.starts_with(b">>=") {
                    (TokenKind::RightShiftAssign, 3)
                } else if slice.starts_with(b">>") {
                    (TokenKind::RightShift, 2)
                } else if slice.starts_with(b">=") {
                    (TokenKind::GreaterThanEquals, 2)
                } else {
                    (TokenKind::GreaterThan, 1)
                }
            }
            b'<' => {
                if slice.starts_with(b"<<=") {
                    (TokenKind::LeftShiftAssign, 3)
                } else if slice.starts_with(b"<<") {
                    (TokenKind::LeftShift, 2)
                } else if slice.starts_with(b"<=") {
                    (TokenKind::LessThanEquals, 2)
                } else {
                    (TokenKind::LessThan, 1)
                }
            }
            b'&' => {
                if slice.starts_with(b"&=") {
                    (TokenKind::BitAndAssign, 2)
                } else if slice.starts_with(b"&&") {
                    (TokenKind::And, 2)
                } else {
                    (TokenKind::BitAnd, 1)
                }
            }
            b'|' => {
                if slice.starts_with(b"|=") {
                    (TokenKind::BitOrAssign, 2)
                } else {
                    (TokenKind::BitOr, 1)
                }
            }
            b'^' => {
                if slice.starts_with(b"^=") {
                    (TokenKind::BitXorAssign, 2)
                } else {
                    (TokenKind::BitXor, 1)
                }
            }
            b'~' => {
                if slice.starts_with(b"~=") {
                    (TokenKind::BitNotAssign, 2)
                } else {
                    (TokenKind::BitNot, 1)
                }
            }
            b'+' => {
                if slice.starts_with(b"+=") {
                    (TokenKind::PlusAssign, 2)
                } else {
                    (TokenKind::Plus, 1)
                }
            }
            b'*' => {
                if slice.starts_with(b"*=") {
                    (TokenKind::MultiplyAssign, 2)
                } else {
                    (TokenKind::Multiply, 1)
                }
            }
            b'/' => {
                if slice.starts_with(b"/=") {
                    (TokenKind::DivideAssign, 2)
                } else {
                    (TokenKind::Divide, 1)
                }
            }
            b'%' => {
                if slice.starts_with(b"%=") {
                    (TokenKind::ModulusAssign, 2)
                } else {
                    (TokenKind::Modulo, 1)
                }
            }
            b'=' => {
                if slice.starts_with(b"==") {
                    (TokenKind::Equals, 2)
                } else {
                    (TokenKind::Assign, 1)
                }
            }
            b'!' => {
                if slice.starts_with(b"!=") {
                    (TokenKind::NotEquals, 2)
                } else {
                    (TokenKind::Not, 1)
                }
            }
            b'-' => {
                if slice.starts_with(b"-=") {
                    (TokenKind::MinusAssign, 2)
                } else {
                    (TokenKind::Minus, 1)
                }
            }
            b'?' => {
                if slice.starts_with(b"??=") {
                    (TokenKind::NullCoalesceAssign, 3)
                } else if slice.starts_with(b"??") {
                    (TokenKind::NullCoalesce, 2)
                } else {
                    (TokenKind::QuestionMark, 1)
                }
            }
            b'[' => {
                if slice.starts_with(b"[|") {
                    (TokenKind::ListAccessor, 2)
                } else if slice.starts_with(b"[?") {
                    (TokenKind::MapAccessor, 2)
                } else if slice.starts_with(b"[#") {
                    (TokenKind::GridAccessor, 2)
                } else if slice.starts_with(b"[@") {
                    (TokenKind::ArrayAccessor, 2)
                } else if slice.starts_with(b"[$") {
                    (TokenKind::StructAccessor, 2)
                } else {
                    (TokenKind::OpenBracket, 1)
                }
            }
            _ => {
                self.lex_error();
                return;
            }
        };

        self.cursor += len;

        let token_index = self.add_token(kind, start);

        if is_open_delimiter(kind) {
            self.open_brackets.push(token_index);
        }
    }

    fn lex_unique_start_symbol(&mut self) {
        debug_assert!(is_unique_symbol_start(self.current()));
        let start = self.cursor;
        let kind = match self.current() {
            b';' => TokenKind::Semicolon,
            b':' => TokenKind::Colon,
            b',' => TokenKind::Comma,
            b'.' => TokenKind::Dot,
            b'{' => TokenKind::OpenBrace,
            b'}' => TokenKind::CloseBrace,
            b'(' => TokenKind::OpenParen,
            b')' => TokenKind::CloseParen,
            b']' => TokenKind::CloseBracket,
            _ => {
                self.lex_error();
                return;
            }
        };

        self.cursor += 1;

        if !is_close_delimiter(kind) {
            self.add_token(kind, start);
            return;
        }

        if let Some(open_token_index) = self.open_brackets.pop() {
            // store the matching delimiter
            let close_token_index =
                self.add_token_with_payload(kind, open_token_index.value(), start);
            let open_token = self.output.tokens.get_mut(open_token_index);
            if is_matching_delimiter(open_token.kind(), kind) {
                open_token.set_payload(close_token_index.value());
            } else {
                self.has_mismatched_brackets = true;
            }
        } else {
            self.has_mismatched_brackets = true;
            self.add_token(kind, start);
        }
    }

    fn lex_keyword_or_identifier(&mut self) {
        let start = self.cursor;
        if self.get_byte(start) > 0x7F {
            self.lex_error();
            return;
        }

        self.cursor += scan_identifier(self.text.get_slice(self.cursor..));
        let slice = self.text.get_slice(start..self.cursor);

        if let Some(kind) = KEYWORDS.get(slice) {
            self.add_token(*kind, start);
            return;
        }

        self.add_token_with_payload(TokenKind::Identifier, 0, start);
    }

    fn lex_number_literal_or_dot(&mut self) {
        let start = self.cursor;
        let (len, kind) = scan_number_or_dot(self.text.get_slice(start..));

        if kind == TokenKind::Error {
            self.lex_error();
            return;
        }

        self.cursor += len;

        if kind == TokenKind::Dot {
            self.add_token(TokenKind::Dot, start);
            return;
        }
        
        self.add_token_with_payload(kind, 0, start);
    }

    fn lex_string_literal(&mut self) {
        let start = self.cursor;
        let (len, kind) = scan_string_literal(self.text.get_slice(start..));

        if kind == TokenKind::Error {
            self.lex_error();
            return;
        }

        self.cursor += len;

        self.add_token_with_payload(kind, 0, start);
    }

    fn lex_verbatim_string_literal(&mut self) {
        let start = self.cursor;
        let (len, kind) = scan_verbatim_string_literal(self.text.get_slice(start..));

        if kind == TokenKind::Error {
            self.lex_error();
            return;
        }

        self.cursor += len;

        self.add_token_with_payload(kind, 0, start);
    }

    fn lex_template_string_or_hex_literal(&mut self) {
        debug_assert!(self.current() == b'$');
        todo!();
    }

    fn lex_comment_or_divide(&mut self) {
        debug_assert!(self.current() == b'/');
        let start = self.cursor;

        match self.peek() {
            b'/' => {
                self.advance_to_next_line();
                let id = self.output.comments.push(Comment::new(start, self.cursor));
                self.add_token_with_payload(TokenKind::SingleLineComment, id.value(), start);
            }
            b'*' => {
                self.cursor += 1;
                while self.cursor < self.text.len() {
                    while self.cursor + 1 < self.text.len() {
                        self.cursor += 1;
                        if self.current() == b'*' {
                            self.cursor += 1;
                            break;
                        }
                    }
                    if self.current() == b'/' {
                        self.cursor += 1;
                        let id = self.output.comments.push(Comment::new(start, self.cursor));
                        self.add_token_with_payload(TokenKind::MultiLineComment, id.value(), start);
                        break;
                    }
                }
            }
            _ => self.lex_common_start_symbol(),
        }
    }

    fn lex_error(&mut self) {
        // keep lexing until we hit a recovery character
        let start = self.cursor;

        while self.cursor < self.text.len() {
            let c = self.current();
            if is_identifier_char(c) || is_horizontal_whitespace(c) {
                break;
            }
            self.cursor += 1;
        }

        let mut len = self.cursor - start;

        // take at least one char
        if len == 0 {
            self.cursor += 1;
            len += 1;
        }

        self.output
            .diagnostics
            .push("unrecognized characters while parsing");

        self.add_token_with_payload(TokenKind::Error, len.value(), start);
    }
}
