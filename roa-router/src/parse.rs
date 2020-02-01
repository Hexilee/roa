//use crate::err::Error;
//use crate::Path;
//use regex::{escape, Regex};
//use TokenType::*;
//
//struct Token {
//    typ: TokenType,
//    index: usize,
//    value: String,
//}
//enum TokenType {
//    Delimiter, // '/'
//    Wildcard,  // '*'
//    Variable,  // ':' [0-9, a-z, A-Z, _]+
//    Open,      // '{'
//    Close,     // '}'
//    Char,      // any character
//    End,       // '' or '?' or '#'
//}
//
//struct Variable {}
//
//impl Token {
//    pub fn new(typ: TokenType, index: usize, value: impl ToString) -> Self {
//        Self {
//            typ,
//            index,
//            value: value.to_string(),
//        }
//    }
//}
//
//fn lexer(path: &[char]) -> Vec<Token> {
//    let mut tokens = Vec::new();
//    let mut index = 0;
//    while index < path.len() {
//        match path[index] {
//            '/' => tokens.push(Token::new(Delimiter, index, '/')),
//            '*' => tokens.push(Token::new(Wildcard, index, '*')),
//            '{' => tokens.push(Token::new(Open, index, '{')),
//            '}' => tokens.push(Token::new(Close, index, '{')),
//            '?' | '#' => {
//                tokens.push(Token::new(End, index, path[index]));
//                break;
//            }
//            ':' => {
//                let start_index = index;
//                let mut var = String::new();
//                while index + 1 < path.len() {
//                    match path[index + 1] {
//                        '0'..'9' | 'a'..'z' | 'A'..'Z' | '_' => var.push(path[index + 1]),
//                        _ => break,
//                    }
//                    index += 1;
//                }
//                tokens.push(Token::new(Variable, start_index, var))
//            }
//            character => tokens.push(Token::new(Char, index, character)),
//        }
//        index += 1;
//    }
//    if index >= path.len() {
//        tokens.push(Token::new(End, index, path[index]));
//    }
//    tokens
//}
