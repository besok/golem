use crate::tree::parser::ast::message::Number;
use logos::Lexer;
use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(subpattern digit = r"[0-9]([0-9_]*[0-9])?")]
#[logos(subpattern letter = r"[a-zA-Z_]")]
#[logos(subpattern exp = r"[eE][+-]?[0-9]+")]
pub enum Token {
    #[regex(r"(?i)(?&letter)((?&letter)|(?&digit))*", parse_id)]
    Id(String),

    #[regex(r#""(?:[^"\\]|\\.)*""#, parse_qt_lit)]
    StringLit(String),

    #[regex(r"-?(?&digit)", number)]
    #[regex(r"-?(?&digit)(?&exp)", number)]
    #[regex(r"-?(?&digit)?\.(?&digit)(?&exp)?[fFdD]?", float)]
    #[regex(r"0[bB][01][01]*", binary)]
    #[regex(r"-?0x[0-9a-f](([0-9a-f]|[_])*[0-9a-f])?", hex)]
    Digit(Number),

    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    #[token("=")]
    Assign,

    #[token("=>")]
    AssignArr,

    #[token("[")]
    LBrack,

    #[token("]")]
    RBrack,

    #[token(":")]
    Colon,

    #[token(";")]
    Semi,

    #[token(",")]
    Comma,

    #[token("..")]
    DotDot,

    #[token("false")]
    False,

    #[token("true")]
    True,

    #[token("array")]
    ArrayT,

    #[token("num")]
    NumT,
    #[token("object")]
    ObjectT,
    #[token("string")]
    StringT,
    #[token("any")]
    AnyT,
    #[token("bool")]
    BoolT,
    #[token("tree")]
    TreeT,

    #[token("import")]
    Import,

    #[regex(r"(?s)/\*[^*/]*\*/", logos::skip)]
    #[regex(r"//[^\r\n]*", logos::skip)]
    Comment,

    #[regex(r"[ \t\r\n\u000C\f]+", logos::skip)]
    Whitespace,
}

fn number(lex: &mut Lexer<Token>) -> Option<Number> {
    lex.slice().parse::<i64>().map(Number::Int).ok()
}

fn float(lex: &mut Lexer<Token>) -> Option<Number> {
    lex.slice().parse::<f64>().map(Number::Float).ok()
}

fn binary(lex: &mut Lexer<Token>) -> Option<Number> {
    isize::from_str_radix(&lex.slice()[2..], 2)
        .map(Number::Binary)
        .ok()
}

fn hex(lex: &mut Lexer<Token>) -> Option<Number> {
    i64::from_str_radix(lex.slice().trim_start_matches("0x"), 16)
        .map(Number::Hex)
        .ok()
}

fn parse_qt_lit(lexer: &mut Lexer<Token>) -> String {
    let qt_lit: &str = lexer.slice();
    qt_lit[1..qt_lit.len() - 1].to_string()
}
fn parse_id(lexer: &mut Lexer<Token>) -> String {
    let qt_lit: &str = lexer.slice();
    qt_lit.to_string()
}

#[cfg(test)]
mod tests {
    use crate::tree::parser::ast::message::Number;
    use crate::tree::parser::lexer::Token;
    use parsit::test::lexer_test as lt;

    #[test]
    fn number() {
        lt::expect::<Token>(r#"1"#, vec![Token::Digit(Number::Int(1))]);
        lt::expect::<Token>(r#"1.1"#, vec![Token::Digit(Number::Float(1.1))]);
        lt::expect::<Token>(
            r#"1000000.000001"#,
            vec![Token::Digit(Number::Float(1000000.000001))],
        );
    }
    #[test]
    fn string() {
        lt::expect::<Token>(
            "\"C:\\projects\"",
            vec![Token::StringLit("C:\\projects".to_string())],
        );
    }
}
