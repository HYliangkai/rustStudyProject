use std::io::Read;

use crate::{ interface::{ Value, ByteCode, Token }, lex::Lex };

/** ### Lua解释器 */

/** 语法解析模块 : 将Token解析成相应的bytecode */
pub struct ParseProto<R: Read> {
    pub constants: Vec<Value> /* 常量表 */,
    pub byte_codes: Vec<ByteCode> /* 字节码表,表示各个模块的调用情况 */,
    locals: Vec<String> /* 变量表,所有进过 local 定义的变量会在里面 */,
    lex: Lex<R> /* 词法解析器本器 */,
}
impl<R: Read> ParseProto<R> {
    /** 语法解析 */
    pub fn load(file: R) -> ParseProto<R> {
        let mut proto = ParseProto {
            constants: Vec::new(),
            byte_codes: Vec::new(),
            locals: Vec::new(),
            lex: Lex::new(file),
        };
        proto.chunk();
        return proto;
    }
    /** 执行解析 */
    pub fn chunk(&mut self) {
        loop {
            /* 词法解析 */
            match self.lex.next() {
                /* Token::name 表示获取到 变量名:可能是局部变量也可能是全局变量;进入下一步判定 */
                Token::Name(name) => {
                    if self.lex.peek() == &Token::Assign {
                        /* 如果下一个Token是等于号 -> 变量赋值 */ self.assignment(name);
                    } else {
                        /* 否则就是 ->  函数调用 */ self.function_call(name);
                    }
                }
                /* 解析local关键字 */
                Token::Local => self.local(),
                /* 解析结束 */
                Token::Eos => {
                    break;
                }
                _ => panic!("unexpected token") /* 读取进行报警 */,
            }
        }
    }

    /** 函数调用 */
    fn function_call(&mut self, fn_name: String) {
        /* 获取函数的偏移量和参数的偏移量 */
        let ifunc = self.locals.len();
        let iarg = ifunc + 1;
        /* 将变量名进行载入 : 函数名本来也算是变量 */
        let code = self.load_var(ifunc, fn_name);
        self.byte_codes.push(code);
        /* 载入函数参数 : 是 LoadConst 还是 Move */
        match self.lex.next() {
            Token::ParL => {
                /* 表达式解析 */
                self.load_exp(iarg);
                if self.lex.next() != Token::ParR {
                    panic!("函数缺少 `)`");
                }
            }
            Token::String(str) => {
                /* 字符串常量 : 进行直接赋值即可 */
                let code = self.load_const(iarg as u8, str.into());
                self.byte_codes.push(code);
            }
            _ => panic!("不受支持的函数调用形式!"),
        }
        //Flag 最后加上调用行为
        self.byte_codes.push(ByteCode::Call(ifunc as u8, 1));
    }

    /** 变量赋值 */
    fn assignment(&mut self, var: String) {
        /* 消耗 `=` */
        if self.lex.next() != Token::Assign {
            panic!("变量赋值的语法错误!");
        }

        /* 先看var(左值)在locals中是否存在 */
        if let Some(dst) = self.get_local(&var) {
            /* 如果是 : 代表的是 --> 变量重新赋值操作  --> 将lex.next()的值重新赋值在dst上 */
            self.load_exp(dst);
        } else {
            /* 不在locals就代表可能全局常量中 */
            /* 先把var(左值)载入consts表 */
            let dst = self.add_const(var) as u8;
            /* 解析变量 -> byte_code */

            let code = match self.lex.next() {
                /* 下面的数据都当成常量  */
                Token::Nil => ByteCode::SetGlobalConst(dst, self.add_const(None) as u8),
                Token::True => ByteCode::SetGlobalConst(dst, self.add_const(true) as u8),
                Token::False => ByteCode::SetGlobalConst(dst, self.add_const(false) as u8),
                Token::Float(float) => ByteCode::SetGlobalConst(dst, self.add_const(float) as u8),
                Token::Integer(int) => ByteCode::SetGlobalConst(dst, self.add_const(int) as u8),
                Token::String(str) => ByteCode::SetGlobalConst(dst, self.add_const(str) as u8),
                /* 下面的当成变量  */
                Token::Name(var_name) => if let Some(i) = self.get_local(&var_name) {
                    // 已有数据 : 重新赋值
                    ByteCode::SetGlobal(dst, i as u8)
                } else {
                    // 未有数据 : 全局赋值
                    ByteCode::SetGlobalGlobal(dst, self.add_const(var_name) as u8)
                }
                /* 语句错误 : */
                _ => panic!("赋值语法错误 !"),
            };
            /* 载入byte_code */
            self.byte_codes.push(code);
        }
    }

