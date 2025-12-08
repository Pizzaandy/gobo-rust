use crate::chunked_index_vec::ChunkedIndexVec;
use crate::lex::token::{Token, TokenIndex};
use crate::source_text::TextSize;
use crate::typed_index;

type Error = &'static str;

pub struct TokenizedText {
    pub tokens: ChunkedIndexVec<Token, TokenIndex>,
    pub comments: ChunkedIndexVec<Comment, CommentIndex>,
    pub lines: ChunkedIndexVec<Line, LineIndex>,
    pub diagnostics: Vec<Error>,
    pub last_line_is_inserted: bool,
}

impl TokenizedText {
    pub fn new() -> TokenizedText {
        TokenizedText {
            tokens: ChunkedIndexVec::new(),
            comments: ChunkedIndexVec::new(),
            lines: ChunkedIndexVec::new(),
            diagnostics: Vec::new(),
            last_line_is_inserted: false,
        }
    }

    pub fn find_line_index(&self, position: TextSize) -> LineIndex {
        debug_assert!(self.lines.len() > 0);

        let mut left = 0;
        let mut right = self.lines.len();

        while left < right {
            let mid = (left + right) / 2;
            if self.lines.get(mid.into()).start() <= position {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        let mut index = left.checked_sub(1).expect("index must be >= 0");

        // Don't place any tokens on the fake line we added at the end
        let is_last = index == self.lines.len() - 1;
        if is_last && index != 0 && self.last_line_is_inserted {
            index -= 1;
        }

        let line_index = LineIndex::from(index);
        debug_assert!(self.lines.get(line_index).start() <= position);
        line_index
    }

    pub fn get_column_number(&self, token: TokenIndex) -> u32 {
        let token_info = self.tokens.get(token);
        let line_info = self.lines.get(self.find_line_index(token_info.start()));
        (token_info.start() + line_info.start() + 1).value()
    }

    pub fn get_line_number(&self, token: TokenIndex) -> u32 {
        let token_info = self.tokens.get(token);
        (self.find_line_index(token_info.start()) + 1).value()
    }

    pub fn get_loc(&self, token: TokenIndex) -> (u32, u32) {
        (self.get_line_number(token), self.get_column_number(token))
    }

    pub fn dump(&self) {
        for (index, token) in self.tokens.iter() {
            let (line, col) = self.get_loc(index);
            println!("{:?}:{:?} {:?}", line, col, token);
        }
    }
}

pub struct Comment {
    start: TextSize,
    end: TextSize,
}
typed_index!(pub struct CommentIndex(u32));

impl Comment {
    pub fn new(start: TextSize, end: TextSize) -> Self {
        Comment { start, end }
    }

    pub fn start(&self) -> TextSize {
        self.start
    }

    pub fn end(&self) -> TextSize {
        self.end
    }
}

pub struct Line {
    start: TextSize,
    indent: u32,
}
typed_index!(pub struct LineIndex(u32));

impl Line {
    pub fn new(start: TextSize) -> Self {
        Line { start, indent: 0 }
    }

    pub fn start(&self) -> TextSize {
        self.start
    }

    pub fn indent(&self) -> u32 {
        self.indent
    }

    pub fn set_indent(&mut self, indent: u32) {
        self.indent = indent;
    }
}
