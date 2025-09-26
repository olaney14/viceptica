pub enum NodeLink {
    Index(usize),
    End
}

pub struct TextNode {
    pub text: String,
    pub speaker: String,
    pub next: NodeLink
}

pub struct Choice {
    pub text: String,
    pub next: NodeLink,
    pub enabled: bool
}

pub struct ChoiceNode {
    pub count: usize,
    pub choices: Vec<Choice>
}

pub enum DialogNode {
    Text(TextNode),
    Choice(ChoiceNode)
}

pub struct DialogGraph {
    pub nodes: Vec<DialogNode>,
    pub current: usize
}

pub mod parse {
    use std::collections::VecDeque;

    #[derive(Debug)]
    pub enum TokenType {
        Identifier(String),
        LeftBrace,
        RightBrace,
        String(String),
        Colon,
        Tag,
        Character, Show, Hide, Expr, Wait, Enter,
        Left, Right, Top, Bottom, Goto, Choice,
        End,
        Number(i32),
        Minus,
        Error(String)
    }

    #[derive(Debug)]
    pub struct Token {
        pub kind: TokenType,
        pub line: usize
    }

    impl Token {
        fn at(kind: TokenType, line: usize) -> Self {
            Self {
                kind, line
            }
        }
    }

    pub struct DialogTokenizer {
        lines: Vec<Vec<char>>,
        current_line: usize,
        current_char: usize
    }

    fn is_whitespace(c: char) -> bool {
        c == ' ' || c == '\t' || c == '\n' || c == '\r'
    }

    fn is_digit(c: char) -> bool {
        c >= '0' && c <= '9'
    }

