use gobo_rust::lex;
use gobo_rust::parse;
use gobo_rust::source_text::SourceText;

fn main() {
    static SOURCE: &str = include_str!("test.gml");
    let text = SourceText::from_str(SOURCE);

    let lex_result = lex::lex(&text);
    let parse_result = parse::parse(&lex_result);

    println!("{}", &parse_result);

    println!("events: {}, tokens: {}", parse_result.events.len(), lex_result.token_count());
}
