mod chunked_index_vec;
mod lex;
mod source_text;
mod typed_index;
mod user_symbols;
mod parser;

use source_text::SourceText;
use crate::user_symbols::UserSymbols;

fn main() {
    let text = SourceText::from_file(r"D:\CS_Projects\gobo-rust\src\test.txt");
    let mut symbols = UserSymbols::new();
    let result = lex::lex(&text, &mut symbols);
    result.dump();
    println!("done!");
}