    fn is_alpha(c: char) -> bool {
        (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z')
    }

    fn valid_for_ident(c: char) -> bool {
        is_alpha(c) || c == '_'
    }

    impl DialogTokenizer {
        pub fn new(source: String) -> Self {
            Self {
                lines: source.split('\n').map(|a| a.trim().to_owned().chars().collect()).collect(),
                current_line: 0, current_char: 0
            }
        }

        fn get_char(&self) -> Option<char> {
            self.lines.get(self.current_line)?.get(self.current_char).copied()
        }

        fn skip_line(&mut self) {
            self.current_line += 1;
            self.current_char = 0;
            while self.lines.get(self.current_line).map(|l| l.len()).unwrap_or(1) == 0 {
                self.current_line += 1
            }
        }

        fn consume(&mut self) {
            self.current_char += 1;
            if self.current_char >= self.lines[self.current_line].len() {
                self.skip_line();
            }
        }

        fn consume_within_line(&mut self) -> bool {
            self.current_char += 1;
            if self.current_char >= self.lines[self.current_line].len() {
                self.skip_line();
                return false;
            }
            true
        }

        fn advance_within_line(&mut self) -> Option<char> {
            if self.consume_within_line() {
                return self.get_char();
            }
            None
        }

        fn advance(&mut self) -> Option<char> {
            self.consume();
            self.get_char()
        }

        fn back(&mut self) -> Option<char> {
            if self.current_char == 0 { return None; }
            self.current_char -= 1;
            self.get_char()
        }

        fn make_token(&self, kind: TokenType) -> Token {
            Token::at(kind, self.current_line + 1)
        }

        fn consume_as_token(&mut self, kind: TokenType, chars: usize) -> Token {
            let line = self.current_line;
            self.consume_n(chars);
            Token::at(kind, line + 1)
        }

        fn string(&mut self) -> Option<Token> {
            let line = self.current_line;
            let mut c = self.advance()?;
            let mut string_contents: Vec<char> = Vec::new();

            while c != '\'' && c != '"' {
                string_contents.push(c);
                c = self.advance()?;
            }
            self.consume();

            Some(Token::at(TokenType::String(string_contents.iter().collect()), line + 1))
        }

        fn number(&mut self) -> Option<Token> {
            let mut c = self.get_char()?;
            let mut number_string = Vec::new();

            while is_digit(c) {
                number_string.push(c);
                c = self.advance()?;
            }

            number_string
                .iter().collect::<String>()
                .parse::<i32>()
                .map(|int| self.make_token(TokenType::Number(int)))
                .ok()
        }

        fn identifier(&mut self) -> Option<Token> {
            let c = self.get_char()?;
            let line = self.current_line;
            let mut contents = vec![c];
            if self.consume_within_line() {
                while valid_for_ident(self.get_char().unwrap_or('!')) {
                    contents.push(self.get_char()?);
                    self.consume_within_line();
                }
            }

            // Some(self.make_token(TokenType::Identifier(contents.iter().collect())))
            Some(Token { kind: TokenType::Identifier(contents.iter().collect()), line: line + 1 })
        }

        fn peek_ahead(&self, n: usize) -> Option<char> {
            self.lines.get(self.current_line)?.get(self.current_char + n).copied()
        }

        fn match_all(&mut self, from: &str) -> bool {
            for (i, char) in from.chars().enumerate() {
                let c = self.peek_ahead(i);
                if let Some(c) = c {
                    if c != char { return false; }
                } else {
                    return false;
                }
            }

            true
        }

        fn consume_n(&mut self, mut n: usize) {
            while n > 0 {
                self.consume();
                n -= 1;
            }
        }

        fn match_all_and_consume(&mut self, from: &str) -> bool {
            if self.match_all(from) {
                self.consume_n(from.len());
                return true;
            }
            false
        }

        fn next(&mut self) -> Option<Token> {
            use TokenType::*;

            let mut c = self.get_char()?;
            while is_whitespace(c) {
                c = self.advance()?;
            }

            // println!("[next] {}", c);

            match c {
                '/' => {
                    if self.peek_ahead(1).unwrap_or('#') == '/' {
                        self.skip_line();
                        return self.next();
                    }
                },
                '{' => return Some(self.consume_as_token(LeftBrace, 1)),
                '}' => return Some(self.consume_as_token(RightBrace, 1)),
                ':' => return Some(self.consume_as_token(Colon, 1)),
                '#' => return Some(self.consume_as_token(Tag, 1)),
                '\'' | '"' => return self.string(),
                '-' => return Some(self.consume_as_token(Minus, 1)),
                c if (is_digit(c)) => return self.number(),
                c if is_alpha(c) => {
                    match c {
                        'b' => if self.match_all_and_consume("bottom") {
                            return Some(self.make_token(Bottom))
                        },
                        'c' => if self.match_all_and_consume("character") {
                            return Some(self.make_token(Character));
                        } else if self.match_all_and_consume("choice") {
                            return Some(self.make_token(Choice));
                        },
                        'e' => if self.match_all_and_consume("enter") {
                            return Some(self.make_token(Enter))
                        } else if self.match_all_and_consume("expr") {
                            return Some(self.make_token(Expr))
                        },
                        'g' => if self.match_all_and_consume("goto") {
                            return Some(self.make_token(Goto))
                        },
                        'h' => if self.match_all_and_consume("hide") {
                            return Some(self.make_token(Hide))
                        },
                        'l' => if self.match_all_and_consume("left") {
                            return Some(self.make_token(Left))
                        },
                        'r' => if self.match_all_and_consume("right") {
                            return Some(self.make_token(Right))
                        },
                        's' => if self.match_all_and_consume("show") {
                            return Some(self.make_token(Show))
                        },
                        't' => if self.match_all_and_consume("top") {
                            return Some(self.make_token(Top))
                        },
                        'w' => if self.match_all_and_consume("wait") {
                            return Some(self.make_token(Wait))
                        },
                        _ => ()
                    }

                    return self.identifier();
                }
                _ => return Some(self.make_token(Error(format!("Unknown character `{}`", c))))
            };

            None
        }

        pub fn tokenize(mut self) -> Vec<Token> {
            let mut tokens = Vec::new();
            while let Some(next) = self.next() {
                tokens.push(next);
            }
            tokens.push(self.make_token(TokenType::End));
            tokens
        }
    }

    // pub struct DialogParser {
    //     lines: Vec<String>,
    // }

    // impl DialogParser {
    //     pub fn new(source: String) -> Self {
    //         Self {
    //             lines: source.split('\n').map(|a| a.trim().to_owned()).collect()
    //         }
    //     }

    //     /// Characters, comments 
    //     fn preprocess()
    // }
}