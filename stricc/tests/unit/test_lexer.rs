use stricc::lexer::{Lexer, TokenKind};

fn lex_all(source: &str) -> Result<Vec<TokenKind>, String> {
    let mut lexer = Lexer::new(source);
    let mut kinds = Vec::new();
    loop {
        let token = lexer.next_token()?;
        if token.kind == TokenKind::EOF {
            break;
        }
        kinds.push(token.kind);
    }
    Ok(kinds)
}

#[test]
fn test_lexer_keywords() {
    let source = "int char float double short long unsigned signed void struct union enum const auto nullptr constexpr typeof bool true false if else while for switch case default break continue return sizeof alignof _Atomic __unsafe restrict";
    let kinds = lex_all(source).unwrap();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Int,
            TokenKind::Char,
            TokenKind::Float,
            TokenKind::Double,
            TokenKind::Short,
            TokenKind::Long,
            TokenKind::Unsigned,
            TokenKind::Signed,
            TokenKind::Void,
            TokenKind::Struct,
            TokenKind::Union,
            TokenKind::Enum,
            TokenKind::Const,
            TokenKind::Auto,
            TokenKind::Nullptr,
            TokenKind::Constexpr,
            TokenKind::Typeof,
            TokenKind::Bool,
            TokenKind::True,
            TokenKind::False,
            TokenKind::If,
            TokenKind::Else,
            TokenKind::While,
            TokenKind::For,
            TokenKind::Switch,
            TokenKind::Case,
            TokenKind::Default,
            TokenKind::Break,
            TokenKind::Continue,
            TokenKind::Return,
            TokenKind::Sizeof,
            TokenKind::Alignof,
            TokenKind::Atomic,
            TokenKind::Unsafe,
            TokenKind::Restrict,
        ]
    );
}

#[test]
fn test_lexer_operators_and_punctuators() {
    let source = "+ ++ - -- -> * / % & && | || ^ ~ ! != = == < <= > >= << >> . ? : , ; ( ) [ ] { }";
    let kinds = lex_all(source).unwrap();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Plus,
            TokenKind::PlusPlus,
            TokenKind::Minus,
            TokenKind::MinusMinus,
            TokenKind::Arrow,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Ampersand,
            TokenKind::AmpAmp,
            TokenKind::Pipe,
            TokenKind::PipePipe,
            TokenKind::Caret,
            TokenKind::Tilde,
            TokenKind::Exclamation,
            TokenKind::BangEqual,
            TokenKind::Equal,
            TokenKind::EqualEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::LessLess,
            TokenKind::GreaterGreater,
            TokenKind::Dot,
            TokenKind::Question,
            TokenKind::Colon,
            TokenKind::Comma,
            TokenKind::Semicolon,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::LBrace,
            TokenKind::RBrace,
        ]
    );
}

#[test]
fn test_lexer_identifiers_and_literals() {
    let source = "my_var123 42 0x2A 0X2a 1.23 2.5f 'c' '\\n' '\\t' '\\0' \"hello\\nworld\"";
    let kinds = lex_all(source).unwrap();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Identifier("my_var123".to_string()),
            TokenKind::IntLiteral(42),
            TokenKind::IntLiteral(42),
            TokenKind::IntLiteral(42),
            TokenKind::FloatLiteral(1.23),
            TokenKind::FloatLiteral(2.5),
            TokenKind::CharLiteral('c'),
            TokenKind::CharLiteral('\n'),
            TokenKind::CharLiteral('\t'),
            TokenKind::CharLiteral('\0'),
            TokenKind::StringLiteral("hello\nworld".to_string()),
        ]
    );
}

#[test]
fn test_lexer_preprocessor_line_marker() {
    let source = "# 123 \"foo.c\"\n#line 456 \"bar.c\"\n";
    let kinds = lex_all(source).unwrap();
    assert_eq!(
        kinds,
        vec![
            TokenKind::LineMarker {
                line: 123,
                filename: "foo.c".to_string()
            },
            TokenKind::LineMarker {
                line: 456,
                filename: "bar.c".to_string()
            },
        ]
    );
}

#[test]
fn test_lexer_comments_and_whitespace() {
    let source = "  // line comment\n  int /* block\ncomment */ char";
    let kinds = lex_all(source).unwrap();
    assert_eq!(kinds, vec![TokenKind::Int, TokenKind::Char,]);
}

#[test]
fn test_lexer_errors() {
    // Unknown escape sequence in string
    assert!(lex_all("\"hello \\x world\"").is_err());
    // Unterminated string parses up to EOF
    assert_eq!(
        lex_all("\"hello").unwrap(),
        vec![TokenKind::StringLiteral("hello".to_string())]
    );
    // Unknown escape sequence in char
    assert!(lex_all("'\\x'").is_err());
    // Empty char literal
    assert!(lex_all("''").is_err());
    // Expected closing quote
    assert!(lex_all("'a").is_err());
    // Unexpected character
    assert!(lex_all("@").is_err());
}
