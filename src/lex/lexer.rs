use crate::lex::identifier_lexer::*;
use crate::lex::number_lexer::{is_digit, scan_number_or_dot};
use crate::lex::token::{Token, TokenIndex, TokenKind};
use crate::lex::{Comment, Line, LineIndex, TokenizedText};
use crate::source_text::{SourceText, TextSize};
use crate::user_symbols::UserSymbols;

static COMMON_START_SYMBOLS: [(&[u8], TokenKind); 40] = [
    (b">>=", TokenKind::RightShiftAssign),
    (b">>", TokenKind::RightShift),
    (b">=", TokenKind::GreaterThanEquals),
    (b">", TokenKind::GreaterThan),
    (b"<<=", TokenKind::LeftShiftAssign),
    (b"<<", TokenKind::LeftShift),
    (b"<=", TokenKind::LessThanEquals),
    (b"<", TokenKind::LessThan),
    (b"&=", TokenKind::BitAndAssign),
    (b"&&", TokenKind::And),
    (b"&", TokenKind::BitAnd),
    (b"|=", TokenKind::BitOrAssign),
    (b"|", TokenKind::BitOr),
    (b"^=", TokenKind::BitXorAssign),
    (b"^", TokenKind::BitXor),
    (b"~=", TokenKind::BitNotAssign),
    (b"~", TokenKind::BitNot),
    (b"+=", TokenKind::PlusAssign),
    (b"+", TokenKind::Plus),
    (b"*=", TokenKind::MultiplyAssign),
    (b"*", TokenKind::Multiply),
    (b"/=", TokenKind::DivideAssign),
    (b"/", TokenKind::Divide),
    (b"%=", TokenKind::ModulusAssign),
    (b"%", TokenKind::Modulo),
    (b"==", TokenKind::Equals),
    (b"=", TokenKind::Assign),
    (b"!=", TokenKind::NotEquals),
    (b"!", TokenKind::Not),
    (b"-=", TokenKind::MinusAssign),
    (b"-", TokenKind::Minus),
    (b"??=", TokenKind::NullCoalesceAssign),
    (b"??", TokenKind::NullCoalesce),
    (b"?", TokenKind::QuestionMark),
    (b"[|", TokenKind::ListAccessor),
    (b"[?", TokenKind::MapAccessor),
    (b"[#", TokenKind::GridAccessor),
    (b"[@", TokenKind::ArrayAccessor),
    (b"[$", TokenKind::StructAccessor),
    (b"[", TokenKind::OpenBrace),
];

static UNIQUE_START_SYMBOLS: [(u8, TokenKind); 9] = [
    (b';', TokenKind::Semicolon),
    (b':', TokenKind::Colon),
    (b',', TokenKind::Comma),
    (b'.', TokenKind::Dot),
    (b'{', TokenKind::OpenBrace),
    (b'}', TokenKind::CloseBrace),
    (b'(', TokenKind::OpenParen),
    (b')', TokenKind::CloseParen),
    (b']', TokenKind::CloseBracket),
];

static KEYWORDS: [(&[u8], TokenKind); 37] = [
    (b"and", TokenKind::And),
    (b"or", TokenKind::Or),
    (b"xor", TokenKind::Xor),
    (b"not", TokenKind::Not),
    (b"mod", TokenKind::Modulo),
    (b"div", TokenKind::IntegerDivide),
    (b"begin", TokenKind::OpenBrace),
    (b"end", TokenKind::CloseBrace),
    (b"true", TokenKind::BooleanLiteral),
    (b"false", TokenKind::BooleanLiteral),
    (b"break", TokenKind::Break),
    (b"exit", TokenKind::Exit),
    (b"do", TokenKind::Do),
    (b"until", TokenKind::Until),
    (b"case", TokenKind::Case),
    (b"else", TokenKind::Else),
    (b"new", TokenKind::New),
    (b"var", TokenKind::Var),
    (b"globalvar", TokenKind::GlobalVar),
    (b"try", TokenKind::Try),
    (b"catch", TokenKind::Catch),
    (b"finally", TokenKind::Finally),
    (b"return", TokenKind::Return),
    (b"continue", TokenKind::Continue),
    (b"for", TokenKind::For),
    (b"switch", TokenKind::Switch),
    (b"while", TokenKind::While),
    (b"repeat", TokenKind::Repeat),
    (b"function", TokenKind::Function),
    (b"with", TokenKind::With),
    (b"default", TokenKind::Default),
    (b"if", TokenKind::If),
    (b"then", TokenKind::Then),
    (b"throw", TokenKind::Throw),
    (b"delete", TokenKind::Delete),
    (b"enum", TokenKind::Enum),
    (b"constructor", TokenKind::Constructor),
];

#[derive(Copy, Clone)]
#[repr(u8)]
enum DispatchKind {
    IdentifierStart,
    CommonSymbolStart,
    UniqueSymbolStart,
    NumberOrDot,
    HorizontalWhitespace,
    CommentOrDivide,
    Newline,
    Cr,
    Error,
}

const CHAR_COUNT: usize = u8::MAX as usize + 1;

static DISPATCH_TABLE: [DispatchKind; CHAR_COUNT] = {
    let mut result: [DispatchKind; CHAR_COUNT] = [DispatchKind::Error; CHAR_COUNT];
    let mut i = 0;
    while i < CHAR_COUNT {
        let c = i as u8;
        result[i] = match c {
            b'/' => DispatchKind::CommentOrDivide,
            b'\n' => DispatchKind::Newline,
            b'\r' => DispatchKind::Cr,
            b'.' => DispatchKind::NumberOrDot,
            c if is_identifier_start(c) => DispatchKind::IdentifierStart,
            c if is_common_symbol_start(c) => DispatchKind::CommonSymbolStart,
            c if is_unique_symbol_start(c) => DispatchKind::UniqueSymbolStart,
            c if is_horizontal_whitespace(c) => DispatchKind::HorizontalWhitespace,
            c if is_digit(c) => DispatchKind::NumberOrDot,
            _ => DispatchKind::Error,
        };
        i += 1;
    }
    result
};

