use crate::lex::{TokenIndex, TokenKind, TokenizedText};
use crate::source_text::TextSize;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
pub type ParseDiagnostic = &'static str;

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Start {
        kind: NodeKind,
    },
    End,
    Leaf {
        token: TokenIndex,
        token_kind: TokenKind,
    },
    Unexpected {
        token: TokenIndex,
        token_kind: TokenKind,
    },
    Missing {
        kind: NodeKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeKind {
    Ignore,
    Error,
    File,
    Block,
    EnumDecl,
    EnumBlock,
    EnumMember,
    Function,
    PrefixOpExpr,
    ParenExpr,
    ArrayExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum StateKind {
    Statement,
    StatementLoop,
    BlockStart,
    BlockEnd,
    EnumStart,
    EnumItem,
    EnumLoop,
    EnumEnd,
}

#[derive(Debug, Clone, Copy)]
struct State {
    kind: StateKind,
    has_error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ConsumeResult {
    Success,
    Recovered,
    FailedRecovery,
}

impl ConsumeResult {
    pub fn failed(self) -> bool {
        matches!(self, Self::FailedRecovery)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenPrecedence {
    Unknown,
    IdentifierOrLiteral,
    ExpressionOperator,
    WeakBracketOpen,
    WeakPunctuator,
    MediumPunctuator,
    WeakBracketClose,
    LeftBrace,
    StrongPunctuator,
    IntroducerKeyword,
    RightBrace,
}

impl TokenPrecedence {
    fn precedence(&self) -> u32 {
        match self {
            TokenPrecedence::Unknown => 0,
            TokenPrecedence::IdentifierOrLiteral => 1,
            TokenPrecedence::ExpressionOperator => 2,
            TokenPrecedence::WeakBracketOpen => 3,
            TokenPrecedence::WeakPunctuator => 4,
            TokenPrecedence::MediumPunctuator => 5,
            TokenPrecedence::WeakBracketClose => 6,
            TokenPrecedence::LeftBrace => 7,
            TokenPrecedence::StrongPunctuator => 8,
            TokenPrecedence::IntroducerKeyword => 9,
            TokenPrecedence::RightBrace => 10,
        }
    }
}

impl From<TokenKind> for TokenPrecedence {
    fn from(kind: TokenKind) -> Self {
        match kind {
            TokenKind::Identifier => TokenPrecedence::IdentifierOrLiteral,
            TokenKind::RealLiteral | TokenKind::IntegerLiteral => {
                TokenPrecedence::IdentifierOrLiteral
            }
            kind if kind.is_prefix_operator()
                || kind.is_postfix_operator()
                || kind.is_binary_operator() =>
            {
                TokenPrecedence::ExpressionOperator
            }
            TokenKind::LeftParen | TokenKind::LeftSquare => TokenPrecedence::WeakBracketOpen,
            TokenKind::Dot => TokenPrecedence::WeakPunctuator,
            TokenKind::Comma => TokenPrecedence::MediumPunctuator,
            TokenKind::RightParen | TokenKind::RightSquare => TokenPrecedence::WeakBracketClose,
            TokenKind::LeftBrace => TokenPrecedence::LeftBrace,
            TokenKind::Semicolon | TokenKind::FileEnd => TokenPrecedence::StrongPunctuator,
            TokenKind::RightBrace => TokenPrecedence::RightBrace,
            _ => TokenPrecedence::Unknown,
        }
    }
}

impl PartialOrd for TokenPrecedence {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.precedence().cmp(&other.precedence()))
    }
}

struct ListNodeKind {
    item_state: StateKind,
    loop_state: StateKind,
    end_state: StateKind,
    item_kind: NodeKind,
    separator: TokenKind,
    close_token: TokenKind,
}

const ENUM_MEMBER_LIST: ListNodeKind = ListNodeKind {
    item_state: StateKind::EnumItem,
    loop_state: StateKind::EnumLoop,
    end_state: StateKind::EnumEnd,
    item_kind: NodeKind::EnumMember,
    separator: TokenKind::Comma,
    close_token: TokenKind::RightBrace,
};

pub struct Parser<'a> {
    input: &'a TokenizedText,
    output: ParseEvents,
    cursor: TokenIndex,
    last_statement_start: usize, // used for error recovery
    depth: u32,
    stack: Vec<State>,
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
        let estimated_event_count = input.token_count() * 2;
        Self {
            input,
            output: ParseEvents {
                events: Vec::with_capacity(estimated_event_count),
                diagnostics: Vec::new(),
            },
            cursor: 0.into(),
            last_statement_start: 0,
            depth: 0,
            stack: Vec::new(),
        }
    }

    fn parse(&mut self) {
        debug_assert_eq!(self.current(), TokenKind::FileStart);
        self.cursor += 1;
        self.push_state(StateKind::StatementLoop);

        while !self.stack.is_empty() {
            let kind = self.stack[self.stack.len() - 1].kind;
            match kind {
                StateKind::StatementLoop => self.statement_loop(),
                StateKind::Statement => self.statement(),
                StateKind::BlockStart => self.block_start(),
                StateKind::BlockEnd => self.block_end(),
                StateKind::EnumStart => self.enum_start(),
                StateKind::EnumLoop => {
                    self.list_loop(ENUM_MEMBER_LIST);
                }
                StateKind::EnumItem => self.enum_item(),
                StateKind::EnumEnd => self.enum_end(),
                _ => todo!(),
            }
        }

        debug_assert_eq!(self.current(), TokenKind::FileEnd);
    }

    fn push_state(&mut self, kind: StateKind) {
        self.stack.push(State {
            kind,
            has_error: false,
        });
    }

    fn push_sequence<const N: usize>(&mut self, states: [StateKind; N]) {
        for i in (0..N).rev() {
            self.push_state(states[i]);
        }
    }

    fn pop_state(&mut self) -> State {
        self.stack.pop().expect("stack underflow")
    }

    fn current_state(&self) -> &State {
        self.stack.last().expect("stack underflow")
    }

    fn emit_start(&mut self, kind: NodeKind) {
        self.output.events.push(Event::Start { kind });
        self.depth += 1;
    }

    fn emit_end(&mut self) {
        self.output.events.push(Event::End);
        self.depth.checked_sub(1).expect("unbalanced events");
    }

    fn emit_leaf(&mut self, token: TokenIndex) {
        self.output.events.push(Event::Leaf {
            token,
            token_kind: self.input.get_kind(token),
        });
    }

    fn emit_unexpected(&mut self, token: TokenIndex) {
        self.output.events.push(Event::Unexpected {
            token,
            token_kind: self.input.get_kind(token),
        });
    }

    fn emit_missing(&mut self, kind: NodeKind) {
        self.output.events.push(Event::Missing { kind });
    }

    fn eat(&mut self) {
        self.emit_leaf(self.cursor);
        self.cursor += 1;
    }

    fn try_eat(&mut self, token_kind: TokenKind) -> bool {
        if self.current() == token_kind {
            self.eat();
            true
        } else {
            false
        }
    }

    fn eat_or_panic(&mut self, token_kind: TokenKind) {
        if !self.try_eat(token_kind) {
            panic!("expected {:?}, found {:?}", token_kind, self.current());
        }
    }

    fn eat_expect(&mut self, token_kind: TokenKind) {
        if !self.try_eat(token_kind) {
            self.emit_unexpected(self.cursor);
            self.cursor += 1;
        }
    }

    fn eat_or_recover(&mut self, token_kind: TokenKind) -> bool {
        if self.current() == token_kind {
            self.eat();
            return true;
        }

        let start = self.cursor;
        let recovery_precedence = TokenPrecedence::from(token_kind);

        let mut recovered = false;
        while !self.hit_eof() {
            let current = self.current();
            if current == token_kind {
                recovered = true;
                break;
            }
            if TokenPrecedence::from(current) >= recovery_precedence {
                recovered = false;
                break;
            }
            self.cursor += 1;
        }

        if recovered {
            for i in start.value()..self.cursor.value() {
                self.emit_unexpected(TokenIndex::from(i as usize));
            }
            self.eat();
        } else {
            self.cursor = start;
        }

        recovered
    }

    fn current(&self) -> TokenKind {
        self.input.tokens.get(self.cursor).kind()
    }

    fn peek(&self) -> TokenKind {
        self.input.tokens.get(self.cursor + 1).kind()
    }

    fn hit_eof(&self) -> bool {
        self.current() == TokenKind::FileEnd
    }

    fn statement_loop(&mut self) {
        if matches!(self.current(), TokenKind::RightBrace | TokenKind::FileEnd) {
            self.pop_state();
        } else {
            self.push_state(StateKind::Statement);
        }
    }

    fn statement(&mut self) {
        self.pop_state();
        self.last_statement_start = self.output.events.len();

        match self.current() {
            TokenKind::LeftBrace => self.push_state(StateKind::BlockStart),
            TokenKind::Enum => self.push_state(StateKind::EnumStart),
            _ => {
                self.emit_unexpected(self.cursor);
                self.cursor += 1;
            }
        }
    }

    fn block_start(&mut self) {
        self.pop_state();
        self.emit_start(NodeKind::Block);
        self.eat_or_panic(TokenKind::LeftBrace);
        self.push_sequence([StateKind::StatementLoop, StateKind::BlockEnd]);
    }

    fn block_end(&mut self) {
        self.pop_state();
        self.eat_or_panic(TokenKind::RightBrace);
        self.emit_end();
    }

    fn push_list_start(&mut self, kind: ListNodeKind) {
        self.push_sequence([kind.item_state, kind.loop_state, kind.end_state]);
        if self.current() == kind.separator {
            self.emit_missing(kind.item_kind);
        }
    }

    fn list_loop(&mut self, kind: ListNodeKind) {
        let this_state = self.pop_state();
        debug_assert!(this_state.kind == kind.loop_state);

        if self.try_eat(kind.separator) {
            while self.current() == kind.separator {
                self.emit_missing(kind.item_kind);
                self.eat();
            }
            if self.current() != kind.close_token {
                self.push_sequence([kind.item_state, this_state.kind]);
            }
            return;
        }

        self.eat_or_recover(kind.close_token);
    }

    fn enum_start(&mut self) {
        self.pop_state();
        self.emit_start(NodeKind::EnumDecl);
        self.eat_or_panic(TokenKind::Enum);

        self.eat_expect(TokenKind::Identifier);

        self.emit_start(NodeKind::EnumBlock);

        if !self.eat_or_recover(TokenKind::LeftBrace) {
            self.emit_end();
            self.emit_end();
            return;
        }

        self.push_list_start(ENUM_MEMBER_LIST);
    }

    fn enum_item(&mut self) {
        self.pop_state();
        self.emit_start(NodeKind::EnumMember);
        self.try_eat(TokenKind::Identifier);
        self.emit_end();
    }

    fn enum_end(&mut self) {
        self.pop_state();
        self.eat_or_recover(TokenKind::RightBrace);

        self.emit_end(); // enum block
        self.emit_end(); // enum decl
    }

    fn assign_or_expression(&mut self) {}

    fn variable_declaration(&mut self) {}

    // fn unary_expr(&mut self) -> bool {
    //     if self.current().is_prefix_operator() {
    //         self.emit_start(NodeKind::PrefixOpExpr);
    //         self.advance();
    //         self.primary_expr(false);
    //         self.emit_end();
    //         return true;
    //     }
    //
    //     self.primary_expr(true)
    // }

    // only allow postfix operators if we didn't already accept a prefix operator
    fn primary_expr(&mut self, in_prefix_op: bool) -> bool {
        todo!()
    }

    // fn primary_expr_start(&mut self) -> bool {
    //     match self.current() {
    //         TokenKind::Identifier
    //         | TokenKind::IntegerLiteral
    //         | TokenKind::RealLiteral
    //         | TokenKind::StringLiteral
    //         | TokenKind::VerbatimStringLiteral
    //         | TokenKind::HexIntegerLiteral
    //         | TokenKind::BinaryLiteral => self.advance(),
    //         TokenKind::ParenOpen => {
    //             self.emit_start(NodeKind::ParenExpr);
    //             self.advance();
    //             self.expr();
    //             self.expect(TokenKind::ParenClose);
    //             self.emit_end();
    //         }
    //         TokenKind::BracketOpen => self.delimited_list(
    //             NodeKind::ArrayExpr,
    //             TokenKind::BracketOpen,
    //             TokenKind::BracketClose,
    //             TokenKind::Comma,
    //         ),
    //         _ => return false,
    //     };
    //
    //     true
    // }

    fn expr(&mut self) -> bool {
        todo!();
    }
}

impl Display for ParseEvents {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut indent = 0;

        for event in &self.events {
            match event {
                Event::Start { kind } => {
                    for _ in 0..indent {
                        write!(f, "  ")?; // 2 spaces per level
                    }
                    writeln!(f, "Start({:?})", kind)?;
                    indent += 1;
                }
                Event::End => {
                    if indent > 0 {
                        indent -= 1;
                    }
                    for _ in 0..indent {
                        write!(f, "  ")?;
                    }
                    writeln!(f, "End")?;
                }
                Event::Leaf { token_kind, .. } => {
                    for _ in 0..indent {
                        write!(f, "  ")?;
                    }
                    writeln!(f, "Token({:?})", token_kind)?;
                }
                Event::Unexpected { token_kind, .. } => {
                    for _ in 0..indent {
                        write!(f, "  ")?;
                    }
                    writeln!(f, "Unexpected({:?})", token_kind)?;
                }
                Event::Missing { kind } => {
                    for _ in 0..indent {
                        write!(f, "  ")?;
                    }
                    writeln!(f, "Missing({:?})", kind)?;
                }
            }
        }

        Ok(())
    }
}
