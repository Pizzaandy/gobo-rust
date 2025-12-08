use crate::lex::{Token, TokenIndex, TokenKind, TokenizedText};
use crate::source_text::TextSize;
pub type ParseDiagnostic = &'static str;

#[derive(Debug, Clone)]
pub enum Event {
    Start { kind: NodeKind },
    End,
    Token { start: TextSize, kind: TokenKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeKind {
    Ignore,
    File,
    Block,
}

pub struct Parser<'a> {
    tokens: &'a TokenizedText,
    events: Vec<Event>,
    diagnostics: Vec<ParseDiagnostic>,
    cursor: TokenIndex,
}

pub fn parse(tokens: &TokenizedText) -> Vec<Event> {
    let mut parser = Parser::new(tokens);
    parser.parse();
    parser.events
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a TokenizedText) -> Self {
        Self {
            tokens,
            events: Vec::new(),
            diagnostics: Vec::new(),
            cursor: 0.into(),
        }
    }

    fn emit_start(&mut self, kind: NodeKind) {
        self.events.push(Event::Start { kind });
    }

    fn emit_end(&mut self) {
        self.events.push(Event::End);
    }

    fn emit_token(&mut self, kind: TokenKind, start: TextSize) {
        self.events.push(Event::Token { kind, start });
    }

    fn emit_current(&mut self) {
        let token = self.current();
        self.emit_token(token.kind(), token.start());
    }

    fn current(&self) -> &Token {
        self.tokens.tokens.get(self.cursor)
    }

    fn hit_eof(&self) -> bool {
        self.current().kind() == TokenKind::FileEnd
    }

    fn accept(&mut self, kind: TokenKind) -> bool {
        if self.current().kind() == kind {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind) {
        assert_eq!(self.current().kind(), kind);
        self.cursor += 1;
    }

    pub fn parse(&mut self) {
        self.emit_start(NodeKind::File);
        self.statement_list();
        self.emit_end();
    }

    fn statement_list(&mut self) {
        while !self.hit_eof() {
            self.statement();
        }
    }

    fn statement(&mut self) {
        match self.current().kind() {
            TokenKind::OpenBrace => self.block(),
            TokenKind::Semicolon => self.emit_current(),
            _ => {}
        }
    }

    fn block(&mut self) {
        self.emit_start(NodeKind::Block);
        self.expect(TokenKind::OpenBrace);
        self.statement_list();
        self.expect(TokenKind::CloseBrace);
    }
}