    /** 解析local关键字 */
    fn local(&mut self) {
        /* 先获取变量名 */
        let var_name = if let Token::Name(local) = self.lex.next() {
            local
        } else {
            panic!("local的语法声明错误: local <var_name> ");
        };

        /* 过渡读取一个等于号 */
        if let Token::Assign = self.lex.next() {
        } else {
            panic!("local的语法声明错误: local <var_name> = ");
        }
        self.load_exp(self.locals.len()); /* 解析表达式 */
        self.locals.push(var_name); /* 将locals表进行推进 */
    }
    /** 解析表达式 : <包含byte_code操作> :: 将self.lex.next()的数据赋值到dst上 */
    fn load_exp(&mut self, dst: usize) {
        /* 解析Tokon : 支持解析成变量的类型如下 */
        let index = dst as u8;
        /* 获取下一个token(变量值) 并转义成相应bytecode */
        let code = match self.lex.next() {
            Token::Nil => ByteCode::LoadNil(index),
            Token::True => ByteCode::LoadBool(index, true),
            Token::False => ByteCode::LoadBool(index, false),
            Token::Integer(int) => {
                if let Ok(interger) = i16::try_from(int) {
                    ByteCode::LoadInt(index, interger as i64)
                } else {
                    self.load_const(index, Value::Integer(int as i64))
                }
            }
            Token::String(str) => self.load_const(index, str.into()),
            Token::Name(var) => self.load_var(dst, var) /* 当解析出来是变量名的时候,使用load_var */,
            _ => panic!("不受支持的语法类型"),
        };
        self.byte_codes.push(code);
    }

    /** 解析行为:载入常量进栈 */
    fn load_const(&mut self, index: u8, val: Value) -> ByteCode {
        return ByteCode::LoadConst(index, self.add_const(val) as u8);
    }

    /** 获取 name 在 local中的偏移量 */
    fn get_local(&mut self, name: &str) -> Option<usize> {
        /* 使用rposition的原因是需要使用旧变量覆盖新变量 */
        return self.locals.iter().rposition(|item| item == name);
    }

    /** 解析变量 : 存在就用Move 不存在就用GetGlobal */
    fn load_var(&mut self, dst: usize, name: String) -> ByteCode {
        if let Some(index) = self.get_local(&name) {
            /* 已存在的变量 */
            ByteCode::Move(dst as u8, index as u8) /* 将栈上index的数据作为dst的数据 */
        } else {
            /* 否则,将数据作为全局常量读取常量 */
            let idx = self.add_const(name);
            ByteCode::GetGlobal(dst as u8, idx as u8)
        }
    }

    /** 载入Value到常量表 , 并返回常量表中的索引 : 对于已有常量返回已有索引 */
    fn add_const<I: Into<Value>>(&mut self, v: I) -> usize {
        /* 时间复杂度是O(N^2) --> 后续需要优化为hashMap */
        let val = v.into();
        return self.constants
            .iter()
            .position(|v| v == &val)
            .unwrap_or_else(|| {
                self.constants.push(val);
                self.constants.len() - 1
            });
    }
}
