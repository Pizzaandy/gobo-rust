use crate::lex::identifier_lexer::*;
use crate::lex::number_lexer::scan_number_or_dot;
use crate::lex::string_lexer::{scan_string_literal, scan_verbatim_string_literal};
use crate::lex::token::{Token, TokenIndex, TokenKind};
use crate::lex::{Comment, Line, LineIndex, TokenizedText};
use crate::parse::ParseDiagnostic;
use crate::source_text::{SourceText, TextSize};

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Dispatch {
    Error,
    NewLine,
    Cr,
    HorizontalSpace,
    Exclamation,
    Quote,
    IdentifierStart,
    Dollar,
    Hash,
    Percent,
    Ampersand,
    ParenOpen,
    ParenClose,
    Asterisk,
    Plus,
    Comma,
    Minus,
    Dot,
    Slash,
    DigitZero,
    DigitNonZero,
    Colon,
    Semicolon,
    LessThan,
    Equal,
    GreaterThan,
    Question,
    At,
    BracketOpen,
    BracketClose,
    Caret,
    BraceOpen,
    Pipe,
    BraceClose,
    Tilde,
    Unicode,
}

const CHAR_COUNT: usize = 256;

static DISPATCH_TABLE: [Dispatch; CHAR_COUNT] = {
    let mut result = [Dispatch::Error; CHAR_COUNT];
    let mut i = 0;
    while i < CHAR_COUNT {
        let c = i as u8;
        result[i] = match c {
            b'\n' => Dispatch::NewLine,
            b'\r' => Dispatch::Cr,
            b'!' => Dispatch::Exclamation,
            b'"' => Dispatch::Quote,
            b'$' => Dispatch::Dollar,
            b'#' => Dispatch::Hash,
            b'%' => Dispatch::Percent,
            b'&' => Dispatch::Ampersand,
            b'(' => Dispatch::ParenOpen,
            b')' => Dispatch::ParenClose,
            b'*' => Dispatch::Asterisk,
            b'+' => Dispatch::Plus,
            b',' => Dispatch::Comma,
            b'-' => Dispatch::Minus,
            b'.' => Dispatch::Dot,
            b'/' => Dispatch::Slash,
            b'0' => Dispatch::DigitZero,
            b'1'..=b'9' => Dispatch::DigitNonZero,
            b':' => Dispatch::Colon,
            b';' => Dispatch::Semicolon,
            b'<' => Dispatch::LessThan,
            b'=' => Dispatch::Equal,
            b'>' => Dispatch::GreaterThan,
            b'?' => Dispatch::Question,
            b'@' => Dispatch::At,
            b'[' => Dispatch::BracketOpen,
            b']' => Dispatch::BracketClose,
            b'^' => Dispatch::Caret,
            b'{' => Dispatch::BraceOpen,
            b'|' => Dispatch::Pipe,
            b'}' => Dispatch::BraceClose,
            b'~' => Dispatch::Tilde,
            c if is_identifier_start(c) => Dispatch::IdentifierStart,
            c if is_horizontal_whitespace(c) => Dispatch::HorizontalSpace,
            0x00..=0x7F => Dispatch::Error,
            _ => Dispatch::Unicode,
        };
        i += 1;
    }
    result
};

const fn is_horizontal_whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\t')
}

const fn is_open_delimiter(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::LeftParen
            | TokenKind::LeftBrace
            | TokenKind::LeftSquare
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
        TokenKind::RightParen | TokenKind::RightBrace | TokenKind::RightSquare
    )
}

