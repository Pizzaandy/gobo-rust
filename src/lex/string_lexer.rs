use crate::lex::TokenKind;

pub fn scan_string_literal(text: &[u8]) -> (usize, TokenKind) {
    debug_assert!(text[0] == b'"');
    let mut index = 1;
    let mut unterminated = true;

    while index < text.len() {
        if text[index] == b'\\' {
            index += 2;
            continue;
        }

        if text[index] == b'"' {
            index += 1;
            unterminated = false;
            break;
        }

        if text[index] == b'\n' {
            break;
        }

        index += 1;
    }

    let kind = if unterminated {
        TokenKind::Error
    } else {
        TokenKind::StringLiteral
    };

    (index, kind)
}

pub fn scan_verbatim_string_literal(text: &[u8]) -> (usize, TokenKind) {
    debug_assert!(text[0] == b'@');
    debug_assert!(text.len() > 2);
    debug_assert!(text[1] == b'"' || text[1] == b'\'');
    let mut index = 1;
    let mut unterminated = true;

    while index < text.len() {
        if text[index] == b'"' {
            index += 1;
            if text[index] == b'"' {
                continue;
            }
            unterminated = false;
            break;
        }

        index += 1;
    }

    let kind = if unterminated {
        TokenKind::Error
    } else {
        TokenKind::VerbatimStringLiteral
    };

    (index, kind)
}
