mod chunked_index_vec;
mod lex;
mod source_text;
mod typed_index;
mod user_symbols;
mod parser;
mod fnv;

use source_text::SourceText;
use crate::user_symbols::UserSymbols;

fn main() {
    let text = SourceText::from_file(r"D:\CS_Projects\gobo-rust\src\test.txt");
    let result = lex::lex(&text);
    result.dump();
    println!("done!");
}
