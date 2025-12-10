use gobo_rust::lex;
use gobo_rust::parser;
use gobo_rust::source_text::SourceText;

fn main() {
    static SOURCE: &str = include_str!("test.gml");
    let text = SourceText::from_str(SOURCE);

    let lex_result = lex::lex(&text);
    let parse_result = parser::parse(&lex_result);

    for event in &parse_result.events {
        println!("{:?}", event);
    }

    println!("events: {}, tokens: {}", parse_result.events.len(), lex_result.tokens.len());
}