const fn is_common_symbol_start(c: u8) -> bool {
    let mut i = 0;
    while i < COMMON_START_SYMBOLS.len() {
        let (bytes, _) = COMMON_START_SYMBOLS[i];
        if bytes[0] == c {
            return true;
        }
        i += 1;
    }
    false
}

const fn is_unique_symbol_start(c: u8) -> bool {
    let mut i = 0;
    while i < UNIQUE_START_SYMBOLS.len() {
        let (byte, _) = UNIQUE_START_SYMBOLS[i];
        if byte == c {
            return true;
        }
        i += 1;
    }
    false
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

const fn is_matching_delimiter(open_kind: TokenKind, close_kind: TokenKind) -> bool {
    debug_assert!(is_open_delimiter(open_kind));
    debug_assert!(is_close_delimiter(close_kind));
    match open_kind {
        TokenKind::OpenParen => matches!(close_kind, TokenKind::CloseParen),
        TokenKind::OpenBrace => matches!(close_kind, TokenKind::CloseBrace),
        TokenKind::OpenBracket
        | TokenKind::ArrayAccessor
        | TokenKind::ListAccessor
        | TokenKind::GridAccessor
        | TokenKind::MapAccessor
        | TokenKind::StructAccessor => matches!(close_kind, TokenKind::CloseBracket),
        _ => panic!("expected an open delimiter"),
    }
}

pub fn lex(text: &SourceText, symbols: &mut UserSymbols) -> TokenizedText {
    let mut tokens = TokenizedText::new();
    let mut lexer = Lexer::new(&mut tokens, text, symbols);
    lexer.lex();
    tokens
}

struct Lexer<'a> {
    output: &'a mut TokenizedText,
    text: &'a SourceText,
    symbols: &'a mut UserSymbols,
    cursor: TextSize,
    line_index: LineIndex,
    open_brackets: Vec<TokenIndex>,
    has_leading_space: bool,
    has_mismatched_brackets: bool,
}

impl<'a> Lexer<'a> {
    fn new(output: &'a mut TokenizedText, text: &'a SourceText, symbols: &'a mut UserSymbols) -> Self {
        Self {
            output,
            text,
            symbols,
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
                DispatchKind::IdentifierStart => self.lex_keyword_or_identifier(),
                DispatchKind::CommonSymbolStart => self.lex_common_start_symbol(),
                DispatchKind::UniqueSymbolStart => self.lex_unique_start_symbol(),
                DispatchKind::NumberOrDot => self.lex_number_literal_or_dot(),
                DispatchKind::HorizontalWhitespace => self.lex_horizontal_whitespace(),
                DispatchKind::Newline => self.lex_vertical_whitespace(),
                DispatchKind::Cr => self.lex_cr(),
                DispatchKind::CommentOrDivide => self.lex_comment_or_divide(),
                DispatchKind::Error => self.lex_error(),
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
    fn peek(&self, c: u8) -> bool {
        self.cursor + 1 < self.text.len() && self.text.get_byte(self.cursor + 1) == c
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
        if self.peek(b'\n') {
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
        let Lexer { text, .. } = self;
        let start = self.cursor;
        let slice = text.get_slice(start..);
        for (bytes, kind) in COMMON_START_SYMBOLS {
            if slice.starts_with(bytes) {
                let token_index = self.add_token(kind, start);
                self.cursor += bytes.len();
                if is_open_delimiter(kind) {
                    self.open_brackets.push(token_index);
                }
                return;
            }
        }

        self.lex_error();
    }

    fn lex_unique_start_symbol(&mut self) {
        let start = self.cursor;
        let slice = self.text.get_slice(start..);
        let mut kind = TokenKind::Error;

        for (byte, _kind) in UNIQUE_START_SYMBOLS {
            if slice[0] == byte {
                self.cursor += 1;
                kind = _kind;
                break;
            }
        }

        if kind == TokenKind::Error {
            self.lex_error();
            return;
        }

        if !is_close_delimiter(kind) {
            self.add_token(kind, start);
            return;
        }

        if let Some(open_token_index) = self.open_brackets.pop() {
            // store the matching delimiter
            let close_token_index = self.add_token_with_payload(kind, open_token_index.value(), start);
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

        for (bytes, kind) in KEYWORDS {
            if slice == bytes {
                self.add_token(kind, start);
                return;
            }
        }

        let span = self.text.get_span(start, self.cursor);
        let id = self.symbols.identifiers.push(span);
        self.add_token_with_payload(TokenKind::Identifier, id.value(), start);
    }

    fn lex_number_literal_or_dot(&mut self) {
        let start = self.cursor;
        let (len, kind) = scan_number_or_dot(self.text.get_slice(start..));
        self.cursor += len;

        if kind == TokenKind::Error {
            self.lex_error();
            return;
        }
        
        if kind == TokenKind::Dot {
            self.add_token(TokenKind::Dot, start);
            return;
        }

        let span = self.text.get_span(start, self.cursor);
        let id = self.symbols.string_literals.push(span);
        self.add_token_with_payload(kind, id.value(), start);
    }

    fn lex_comment_or_divide(&mut self) {
        debug_assert!(self.current() == b'/');
        let start = self.cursor;

        match self.current() {
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