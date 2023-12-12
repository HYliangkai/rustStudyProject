use std::{ fs::File, io::{ Read, SeekFrom, Seek }, mem };

use crate::interface::Token;

/** 词法解析模块 : 将解析到string 转化成相应的Token */
#[derive(Debug)]
pub struct Lex {
    input: File,
    ahead: Token /* 后一个字段 */,
}

impl Lex {
    pub fn new(input: File) -> Self {
        return Lex { input, ahead: Token::Eos };
    } /* new()基于输入文件创建语法分析器 */
    /* 开始进行词法分析:将文本转换成Token */
    pub fn next(&mut self) -> Token {
        if self.ahead == Token::Eos {
            return self.do_next();
        } else {
            //mem::replace(&mut self.ahead, Token::Eos)的作用类同于 Option::take() :
            //将 Token::Eos赋值给self.ahead并且返回self.ahead
            //用于处理peek情况下获取的ahead数据作为next()数据,减少循环次数,增强性能
            return mem::replace(&mut self.ahead, Token::Eos);
        }
    }

    /** 返回下一个Token,但是没有移动效果 */
    pub fn peek(&mut self) -> &Token {
        /* 为什么返回 &Token而不是 Token : 因为Token的所有者还是属于Lex,并不做所有权转移,同时避免使用clone增加性能开销 */
        if self.ahead == Token::Eos {
            self.ahead = self.do_next();
        }
        return &self.ahead;
    }

    /** do_next()返回下一个Token */
    fn do_next(&mut self) -> Token {
        let ch = self.read_char();
        match ch {
            '\0' => Token::Eos,
            ' ' | '\r' | '\n' | '\t' => self.next(),
            '+' => Token::Add,
            '*' => Token::Mul,
            '%' => Token::Mod,
            '^' => Token::Pow,
            '#' => Token::Len,
            '&' => Token::BitAnd,
            '|' => Token::BitOr,
            '(' => Token::ParL,
            ')' => Token::ParR,
            '{' => Token::CurlyL,
            '}' => Token::CurlyR,
            '[' => Token::SqurL,
            ']' => Token::SqurR,
            ';' => Token::SemiColon,
            ',' => Token::Comma,
            '/' => self.check_ahead('/', Token::Idiv, Token::Div),
            '=' => self.check_ahead('=', Token::Equal, Token::Assign),
            '~' => self.check_ahead('=', Token::NotEq, Token::BitXor),
            ':' => self.check_ahead(':', Token::DoubColon, Token::Colon),
            '<' => self.check_ahead2('=', Token::LesEq, '<', Token::ShiftL, Token::Less),
            '>' => self.check_ahead2('=', Token::GreEq, '>', Token::ShiftR, Token::Greater),
            '\'' | '"' => self.read_string(ch),
            'A'..='Z' | 'a'..='z' | '_' => self.read_name(ch),
            '0'..='9' => self.read_number(ch),
            '.' => self.read_dot(),
            '-' => self.read_sub(),
            _ => panic!("语法出错/不支持语法"),
        }
    }

    /** 读取一个char */
    fn read_char(&mut self) -> char {
        let mut buf: [u8; 1] = [0];
        if self.input.by_ref().read(&mut buf).unwrap() == 1 {
            return buf[0] as char;
        } else {
            return '\0';
        }
    }

    /** 读取字符串(单字符串和双字符串) */
    fn read_string(&mut self, quote: char) -> Token {
        let mut s = String::new();
        loop {
            match self.read_char() {
                '\n' | '\0' => panic!("string 未完成"),
                '\\' => todo!("escape"),
                ch if ch == quote => {
                    break;
                }
                ch => s.push(ch),
            }
        }
        return Token::String(s);
    }

    /** 读取变量名 和 关键字 */
    fn read_name(&mut self, first: char) -> Token {
        let mut s = first.to_string();
        loop {
            let ch = self.read_char();
            if ch.is_alphanumeric() || ch == '_' {
                s.push(ch);
            } else {
                self.putback_char();
                break;
            }
        }

        /* 关键字匹配 */
        match &s as &str {
            // TODO optimize by hash
            "and" => Token::And,
            "break" => Token::Break,
            "do" => Token::Do,
            "else" => Token::Else,
            "elseif" => Token::Elseif,
            "end" => Token::End,
            "false" => Token::False,
            "for" => Token::For,
            "function" => Token::Function,
            "goto" => Token::Goto,
            "if" => Token::If,
            "in" => Token::In,
            "local" => Token::Local,
            "nil" => Token::Nil,
            "not" => Token::Not,
            "or" => Token::Or,
            "repeat" => Token::Repeat,
            "return" => Token::Return,
            "then" => Token::Then,
            "true" => Token::True,
            "until" => Token::Until,
            "while" => Token::While,
            _ => Token::Name(s),
        }
    }

