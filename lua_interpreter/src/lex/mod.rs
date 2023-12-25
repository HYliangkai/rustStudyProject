use std::{ io::{ Read, Bytes }, mem, iter::Peekable };

use crate::interface::Token;

/** 词法解析模块 : 将解析到string 转化成相应的Token */
#[derive(Debug)]
pub struct Lex<R: Read> {
    input: Peekable<Bytes<R>> /* 将file变成Bytes以满足迭代需要 */,
    ahead: Token /* 后一个字段 */,
}

impl<R: Read> Lex<R> {
    pub fn new(input: R) -> Self {
        return Lex { input: input.bytes().peekable(), ahead: Token::Eos };
    } /* new()基于输入文件创建语法分析器 */
    /* 开始进行词法分析:将文本转换成Token */
    pub fn next(&mut self) -> Token {
        if self.ahead == Token::Eos {
            return self.do_next();
        } else {
            return mem::replace(&mut self.ahead, Token::Eos);
            //mem::replace(&mut self.ahead, Token::Eos)的作用类同于 Option::take() :
            //将 Token::Eos赋值给self.ahead并且返回self.ahead
            //用于处理peek情况下获取的ahead数据作为next()数据,减少循环次数,增强性能
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
        /* 直接读取u8 */
        if let Some(ch) = self.next_byte() {
            let token = match ch {
                b'\0' => Token::Eos,
                b' ' | b'\r' | b'\n' | b'\t' => self.do_next(),
                b'+' => Token::Add,
                b'*' => Token::Mul,
                b'%' => Token::Mod,
                b'^' => Token::Pow,
                b'#' => Token::Len,
                b'&' => Token::BitAnd,
                b'|' => Token::BitOr,
                b'(' => Token::ParL,
                b')' => Token::ParR,
                b'{' => Token::CurlyL,
                b'}' => Token::CurlyR,
                b'[' => Token::SqurL,
                b']' => Token::SqurR,
                b';' => Token::SemiColon,
                b',' => Token::Comma,
                b'/' => self.check_ahead(b'/', Token::Idiv, Token::Div),
                b'=' => self.check_ahead(b'=', Token::Equal, Token::Assign),
                b'~' => self.check_ahead(b'=', Token::NotEq, Token::BitXor),
                b':' => self.check_ahead(b':', Token::DoubColon, Token::Colon),
                b'<' => self.check_ahead2(b'=', Token::LesEq, b'<', Token::ShiftL, Token::Less),
                b'>' => self.check_ahead2(b'=', Token::GreEq, b'>', Token::ShiftR, Token::Greater),
                b'\'' | b'"' => self.read_string(ch),
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => self.read_name(ch),
                b'0'..=b'9' => self.read_number(ch),
                b'.' => self.read_dot(),
                b'-' => self.read_sub(),
                _ => panic!("语法出错/不支持语法"),
            };

            return token;
        } else {
            Token::Eos
        }
    }

    /** 读取一个char : 利用bytes的迭代器特性轻松获取 */
    fn read_char(&mut self) -> char {
        /* self.input.next() 是消耗型的 */
        return match self.input.next() {
            Some(Ok(ch)) => ch as char,
            Some(_) => panic!("读取到错误字节"),
            None => '\0',
        };
    }

    /** 读取字符串(单字符串和双字符串) */
    fn read_string(&mut self, quote: u8) -> Token {
        let mut s = Vec::new();
        loop {
            match self.next_byte().expect("string 读取错误") {
                b'\n' => panic!("string 未完成"),
                b'\\' => s.push(self.read_escape()),
                byt if byt == quote => {
                    /* 字符串中止 */ break;
                }
                byt => s.push(byt),
            }
        }
        return Token::String(s);
    }

    /** 读取转义字符 */
    fn read_escape(&mut self) -> u8 {
        match self.next_byte().expect("转义失败") {
            b'a' => 0x07,
            b'b' => 0x08,
            b'f' => 0x0c,
            b'v' => 0x0b,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'\\' => b'\\',
            b'"' => b'"',
            b'\'' => b'\'',
            b'x' => {
                // format: \xXX
                let n1 = char::to_digit(self.next_byte().unwrap() as char, 16).unwrap();
                let n2 = char::to_digit(self.next_byte().unwrap() as char, 16).unwrap();
                (n1 * 16 + n2) as u8
            }
            ch @ b'0'..=b'9' => {
                // format: \d[d[d]]
                let mut n = char::to_digit(ch as char, 10).unwrap(); // TODO no unwrap
                if let Some(d) = char::to_digit(self.peek_byte() as char, 10) {
                    self.next_byte();
                    n = n * 10 + d;
                    if let Some(d) = char::to_digit(self.peek_byte() as char, 10) {
                        self.next_byte();
                        n = n * 10 + d;
                    }
                }
                u8::try_from(n).expect("decimal escape too large")
            }
            _ => panic!("未识别的转义字符"),
        }
    }

    /** 读取变量名 和 关键字 必须是char格式数据 */
    fn read_name(&mut self, first: u8) -> Token {
        let mut s = String::new();
        s.push(first as char); /* 变量名 */
        loop {
            let ch = self.peek_byte() as char;
            if ch.is_alphanumeric() || ch == '_' {
                self.next_byte();
                s.push(ch);
            } else {
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
    fn read_number(&mut self, num: u8) -> Token {
        /* 0开头可能是16进制数字 */
        if num == b'0' {
            let second = self.peek_byte();
            if second == b'x' || second == b'X' {
                return self.read_heximal();
            }
        }
        /* 不是16进制继往下走 */
        let mut n = (num - b'0') as i64; /* 进行类型转化 */
        loop {
            let ch = self.peek_byte();
            if let Some(x) = char::to_digit(ch as char, 10) {
                self.next_byte(); /* 原来的数乘10再加到个位上 */
                n = n * 10 + (x as i64);
            } else if ch == b'.' {
                return self.read_number_fraction(n); /* 解析小数值 */
            } else if ch == b'e' || ch == b'E' {
                return self.read_number_exp(n as f64); /* 解析科学计数法 */
            } else {
                break; /* 解析结束 */
            }
        }

        /* 处理Integer情况 */
        let fch = self.peek_byte() as char;
        if fch.is_alphabetic() || fch == '.' {
            panic!("数字格式不合法!");
        }
        return Token::Integer(n);
    }
    /** 解析小数值 */
    fn read_number_fraction(&mut self, i: i64) -> Token {
        /* 先把小数按整数算,最后再除于进的位然后加上原始量 */
        let mut n: i64 = 0;
        let mut f: f64 = 1.0;
        loop {
            let ch = self.peek_byte() as char;
            if let Some(d) = char::to_digit(ch, 10) {
                self.next_byte();
                n = n * 10 + (d as i64);
                f = f * 10.0;
            } else {
                break;
            }
        }
        return Token::Float((i as f64) + (n as f64) / f);
    }
    /** 解析16进制数字 */
    fn read_heximal(&mut self) -> Token {
        let mut hex_string = String::new();
        loop {
            let ch = self.peek_byte() as char;
            if let Some(_) = char::to_digit(ch, 10) {
                hex_string.push(ch);
                self.next_byte();
            } else {
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
        if self.peek_byte() == b'-' {
            self.next_byte();
            self.read_comment();
            return self.next();
        } else {
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
    fn check_ahead(&mut self, ahear: u8, long: Token, short: Token) -> Token {
        if self.peek_byte() == ahear {
            self.next_byte();
            long
        } else {
            short
        }
    }
    /** 读取句号 */
    fn read_dot(&mut self) -> Token {
        match self.read_char() {
            '.' => {
                if self.peek_byte() == b'.' {
                    self.next_byte();
                    return Token::Dots; /* 三个省略号 */
                } else {
                    return Token::Concat; /* 两个省略号 */
                }
            }
            '0'..='9' => {
                return self.read_number_fraction(0); /* 小数点 */
            }
            _ => {
                return Token::Dot; /* 单纯句号 */
            }
        }
    }
    /** 二级check_ahead,因为可能可能要读取两次 */
    fn check_ahead2(
        &mut self,
        ahead1: u8,
        long1: Token,
        ahead2: u8,
        long2: Token,
        short: Token
    ) -> Token {
        let ch = self.peek_byte();
        if ch == ahead1 {
            self.next_byte();
            long1
        } else if ch == ahead2 {
            self.next_byte();
            long2
        } else {
            short
        }
    }

    /** 文件读取回溯,用于read_char消费char的反悔 */
    // fn putback_char(&mut self) {
    //     self.input.seek(SeekFrom::Current(-1)).unwrap();
    // }

    /**  */
    fn _peek_char(&mut self) -> char {
        /* self.input.peek() 是非消耗型的 */
        return match self.input.peek() {
            Some(Ok(c)) => *c as char,
            Some(_) => panic!("错误的类型引用"),
            None => '\0',
        };
    }

    /** peek look a byte */
    fn peek_byte(&mut self) -> u8 {
        return match self.input.peek() {
            Some(Ok(byt)) => *byt,
            Some(_) => panic!("lex peek error"),
            None => b'\0', // good for usage
        };
    }

    /** read next byte  in consume */
    fn next_byte(&mut self) -> Option<u8> {
        return self.input.next().and_then(|it| Some(it.unwrap()));
    }
}
