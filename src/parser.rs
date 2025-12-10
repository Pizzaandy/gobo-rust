use crate::lex::{Token, TokenIndex, TokenKind, TokenizedText};
use crate::source_text::TextSize;
pub type ParseDiagnostic = &'static str;

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Start { kind: NodeKind },
    End,
    Token { start: TextSize, kind: TokenKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeKind {
    Ignore,
    Error,
    File,
    Block,
    Function,
    PrefixOpExpr,
    ParenExpr,
    ArrayExpr,
}

pub struct Parser<'a> {
    input: &'a TokenizedText,
    output: ParseEvents,
    cursor: TokenIndex,
    last_statement_start: usize, // used for error recovery
    depth: u32,
}

pub struct ParseEvents {
    pub events: Vec<Event>,
    pub diagnostics: Vec<ParseDiagnostic>,
}

pub fn parse(tokens: &TokenizedText) -> ParseEvents {
    let mut parser = Parser::new(tokens);
    parser.parse();
    parser.output
}

impl<'a> Parser<'a> {
    fn new(input: &'a TokenizedText) -> Self {
        let estimated_event_count = input.tokens.len() * 2;
        Self {
            input,
            output: ParseEvents {
                events: Vec::with_capacity(estimated_event_count),
                diagnostics: Vec::new(),
            },
            cursor: 0.into(),
            last_statement_start: 0,
            depth: 0
        }
    }

    pub fn parse(&mut self) {
        debug_assert_eq!(self.current(), TokenKind::FileStart);
        self.cursor += 1;
        self.statement_list();
        debug_assert_eq!(self.current(), TokenKind::FileEnd);
    }

    fn emit_start(&mut self, kind: NodeKind) {
        self.output.events.push(Event::Start { kind });
        self.depth += 1;
    }

    fn emit_end(&mut self) {
        self.output.events.push(Event::End);
        self.depth.checked_sub(1).expect("unbalanced events");
    }

    fn advance(&mut self) {
        let token = self.input.tokens.get(self.cursor);
        self.output.events.push(Event::Token {
            kind: token.kind(),
            start: token.start(),
        });
        self.cursor += 1;
    }

    fn current(&self) -> TokenKind {
        self.input.tokens.get(self.cursor).kind()
    }

    fn hit_eof(&self) -> bool {
        self.current() == TokenKind::FileEnd
    }

    fn accept_comments(&mut self) {
        while self.current().is_comment() {
            self.advance();
        }
    }

    fn accept(&mut self, kind: TokenKind) -> bool {
        self.accept_comments();
        if self.current() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    // Flags the current statement as an error
    fn expect(&mut self, kind: TokenKind) {
        if !self.accept(kind) {
            self.error();
        }
    }

    fn error(&mut self) {
        let event = &mut self.output.events[self.last_statement_start];
        match *event {
            Event::Start { ref mut kind } => {
                *kind = NodeKind::Error;
            }
            _ => panic!("expected a start event at self.last_statement_start"),
        }
    }


    fn statement_list(&mut self) {
        while !self.hit_eof() {
            if !self.statement() {
                break;
            }
        }
    }

    fn statement(&mut self) -> bool {
        let start = self.output.events.len();

        match self.current() {
            TokenKind::BraceOpen => self.block(),
            TokenKind::Function => self.function(),
            TokenKind::Semicolon => self.advance(),
            TokenKind::Var | TokenKind::Static | TokenKind::GlobalVar => {
                self.variable_declaration()
            }
            TokenKind::Identifier => self.assign_or_expression(),
            _ => return false,
        };

        debug_assert!(self.output.events.len() > start);
        self.last_statement_start = start;
        true
    }

    fn assign_or_expression(&mut self) {}

    fn variable_declaration(&mut self) {}

    fn unary_expr(&mut self) -> bool {
        if self.current().is_prefix_operator() {
            self.emit_start(NodeKind::PrefixOpExpr);
            self.advance();
            self.primary_expr(false);
            self.emit_end();
            return true;
        }

        self.primary_expr(true)
    }

    // only allow postfix operators if we didn't already accept a prefix operator
    fn primary_expr(&mut self, in_prefix_op: bool) -> bool {
        todo!()
    }

    fn primary_expr_start(&mut self) -> bool {
        match self.current() {
            TokenKind::Identifier
            | TokenKind::IntegerLiteral
            | TokenKind::RealLiteral
            | TokenKind::StringLiteral
            | TokenKind::VerbatimStringLiteral
            | TokenKind::HexIntegerLiteral
            | TokenKind::BinaryLiteral => self.advance(),
            TokenKind::ParenOpen => {
                self.emit_start(NodeKind::ParenExpr);
                self.advance();
                self.expr();
                self.expect(TokenKind::ParenClose);
                self.emit_end();
            }
            TokenKind::BracketOpen => self.delimited_list(
                NodeKind::ArrayExpr,
                TokenKind::BracketOpen,
                TokenKind::BracketClose,
                TokenKind::Comma,
            ),
            _ => return false,
        };

        true
    }

    fn delimited_list(
        &mut self,
        node_kind: NodeKind,
        open: TokenKind,
        close: TokenKind,
        separator: TokenKind,
    ) {
        self.emit_start(node_kind);
        self.expect(open);

        let mut expect_separator = false;
        let mut ended_on_closing_delimiter = false;

        while !self.hit_eof() {
            if expect_separator {
                self.expect(separator);
            } else {
                self.expr();
            }
            expect_separator = !expect_separator;
            if self.accept(TokenKind::BracketClose) {
                ended_on_closing_delimiter = true;
                break;
            }
        }

        if !ended_on_closing_delimiter {
            todo!("err")
        }

        self.emit_end();
    }

    fn expr(&mut self) -> bool {
        todo!();
    }

    fn block(&mut self) {
        self.emit_start(NodeKind::Block);
        self.expect(TokenKind::BraceOpen);
        self.statement_list();
        self.expect(TokenKind::BraceClose);
        self.emit_end();
    }

    fn function(&mut self) {
        self.emit_start(NodeKind::Function);
        self.expect(TokenKind::Function);
    }
}
