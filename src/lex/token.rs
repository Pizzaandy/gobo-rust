use crate::source_text::TextSize;
use crate::typed_index;
use std::fmt::{Debug, Formatter};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Token {
    data: u32, // [ kind:8 | payload:23 | has_space:1 ]
    start: TextSize,
}

typed_index!(pub struct TokenIndex(u32));

const _: () = {
    assert!(size_of::<Token>() == 8, "expected Token to be 8 bytes");
};

impl Token {
    const PAYLOAD_MASK: u32 = (1 << 23) - 1;
    const KIND_MASK: u32 = (1 << 8) - 1;
    pub const MAX_INDEX: usize = Self::PAYLOAD_MASK as usize;

    pub fn new(kind: TokenKind, has_space: bool, payload: u32, start: TextSize) -> Self {
        debug_assert!(payload < Self::PAYLOAD_MASK);
        Self {
            data: (kind as u32 & Self::KIND_MASK)
                | ((payload & Self::PAYLOAD_MASK) << 8)
                | ((has_space as u32) << 31),
            start,
        }
    }

    pub fn kind(&self) -> TokenKind {
        unsafe { std::mem::transmute((self.data & Self::KIND_MASK) as u8) }
    }

    pub fn payload(&self) -> u32 {
        (self.data >> 8) & Self::PAYLOAD_MASK
    }

    pub fn set_payload(&mut self, payload: u32) {
        debug_assert!(payload < Self::PAYLOAD_MASK);
        // clear old bits
        self.data &= !(Self::PAYLOAD_MASK << 8);
        // set new payload
        self.data |= (payload & Self::PAYLOAD_MASK) << 8;
    }

    pub fn has_leading_space(&self) -> bool {
        (self.data >> 31) != 0
    }

    pub fn start(&self) -> TextSize {
        self.start
    }
}

impl Debug for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Token")
            .field("kind", &self.kind())
            .field("start", &self.start)
            .field("has_leading_space", &self.has_leading_space())
            .field("payload", &self.payload())
            .finish()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Error = 0,
    FileStart,
    FileEnd,
    SingleLineComment,
    MultiLineComment,
    BracketOpen,
    BracketClose,
    ListAccessor,
    MapAccessor,
    GridAccessor,
    ArrayAccessor,
    StructAccessor,
    ParenOpen,
    ParenClose,
    BraceOpen,
    BraceClose,
    Semicolon,
    Comma,
    Colon,
    Dot,
    PlusPlus,
    MinusMinus,
    Plus,
    Minus,
    BitNot,
    BitNotAssign,
    Not,
    Multiply,
    Divide,
    IntegerDivide,
    Modulo,
    Power,
    QuestionMark,
    NullCoalesce,
    NullCoalesceAssign,
    RightShift,
    LeftShift,
    LessThan,
    GreaterThan,
    LessThanEquals,
    GreaterThanEquals,
    Equals,
    NotEquals,
    BitAnd,
    BitXor,
    BitOr,
    And,
    Or,
    Xor,
    MultiplyAssign,
    DivideAssign,
    PlusAssign,
    MinusAssign,
    ModuloAssign,
    LeftShiftAssign,
    RightShiftAssign,
    BitAndAssign,
    BitXorAssign,
    BitOrAssign,
    NumberSign,
    DollarSign,
    AtSign,
    Identifier,
    BooleanLiteral,
    IntegerLiteral,
    RealLiteral,
    BinaryLiteral,
    HexIntegerLiteral,
    StringLiteral,
    VerbatimStringLiteral,
    Undefined,
    Noone,
    Break,
    Exit,
    Do,
    Case,
    Else,
    New,
    Var,
    GlobalVar,
    Catch,
    Finally,
    Return,
    Continue,
    For,
    Switch,
    While,
    Until,
    Repeat,
    Function,
    With,
    Default,
    If,
    Then,
    Throw,
    Delete,
    Try,
    Enum,
    Constructor,
    Static,
    Macro,
    MacroName,
    MacroBody,
    Define,
    Region,
    EndRegion,
    RegionName,
    UnknownDirective,
    Backslash,
    TemplateStart,
    TemplateMiddle,
    TemplateEnd,
    SimpleTemplateString,
    LineBreak,
    Whitespace,
}

impl TokenKind {
    pub fn is_comment(&self) -> bool {
        matches!(
            self,
            TokenKind::SingleLineComment | TokenKind::MultiLineComment
        )
    }

    pub fn is_assign_operator(&self) -> bool {
        matches!(
            self,
            TokenKind::Equals
                | TokenKind::MultiplyAssign
                | TokenKind::DivideAssign
                | TokenKind::PlusAssign
                | TokenKind::MinusAssign
                | TokenKind::ModuloAssign
                | TokenKind::LeftShiftAssign
                | TokenKind::RightShiftAssign
                | TokenKind::BitAndAssign
                | TokenKind::BitXorAssign
                | TokenKind::BitOrAssign
                | TokenKind::NullCoalesceAssign
        )
    }

    pub fn is_prefix_operator(&self) -> bool {
        matches!(
            self,
            TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Not
                | TokenKind::BitNot
                | TokenKind::PlusPlus
                | TokenKind::MinusMinus
                | TokenKind::New
        )
    }
}
