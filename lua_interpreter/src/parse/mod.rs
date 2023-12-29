use std::io::Read;

use crate::{
    interface::{ Value, ByteCode, Token, ConstStack, table::TableEntry },
    lex::Lex,
    exp_desc::ExpDesc,
};

/** ### Lua解释器 */

/** 语法解析模块 : 将Token解析成相应的bytecode */
pub struct ParseProto<R: Read> {
    pub constants: Vec<Value> /* 常量表 */,
    pub byte_codes: Vec<ByteCode> /* 字节码表,表示各个模块的调用情况 */,
    locals: Vec<String> /* 变量表,所有进过 local 定义的变量会在里面 */,
    lex: Lex<R> /* 词法解析器本器 */,
    sp: usize /* 指向当前栈顶位置 */,
}
impl<R: Read> ParseProto<R> {
    /** 语法解析 */
    pub fn load(file: R) -> ParseProto<R> {
        let mut proto = ParseProto {
            constants: Vec::new(),
            byte_codes: Vec::new(),
            locals: Vec::new(),
            lex: Lex::new(file),
            sp: 0,
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
                /* MayBe is a Table */
                Token::CurlyL => {
                    self.table_constructor();
                }
                _ => panic!("unexpected token") /* 读取进行报警 */,
            }
        }
    }

    /** 创建table : 由于table初始化的步骤不止一步所以返回ExpDesc代表一个需要中间处理的过程(经典包一层) */
    fn table_constructor(&mut self) -> ExpDesc {
        let index = self.sp as u8;
        self.sp += 1; // 更新sp，后续语句如需临时变量，则使用表后面的栈位置
        /* Token解析 : 所以只要负责push相应ByteCode即可 */
        let bidx = self.byte_codes.len();
        self.byte_codes.push(ByteCode::NewTable(index, 0, 0)); /*长度是随时要变的 */

        let mut narray = 0;
        let mut nmap = 0;
        loop {
            let nsp = self.sp;
            /* {    100, 200, 300;  -- list style
                x="hello", y="world";  -- record style
                [key]="vvv";  -- general style
            }  */

            /* 处理 Key */
            let entry = match self.lex.peek() {
                //Eos
                Token::CurlyR => {
                    self.lex.next();
                    break;
                }
                // [key]="value"
                Token::SqurL => {
                    self.lex.next(); /* consume */
                    let desc = self.exp(); /* read exp to desc */
                    self.lex.expect(Token::SqurR); /* consume ']' */
                    self.lex.expect(Token::Equal); /* consume '=' */

                    TableEntry::Map(match desc {
                        ExpDesc::Nil => panic!("nil cannot be table key"),
                        ExpDesc::Float(f) if f.is_nan() => panic!("nan cannot be table key"),
                        ExpDesc::Integer(i) if u8::try_from(i).is_ok() =>
                            (ByteCode::SetInt, ByteCode::SetIntConst, i as usize),
                        ExpDesc::String(_) => todo!(),
                        ExpDesc::Local(i) => (ByteCode::SetTable, ByteCode::SetTableConst, i),
                        /* 其他ExpDesc表示为栈顶变量 */
                        _ =>
                            (ByteCode::SetTable, ByteCode::SetTableConst, self.discharge_top(desc)),
                    })
                }
                // key=="value" or value
                Token::Name(_) => {
                    let name = self.read_name();
                    if let Token::Equal = self.lex.peek() {
                        /* key="value" */
                        self.lex.next();
                        /* 只能被解释为Field : 因为 */
                    } else {
                        /* value  : Array save */
                        TableEntry::Array(self.exp_with_ahead(Token::Name(name)));
                    }
                    todo!()
                }
                _ => todo!("完善其他解析"),
            };

            /* 处理Value */
            match entry {
                TableEntry::Map((stack, sconst, key)) => {
                    /*  通过判断value是需要栈操作还是常量操作来进行具体ByteCode映射 */
                    let value = self.exp();
                    let code = match self.discharge_const(value) {
                        ConstStack::Const(c) => sconst(index, key as u8, c as u8),
                        ConstStack::Stack(s) => stack(index, key as u8, s as u8),
                    };
                    self.byte_codes.push(code);
                    nmap += 1;
                    self.sp = nsp; /* sp回溯? */
                }
                TableEntry::Array(desc) => {
                    self.discharge(nsp, desc);
                    narray += 1;
                    if narray % 2 == 50 {
                        /* time to SetList */
                        self.byte_codes.push(ByteCode::SetList(index, 50));
                        self.sp = (index as usize) + 1; /* push byte_code then push sp */
                    }
                }
            }
        }

        self.sp = (index as usize) + 1; // 返回前，设置栈顶sp，只保留新建的表，而清理构造过程中可能使用的其他临时变量
        return ExpDesc::Local(index as usize); // 返回表的类型（栈上临时变量）和栈上的位置
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
                self.load_exp();
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
            self.load_exp();
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
        self.load_exp(); /* 解析表达式 */
        self.locals.push(var_name); /* 将locals表进行推进 */
    }

    /** 解析表达式 : <包含byte_code操作> :: 将下一个表达式数据进行解析 */
    fn load_exp(&mut self) {
        let sp = self.sp; /* 获取栈顶 */
        let desc = self.exp(); /* 转化成ExpDesc  */
        self.discharge(sp, desc); /* ExpDesc转化并推栈 */
    }

    /** 解析行为:载入常量进栈stack */
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

    /** 载入Value到常量表constants中 , 并返回常量表中的索引 : 对于已有常量返回已有索引 */
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

    /** Next Token -> ExpDesc :: 提供一个中间转化蕴含多项操作的办法 */
    fn exp(&mut self) -> ExpDesc {
        let token = self.lex.next();
        self.exp_with_ahead(token)
    }

    /* The Token  -> ExpDesc */
    fn exp_with_ahead(&mut self, token: Token) -> ExpDesc {
        return match token {
            Token::Nil => ExpDesc::Nil,
            Token::True => ExpDesc::Boolean(true),
            Token::False => ExpDesc::Boolean(false),
            Token::Integer(i) => ExpDesc::Integer(i),
            Token::Float(f) => ExpDesc::Float(f),
            Token::String(s) => ExpDesc::String(s),
            Token::Function => todo!("Function"),
            Token::CurlyL => self.table_constructor(),
            Token::Sub | Token::Not | Token::BitXor | Token::Len => todo!("unop"),
            Token::Dots => todo!("dots"),
            t => self.prefixexp(t),
        };
    }
    /* 进一步解析: token->ExpDesc */
    fn prefixexp(&mut self, token: Token) -> ExpDesc {
        todo!()
    }

    /** String<Local|Global> -> ExpDesc */
    fn simple_name(&mut self, name: String) -> ExpDesc {
        /* 判断变量名是局部还是全局变量 */
        return if let Some(idx) = self.locals.iter().rposition(|v| v == &name) {
            ExpDesc::Local(idx) /* 栈上的临时变量 */
        } else {
            ExpDesc::Global(self.add_const(name)) /* 全局变量 */
        };
    }

    /** read name  */
    fn read_name(&mut self) -> String {
        if let Token::Name(name) = self.lex.next() {
            return name;
        } else {
            panic!("next token isnot Name!")
        }
    }

    /** 将ExpDesc转化成byteCode ,然后推到指定栈dst上 */
    fn discharge(&mut self, dst: usize, desc: ExpDesc) {
        /* 将ExpDesc转化成byteCode后 推入当前栈顶 */
        let code = match desc {
            ExpDesc::Nil => ByteCode::LoadNil(dst as u8),
            ExpDesc::Boolean(b) => ByteCode::LoadBool(dst as u8, b),
            ExpDesc::Integer(i) => ByteCode::LoadInt(dst as u8, i),
            ExpDesc::Float(f) => self.load_const(dst as u8, Value::Float(f)),
            ExpDesc::String(s) => self.load_const(dst as u8, s.into()),
            ExpDesc::Local(l) => {
                //Local表示数据是从栈上获取的,所以使用Move
                if dst != l {
                    ByteCode::Move(dst as u8, l as u8)
                } else {
                    return;
                }
            }
            ExpDesc::Global(g) => {
                //Global表示数据从常量表中获取
                ByteCode::GetGlobal(dst as u8, g as u8)
            }
        };
        self.byte_codes.push(code);
        self.sp += 1;
    }

    /** 将ExpDesc推到当前栈顶 */
    fn discharge_top(&mut self, desc: ExpDesc) -> usize {
        return self.discharge_if_need(self.sp, desc);
    }

    /** 将ExpDesc推到dst位置上  
        @return 栈位置 
     */
    fn discharge_if_need(&mut self, dst: usize, desc: ExpDesc) -> usize {
        //如果位置刚好是ExpDesc::Local(l)的位置说明就是栈顶那就啥都不用做
        return if let ExpDesc::Local(i) = desc {
            i
        } else {
            self.discharge(dst, desc);
            dst
        };
    }

    /** ExpDesc -> ConStack :: 通过ExpDesc转化成对应的堆栈状态获取 */
    fn discharge_const(&mut self, desc: ExpDesc) -> ConstStack {
        return match desc {
            ExpDesc::Nil => ConstStack::Const(self.add_const(None)),
            ExpDesc::Boolean(b) => ConstStack::Const(self.add_const(b)),
            ExpDesc::Integer(i) => ConstStack::Const(self.add_const(i)),
            ExpDesc::Float(f) => ConstStack::Const(self.add_const(f)),
            ExpDesc::String(s) => ConstStack::Const(self.add_const(s)),
            _ => ConstStack::Stack(self.discharge_top(desc)),
        };
    }
}