fn is_matching_delimiter(open_kind: TokenKind, close_kind: TokenKind) -> bool {
    debug_assert!(is_open_delimiter(open_kind));
    debug_assert!(is_close_delimiter(close_kind));
    match open_kind {
        TokenKind::LeftParen => close_kind == TokenKind::RightParen,
        TokenKind::LeftBrace => close_kind == TokenKind::RightBrace,
        TokenKind::LeftSquare
        | TokenKind::ArrayAccessor
        | TokenKind::ListAccessor
        | TokenKind::GridAccessor
        | TokenKind::MapAccessor
        | TokenKind::StructAccessor => close_kind == TokenKind::RightSquare,
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
    open_delimiters: Vec<TokenIndex>,
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
            open_delimiters: Vec::new(),
            has_leading_space: false,
            has_mismatched_brackets: false,
        }
    }

    fn lex(&mut self) {
        self.make_lines();
        self.lex_file_start();

        while self.cursor < self.text.len() {
            // dispatch table covers all possible u8 values
            let dispatch_kind = unsafe {
                *DISPATCH_TABLE.get_unchecked(self.text.get_byte_unchecked(self.cursor) as usize)
            };
            match dispatch_kind {
                Dispatch::IdentifierStart => self.lex_keyword_or_identifier(),
                Dispatch::Dot => self.lex_number_literal_or_dot(),
                Dispatch::DigitZero => self.lex_number_literal_or_dot(),
                Dispatch::DigitNonZero => self.lex_number_literal_or_dot(),
                Dispatch::Quote => self.lex_string_literal(),
                Dispatch::At => self.lex_verbatim_string_literal(),
                Dispatch::Dollar => self.lex_template_string_or_hex_literal(),
                Dispatch::HorizontalSpace => self.lex_horizontal_whitespace(),
                Dispatch::NewLine => self.lex_vertical_whitespace(),
                Dispatch::Cr => self.lex_cr(),
                Dispatch::Slash => self.lex_comment_or_divide(),
                Dispatch::Hash => todo!("hex literal + directives"),

                Dispatch::BracketOpen => self.lex_accessor(),
                Dispatch::BracketClose => self.lex_close_delimiter(TokenKind::RightSquare),
                Dispatch::ParenOpen => self.lex_open_delimiter(TokenKind::LeftParen),
                Dispatch::ParenClose => self.lex_close_delimiter(TokenKind::RightParen),
                Dispatch::BraceOpen => self.lex_open_delimiter(TokenKind::LeftBrace),
                Dispatch::BraceClose => self.lex_close_delimiter(TokenKind::RightBrace),

                Dispatch::Comma => self.lex_byte(TokenKind::Comma),
                Dispatch::Colon => self.lex_byte(TokenKind::Colon),
                Dispatch::Semicolon => self.lex_byte(TokenKind::Semicolon),

                Dispatch::Exclamation => {
                    self.lex_byte_and_equals(self.cursor, TokenKind::Not, TokenKind::NotEquals)
                }
                Dispatch::Percent => self.lex_byte_and_equals(
                    self.cursor,
                    TokenKind::Modulo,
                    TokenKind::ModuloAssign,
                ),
                Dispatch::Caret => self.lex_byte_and_equals(
                    self.cursor,
                    TokenKind::BitXor,
                    TokenKind::BitXorAssign,
                ),
                Dispatch::Tilde => self.lex_byte_and_equals(
                    self.cursor,
                    TokenKind::BitNot,
                    TokenKind::BitNotAssign,
                ),
                Dispatch::Equal => {
                    self.lex_byte_and_equals(self.cursor, TokenKind::Equals, TokenKind::Equals)
                }

                Dispatch::Ampersand => self.lex_byte_twice_or_equals(
                    TokenKind::BitAnd,
                    TokenKind::And,
                    TokenKind::BitAndAssign,
                ),
                Dispatch::Asterisk => self.lex_byte_twice_or_equals(
                    TokenKind::Multiply,
                    TokenKind::Power,
                    TokenKind::MultiplyAssign,
                ),
                Dispatch::Plus => self.lex_byte_twice_or_equals(
                    TokenKind::Plus,
                    TokenKind::PlusPlus,
                    TokenKind::PlusAssign,
                ),
                Dispatch::Minus => self.lex_byte_twice_or_equals(
                    TokenKind::Minus,
                    TokenKind::MinusMinus,
                    TokenKind::MinusAssign,
                ),
                Dispatch::Pipe => self.lex_byte_twice_or_equals(
                    TokenKind::BitOr,
                    TokenKind::Or,
                    TokenKind::BitOrAssign,
                ),
                Dispatch::LessThan => self.lex_less_than(),
                Dispatch::GreaterThan => self.lex_greater_than(),
                Dispatch::Question => self.lex_question(),

                Dispatch::Unicode => todo!("unicode"),
                Dispatch::Error => self.lex_error(),
            };
        }

        self.lex_file_end();

        if self.output.token_count() >= Token::MAX_INDEX {
            todo!("report too many tokens");
        }
    }

    fn add_token(&mut self, kind: TokenKind, start: TextSize) -> TokenIndex {
        Self::add_token_with_payload(self, kind, 0, start)
    }

    fn add_token_with_payload(
        &mut self,
        kind: TokenKind,
        payload: u32,
        start: TextSize,
    ) -> TokenIndex {
        let token = Token::new(kind, self.has_leading_space, payload, start);
        self.has_leading_space = false;
        self.output.add_token(token)
    }

    fn current(&self) -> u8 {
        self.text.get_byte(self.cursor)
    }

    fn peek(&self) -> u8 {
        if self.cursor + 1 < self.text.len() {
            self.text.get_byte(self.cursor + 1)
        } else {
            0
        }
    }

    fn lex_byte(&mut self, kind: TokenKind) {
        self.add_token(kind, self.cursor);
        self.cursor += 1;
    }

    fn lex_open_delimiter(&mut self, kind: TokenKind) {
        debug_assert!(is_open_delimiter(kind));
        let token_index = self.add_token(kind, self.cursor);
        self.cursor += 1;
        self.handle_open_delimiter(token_index);
    }

    fn lex_close_delimiter(&mut self, kind: TokenKind) {
        debug_assert!(is_close_delimiter(kind));
        let token_index = self.add_token(kind, self.cursor);
        self.cursor += 1;
        self.handle_close_delimiter(token_index);
    }

    fn lex_byte_and_equals(&mut self, start: TextSize, kind: TokenKind, equals_kind: TokenKind) {
        if self.peek() == b'=' {
            self.cursor += 2;
            self.add_token(equals_kind, start);
        } else {
            self.cursor += 1;
            self.add_token(kind, start);
        }
    }

    fn lex_byte_twice_or_equals(
        &mut self,
        kind: TokenKind,
        twice_kind: TokenKind,
        equals_kind: TokenKind,
    ) {
        let start = self.cursor;
        let start_byte = self.current();

        if self.peek() == b'=' {
            self.cursor += 2;
            self.add_token(equals_kind, start);
        } else if self.peek() == start_byte {
            self.cursor += 2;
            self.add_token(twice_kind, start);
        } else {
            self.cursor += 1;
            self.add_token(kind, start);
        }
    }

    fn handle_open_delimiter(&mut self, open_token_index: TokenIndex) {
        self.open_delimiters.push(open_token_index);
    }

    fn handle_close_delimiter(&mut self, close_token_index: TokenIndex) {
        if let Some(open_token_index) = self.open_delimiters.pop() {
            // store the matching delimiter in the payload
            let close_kind: TokenKind;
            {
                let close_token = self.output.tokens.get_mut(close_token_index);
                close_token.set_payload(open_token_index.value());
                close_kind = close_token.kind();
            }

            let open_token = self.output.tokens.get_mut(open_token_index);
            if is_matching_delimiter(open_token.kind(), close_kind) {
                open_token.set_payload(close_token_index.value());
            } else {
                self.has_mismatched_brackets = true;
            }
        } else {
            self.has_mismatched_brackets = true;
        }
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

        let is_lfcr = self.cursor.value() > 0 && self.text.get_byte(self.cursor - 1) == b'\n';

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

    fn lex_keyword_or_identifier(&mut self) {
        let start = self.cursor;
        if self.text.get_byte(start) > 0x7F {
            self.lex_error();
            return;
        }

        self.cursor += scan_identifier(self.text.get_slice(self.cursor..));
        let slice = self.text.get_slice(start..self.cursor);

        let kind = Self::match_keyword(slice);
        self.add_token(kind, start);
    }

    fn match_keyword(text: &[u8]) -> TokenKind {
        match text {
            b"and" => TokenKind::And,
            b"or" => TokenKind::Or,
            b"xor" => TokenKind::Xor,
            b"not" => TokenKind::Not,
            b"mod" => TokenKind::Modulo,
            b"div" => TokenKind::IntegerDivide,
            b"begin" => TokenKind::LeftBrace,
            b"end" => TokenKind::RightBrace,
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
            _ => TokenKind::Identifier,
        }
    }

    fn lex_accessor(&mut self) {
        debug_assert!(self.current() == b'[');

        let start = self.cursor;
        self.cursor += 1;
        let kind = match self.current() {
            b'|' => TokenKind::ListAccessor,
            b'?' => TokenKind::MapAccessor,
            b'#' => TokenKind::GridAccessor,
            b'@' => TokenKind::ArrayAccessor,
            b'$' => TokenKind::StructAccessor,
            _ => TokenKind::LeftSquare,
        };

        if kind != TokenKind::LeftSquare {
            self.cursor += 1;
        }

        let token_index = self.add_token(kind, start);
        self.handle_open_delimiter(token_index);
    }

    fn lex_less_than(&mut self) {
        debug_assert!(self.current() == b'<');

        let start = self.cursor;
        self.cursor += 1;
        match self.current() {
            b'<' => {
                self.cursor += 1;
                self.lex_byte_and_equals(start, TokenKind::LeftShift, TokenKind::LeftShiftAssign);
            }
            b'=' => {
                self.cursor += 1;
                self.add_token(TokenKind::LessThanEquals, start);
            }
            _ => {
                self.add_token(TokenKind::LessThan, start);
            }
        }
    }

    fn lex_greater_than(&mut self) {
        debug_assert!(self.current() == b'>');

        let start = self.cursor;
        self.cursor += 1;
        match self.current() {
            b'>' => {
                self.cursor += 1;
                self.lex_byte_and_equals(start, TokenKind::RightShift, TokenKind::RightShiftAssign);
            }
            b'=' => {
                self.cursor += 1;
                self.add_token(TokenKind::GreaterThanEquals, start);
            }
            _ => {
                self.add_token(TokenKind::GreaterThan, start);
            }
        }
    }

    fn lex_question(&mut self) {
        let start = self.cursor;
        self.cursor += 1;
        match self.current() {
            b'?' => {
                self.cursor += 1;
                self.lex_byte_and_equals(
                    start,
                    TokenKind::NullCoalesce,
                    TokenKind::NullCoalesceAssign,
                );
            }
            _ => {
                self.add_token(TokenKind::GreaterThan, start);
            }
        }
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
        todo!("lex template strings and hex literals");
    }

    fn lex_comment_or_divide(&mut self) {
        debug_assert!(self.current() == b'/');
        let start = self.cursor;

        match self.peek() {
            b'/' => {
                self.advance_to_next_line();
                self.output.add_comment(Comment::new(start, self.cursor));
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
                        self.output.add_comment(Comment::new(start, self.cursor));
                        break;
                    }
                }
            }
            _ => self.lex_byte_and_equals(start, TokenKind::Divide, TokenKind::DivideAssign),
        }
    }

    fn lex_error(&mut self) {
        // keep lexing until we hit a recovery character
        let start = self.cursor;

        while self.cursor < self.text.len() {
            let c = self.current();
            if is_identifier_byte(c) || is_horizontal_whitespace(c) {
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
