use crate::error::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Int,
    Char,
    Float,
    Double,
    Short,
    Long,
    Unsigned,
    Signed,
    Void,
    Struct,
    Union,
    Enum,
    Const,
    Auto,
    Nullptr,
    Constexpr,
    Typeof,
    Bool,
    True,
    False,
    If,
    Else,
    While,
    For,
    Switch,
    Case,
    Default,
    Break,
    Continue,
    Return,
    Sizeof,
    Alignof,
    Atomic,
    Unsafe,

    // Identifiers and Literals
    Identifier(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    CharLiteral(char),
    StringLiteral(String),

    // Operators and Punctuators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    Ampersand,  // &
    Pipe,       // |
    Caret,      // ^
    Tilde,      // ~
    Exclamation,// !
    Equal,      // =
    EqualEqual, // ==
    BangEqual,  // !=
    Less,       // <
    LessEqual,  // <=
    Greater,    // >
    GreaterEqual,// >=
    LessLess,   // <<
    GreaterGreater,// >>
    AmpAmp,     // &&
    PipePipe,   // ||
    PlusPlus,   // ++
    MinusMinus, // --
    Arrow,      // ->
    Dot,        // .
    Question,   // ?
    Colon,      // :
    Comma,      // ,
    Semicolon,  // ;
    LParen,     // (
    RParen,     // )
    LBracket,   // [
    RBracket,   // ]
    LBrace,     // {
    RBrace,     // }

    // Special
    LineMarker { line: usize, filename: String },
    EOF,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub struct Lexer<'a> {
    source: &'a str,
    chars: std::str::CharIndices<'a>,
    current_char: Option<(usize, char)>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut chars = source.char_indices();
        let current_char = chars.next();
        Self {
            source,
            chars,
            current_char,
        }
    }

    fn advance(&mut self) {
        self.current_char = self.chars.next();
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.clone().next().map(|(_, c)| c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some((_, c)) = self.current_char {
            if c.is_whitespace() {
                self.advance();
            } else if c == '/' && self.peek_char() == Some('/') {
                // Line comment
                self.advance();
                self.advance();
                while let Some((_, c2)) = self.current_char {
                    if c2 == '\n' {
                        break;
                    }
                    self.advance();
                }
            } else if c == '/' && self.peek_char() == Some('*') {
                // Block comment
                self.advance();
                self.advance();
                while let Some((_, c2)) = self.current_char {
                    if c2 == '*' && self.peek_char() == Some('/') {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace_and_comments();

        let (start, c) = match self.current_char {
            Some(x) => x,
            None => {
                let len = self.source.len();
                return Ok(Token {
                    kind: TokenKind::EOF,
                    span: Span::new(len, len),
                });
            }
        };

        // Handle preprocessor line directives: `# <line> "<filename>"` or `#line <line> "<filename>"`
        if c == '#' && (start == 0 || (start > 0 && self.source.as_bytes()[start - 1] == b'\n')) {
            self.advance(); // consume '#'
            self.skip_whitespace_and_comments();
            
            // Check for optional "line" keyword
            let mut word = String::new();
            while let Some((_, c2)) = self.current_char {
                if c2.is_alphabetic() {
                    word.push(c2);
                    self.advance();
                } else {
                    break;
                }
            }
            self.skip_whitespace_and_comments();

            // Read line number
            let mut line_str = String::new();
            while let Some((_, c2)) = self.current_char {
                if c2.is_ascii_digit() {
                    line_str.push(c2);
                    self.advance();
                } else {
                    break;
                }
            }

            self.skip_whitespace_and_comments();

            // Read filename in quotes
            let mut filename = String::new();
            if let Some((_, '"')) = self.current_char {
                self.advance(); // consume quote
                while let Some((_, c2)) = self.current_char {
                    if c2 == '"' {
                        self.advance();
                        break;
                    }
                    filename.push(c2);
                    self.advance();
                }
            }

            // Consume rest of line
            while let Some((_, c2)) = self.current_char {
                self.advance();
                if c2 == '\n' {
                    break;
                }
            }

            if let Ok(line) = line_str.parse::<usize>() {
                return Ok(Token {
                    kind: TokenKind::LineMarker { line, filename },
                    span: Span::new(start, start),
                });
            } else {
                return self.next_token();
            }
        }

        let kind = match c {
            '+' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('+') {
                    self.advance();
                    TokenKind::PlusPlus
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('-') {
                    self.advance();
                    TokenKind::MinusMinus
                } else if self.current_char.map(|(_, c)| c) == Some('>') {
                    self.advance();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                self.advance();
                TokenKind::Star
            }
            '/' => {
                self.advance();
                TokenKind::Slash
            }
            '%' => {
                self.advance();
                TokenKind::Percent
            }
            '&' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('&') {
                    self.advance();
                    TokenKind::AmpAmp
                } else {
                    TokenKind::Ampersand
                }
            }
            '|' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('|') {
                    self.advance();
                    TokenKind::PipePipe
                } else {
                    TokenKind::Pipe
                }
            }
            '^' => {
                self.advance();
                TokenKind::Caret
            }
            '~' => {
                self.advance();
                TokenKind::Tilde
            }
            '!' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('=') {
                    self.advance();
                    TokenKind::BangEqual
                } else {
                    TokenKind::Exclamation
                }
            }
            '=' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('=') {
                    self.advance();
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                }
            }
            '<' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('=') {
                    self.advance();
                    TokenKind::LessEqual
                } else if self.current_char.map(|(_, c)| c) == Some('<') {
                    self.advance();
                    TokenKind::LessLess
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                self.advance();
                if self.current_char.map(|(_, c)| c) == Some('=') {
                    self.advance();
                    TokenKind::GreaterEqual
                } else if self.current_char.map(|(_, c)| c) == Some('>') {
                    self.advance();
                    TokenKind::GreaterGreater
                } else {
                    TokenKind::Greater
                }
            }
            '.' => {
                self.advance();
                TokenKind::Dot
            }
            '?' => {
                self.advance();
                TokenKind::Question
            }
            ':' => {
                self.advance();
                TokenKind::Colon
            }
            ',' => {
                self.advance();
                TokenKind::Comma
            }
            ';' => {
                self.advance();
                TokenKind::Semicolon
            }
            '(' => {
                self.advance();
                TokenKind::LParen
            }
            ')' => {
                self.advance();
                TokenKind::RParen
            }
            '[' => {
                self.advance();
                TokenKind::LBracket
            }
            ']' => {
                self.advance();
                TokenKind::RBracket
            }
            '{' => {
                self.advance();
                TokenKind::LBrace
            }
            '}' => {
                self.advance();
                TokenKind::RBrace
            }
            '"' => {
                self.advance(); // consume starting quote
                let mut s = String::new();
                while let Some((_, c2)) = self.current_char {
                    if c2 == '"' {
                        self.advance();
                        break;
                    }
                    if c2 == '\\' {
                        self.advance();
                        if let Some((_, c3)) = self.current_char {
                            match c3 {
                                'n' => s.push('\n'),
                                'r' => s.push('\r'),
                                't' => s.push('\t'),
                                '\\' => s.push('\\'),
                                '"' => s.push('"'),
                                '\'' => s.push('\''),
                                '0' => s.push('\0'),
                                _ => return Err(format!("Unknown escape sequence \\{}", c3)),
                            }
                            self.advance();
                        } else {
                            return Err("Unterminated string literal".to_string());
                        }
                    } else {
                        s.push(c2);
                        self.advance();
                    }
                }
                TokenKind::StringLiteral(s)
            }
            '\'' => {
                self.advance(); // consume quote
                let val = if let Some((_, c2)) = self.current_char {
                    if c2 == '\\' {
                        self.advance();
                        if let Some((_, c3)) = self.current_char {
                            let esc = match c3 {
                                'n' => '\n',
                                'r' => '\r',
                                't' => '\t',
                                '\\' => '\\',
                                '"' => '"',
                                '\'' => '\'',
                                '0' => '\0',
                                _ => return Err(format!("Unknown escape sequence \\{}", c3)),
                            };
                            self.advance();
                            esc
                        } else {
                            return Err("Unterminated char literal".to_string());
                        }
                    } else {
                        self.advance();
                        c2
                    }
                } else {
                    return Err("Empty char literal".to_string());
                };
                if let Some((_, '\'')) = self.current_char {
                    self.advance();
                } else {
                    return Err("Expected closing quote for character literal".to_string());
                }
                TokenKind::CharLiteral(val)
            }
            _ if c.is_ascii_digit() => {
                let mut num_str = String::new();
                let mut is_hex = false;
                let mut is_float = false;

                if c == '0' && (self.peek_char() == Some('x') || self.peek_char() == Some('X')) {
                    is_hex = true;
                    num_str.push(c);
                    self.advance(); // consume '0'
                    num_str.push(self.current_char.unwrap().1);
                    self.advance(); // consume 'x'
                }

                while let Some((_, c2)) = self.current_char {
                    if is_hex {
                        if c2.is_ascii_hexdigit() {
                            num_str.push(c2);
                            self.advance();
                        } else {
                            break;
                        }
                    } else {
                        if c2.is_ascii_digit() {
                            num_str.push(c2);
                            self.advance();
                        } else if c2 == '.' {
                            is_float = true;
                            num_str.push(c2);
                            self.advance();
                        } else if c2 == 'e' || c2 == 'E' {
                            is_float = true;
                            num_str.push(c2);
                            self.advance();
                            if let Some((_, c3)) = self.current_char {
                                if c3 == '+' || c3 == '-' {
                                    num_str.push(c3);
                                    self.advance();
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }

                // Consume suffixes (like U, L, F, UL, LL, etc.)
                let mut suffix = String::new();
                while let Some((_, c2)) = self.current_char {
                    if c2 == 'u' || c2 == 'U' || c2 == 'l' || c2 == 'L' || c2 == 'f' || c2 == 'F' {
                        suffix.push(c2);
                        self.advance();
                    } else {
                        break;
                    }
                }

                if is_float || suffix.contains('f') || suffix.contains('F') {
                    let val = num_str.parse::<f64>().map_err(|e| e.to_string())?;
                    TokenKind::FloatLiteral(val)
                } else {
                    let val = if is_hex {
                        i64::from_str_radix(num_str.trim_start_matches("0x").trim_start_matches("0X"), 16)
                            .map_err(|e| e.to_string())?
                    } else {
                        num_str.parse::<i64>().map_err(|e| e.to_string())?
                    };
                    TokenKind::IntLiteral(val)
                }
            }
            _ if c.is_alphabetic() || c == '_' => {
                let mut word = String::new();
                while let Some((_, c2)) = self.current_char {
                    if c2.is_alphanumeric() || c2 == '_' {
                        word.push(c2);
                        self.advance();
                    } else {
                        break;
                    }
                }

                match word.as_str() {
                    "int" => TokenKind::Int,
                    "char" => TokenKind::Char,
                    "float" => TokenKind::Float,
                    "double" => TokenKind::Double,
                    "short" => TokenKind::Short,
                    "long" => TokenKind::Long,
                    "unsigned" => TokenKind::Unsigned,
                    "signed" => TokenKind::Signed,
                    "void" => TokenKind::Void,
                    "struct" => TokenKind::Struct,
                    "union" => TokenKind::Union,
                    "enum" => TokenKind::Enum,
                    "const" => TokenKind::Const,
                    "auto" => TokenKind::Auto,
                    "nullptr" => TokenKind::Nullptr,
                    "constexpr" => TokenKind::Constexpr,
                    "typeof" => TokenKind::Typeof,
                    "bool" => TokenKind::Bool,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "for" => TokenKind::For,
                    "switch" => TokenKind::Switch,
                    "case" => TokenKind::Case,
                    "default" => TokenKind::Default,
                    "break" => TokenKind::Break,
                    "continue" => TokenKind::Continue,
                    "return" => TokenKind::Return,
                    "sizeof" => TokenKind::Sizeof,
                    "alignof" => TokenKind::Alignof,
                    "_Atomic" => TokenKind::Atomic,
                    "__unsafe" => TokenKind::Unsafe,
                    _ => TokenKind::Identifier(word),
                }
            }
            _ => {
                return Err(format!("Unexpected character: '{}'", c));
            }
        };

        let end = match self.current_char {
            Some((idx, _)) => idx,
            None => self.source.len(),
        };

        Ok(Token {
            kind,
            span: Span::new(start, end),
        })
    }
}
