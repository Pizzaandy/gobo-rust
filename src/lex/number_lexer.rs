use crate::lex::token::TokenKind;

pub const fn is_digit(c: u8) -> bool {
    matches!(c, b'0'..=b'9')
}

pub fn scan_number_or_dot(text: &[u8]) -> (usize, TokenKind) {
    let mut index = 0;
    let mut found_dot = false;

    while index < text.len() {
        let c = text[index];

        if matches!(c, b'0'..=b'9' | b'_') {
            index += 1;
            continue;
        }

        if c == b'.' {
            if found_dot {
                return (index, TokenKind::Error);
            }
            found_dot = true;
            index += 1;
            continue;
        }

        break;
    }

    let kind = if found_dot {
        if index == 1  {
            TokenKind::Dot
        } else {
            TokenKind::RealLiteral
        }
    } else {
        TokenKind::IntegerLiteral
    };

    (index, kind)
}