    /** 读取数字 */
    fn read_number(&mut self, num: char) -> Token {
        /* 0开头可能是16进制数字 */
        if num == '0' {
            let second = self.read_char();
            if second == 'x' || second == 'X' {
                return self.read_heximal();
            }
        }
        /* 不是16进制继往下走 */
        let mut n = char::to_digit(num, 10).unwrap() as i64; /* 默认是i64 */
        loop {
            let ch = self.read_char();
            if let Some(x) = char::to_digit(ch, 10) {
                /* 原来的数乘10再加到各位 */
                n = n * 10 + (x as i64);
            } else if ch == '.' {
                /* 解析小数值 */ return self.read_number_fraction(n);
            } else if ch == 'e' || ch == 'E' {
                /* 解析科学计数法 */ return self.read_number_exp(n as f64);
            } else {
                /* 解析结束 */ self.putback_char();
                break;
            }
        }
        /* 处理Integer情况 */
        let fch = self.read_char();
        if fch.is_alphabetic() || fch == '.' {
            panic!("数字格式不合法!");
        } else {
            self.putback_char();
        }
        return Token::Integer(n);
    }
    /** 解析小数值 */
    fn read_number_fraction(&mut self, i: i64) -> Token {
        /* 先把小数按整数算,最后再除于进的位然后加上原始量 */
        let mut n: i64 = 0;
        let mut f: f64 = 1.0;
        loop {
            let ch = self.read_char();
            if let Some(d) = char::to_digit(ch, 10) {
                n = n * 10 + (d as i64);
                f = f * 10.0;
            } else {
                self.putback_char();
                break;
            }
        }
        return Token::Float((i as f64) + (n as f64) / f);
    }
    /** 解析16进制数字 */
    fn read_heximal(&mut self) -> Token {
        let mut hex_string = String::new();
        loop {
            let ch = self.read_char();
            if let Some(_) = char::to_digit(ch, 10) {
                hex_string.push(ch);
            } else {
                self.putback_char();
                break;
            }
        }
        return Token::Integer(
            i64::from_str_radix(hex_string.as_str(), 16).expect("16进制数字解析失败")
        );
    }
    /** 解析科学计数法 */
    fn read_number_exp(&mut self, _f: f64) -> Token {
        todo!("lex number exp")
    }
    /** 读取减号 */
    fn read_sub(&mut self) -> Token {
        if self.read_char() == '-' {
            self.read_comment();
            return self.next();
        } else {
            self.putback_char();
            return Token::Sub;
        }
    }
    fn read_comment(&mut self) {
        match self.read_char() {
            '[' => todo!("long comment"),
            _ => {
                // line comment
                loop {
                    let ch = self.read_char();
                    if ch == '\n' || ch == '\0' {
                        break;
                    }
                }
            }
        }
    }
    /** 判断下一个char是否达预期,如果是返回long,如果不是返回short,并且不进行步进 */
    fn check_ahead(&mut self, ahear: char, long: Token, short: Token) -> Token {
        if self.read_char() == ahear {
            long
        } else {
            self.putback_char();
            short
        }
    }
    /** 读取句号 */
    fn read_dot(&mut self) -> Token {
        match self.read_char() {
            '.' => {
                if self.read_char() == '.' {
                    /* 三个省略号 */ return Token::Dots;
                } else {
                    /* 两个省略号 */ self.putback_char();
                    return Token::Concat;
                }
            }
            '0'..='9' => {
                /* 小数点 */ self.putback_char();
                return self.read_number_fraction(0);
            }
            _ => {
                /* 单纯句号 */ self.putback_char();
                return Token::Dot;
            }
        }
    }
    /** 二级check_ahead,因为可能可能要读取两次] */
    fn check_ahead2(
        &mut self,
        ahead1: char,
        long1: Token,
        ahead2: char,
        long2: Token,
        short: Token
    ) -> Token {
        let ch = self.read_char();
        if ch == ahead1 {
            long1
        } else if ch == ahead2 {
            long2
        } else {
            self.putback_char();
            short
        }
    }

    /** 文件读取回溯,用于read_char消费char的反悔 */
    fn putback_char(&mut self) {
        self.input.seek(SeekFrom::Current(-1)).unwrap();
    }
}
