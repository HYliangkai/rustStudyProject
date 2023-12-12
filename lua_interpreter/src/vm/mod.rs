use std::{ collections::HashMap, cmp::Ordering };

use crate::{ interface::{ Value, ByteCode }, global::lib_print, parse::ParseProto };

/** ## Lua虚拟机 */
pub struct ExeState {
    pub globals: HashMap<String, Value> /* 全局函数表 */,
    pub stack: Vec<Value> /* 调用栈 */,
    pub func_index: usize /* 函数调用的位置,实时更新 */,
}

impl ExeState {
    pub fn new() -> Self {
        /* 提前往堆栈中加入全局的执行函数 */
        let mut global_var: HashMap<String, Value> = HashMap::new();
        global_var.insert(String::from("print"), Value::Function(lib_print));
        return ExeState {
            globals: global_var /* 全局变量 */,
            stack: Vec::new() /* 调用栈 */,
            func_index: 0,
        };
    }

    /** 虚拟机执行 */
    pub fn execute(&mut self, proto: &ParseProto) {
        /* proto.constants作为常量表存储在proto中而不是虚拟机的global中 */
        /* 虚拟机执行就是解析语法分析产生的字节码 */
        println!("constants is : {:?}", proto.constants);
        println!("bytecodes is : {:?}", proto.byte_codes);
        println!("------------------------");
        for code in proto.byte_codes.iter() {
            /* 解析字节码 */
            match *code {
                /* 第一个参数是目标栈索引,第二个参数是全局变量名在全局变量中的索引 */
                ByteCode::GetGlobal(dst, name) => {
                    let name = &proto.constants[name as usize];
                    if let Value::String(key) = name {
                        /* 获取到函数名之后从装载好的全局函数中寻找相应匹配,如果没有就用nil做默认替代 */
                        let val = self.globals.get(key).unwrap_or(&Value::Nil).clone();
                        /* 下一步是装载到调用栈中 */
                        self.set_stack(dst, val);
                    } else {
                        panic!("获取不到全局变量");
                    }
                }
                /*  函数执行,Call */
                ByteCode::Call(func, _) => {
                    /* 确定函数位置 */
                    self.func_index = func as usize;
                    let func = &self.stack[self.func_index]; /* 获取到全局函数 */
                    if let Value::Function(f) = func {
                        f(self);
                    } else {
                        panic!("函数执行错误");
                    }
                }
                /* 将常量进行装载 */
                ByteCode::LoadConst(dst, con) => {
                    /* 先从常量表中进行复制再入栈 */
                    let val = proto.constants[con as usize].clone();
                    self.set_stack(dst, val);
                }
                /* 将boolean放入全局变量,不需要进经过proto.constants进行中间流转 */
                ByteCode::LoadBool(dst, boolean) => {
                    self.set_stack(dst, Value::Boolean(boolean));
                }
                /*  */
                ByteCode::LoadNil(dst) => {
                    self.set_stack(dst, Value::Nil);
                }
                ByteCode::LoadInt(dst, val) => {
                    self.set_stack(dst, Value::Integer(val));
                }
                /* 将栈上的数据做迁移 */
                ByteCode::Move(target, src) => {
                    let index = self.stack[src as usize].clone();
                    self.set_stack(target, index);
                }
                /* 设置全局变量 */
                ByteCode::SetGlobal(name, src) => {
                    /* locals的数据最终去到了constants中 */
                    let name = proto.constants[name as usize].clone();
                    if let Value::String(key) = name {
                        let value = self.stack[src as usize].clone();
                        self.globals.insert(key, value);
                    } else {
                        panic!("错误的语法解析!");
                    }
                }
                /* 设置全局常量 : 区别是数据都从constants获取 */
                ByteCode::SetGlobalConst(name, src) => {
                    let name = proto.constants[name as usize].clone();
                    if let Value::String(key) = name {
                        let value = proto.constants[src as usize].clone();
                        self.globals.insert(key, value);
                    } else {
                        panic!("invalid global key: {name:?}");
                    }
                }
                /* 获取二段全局常量 */
                ByteCode::SetGlobalGlobal(name, src) => {
                    let name = proto.constants[name as usize].clone();
                    if let Value::String(key) = name {
                        let src = &proto.constants[src as usize];
                        if let Value::String(src) = src {
                            let value = self.globals.get(src).unwrap_or(&Value::Nil).clone();
                            self.globals.insert(key, value);
                        } else {
                            panic!("invalid global key: {src:?}");
                        }
                    } else {
                        panic!("invalid global key: {name:?}");
                    }
                }
                // _ => panic!("暂时不支持更多字节码"),
            }
        }
    }
    /** 入栈,进行位置覆盖 : 在 stack的dst位置载入Value */
    fn set_stack(&mut self, dst: u8, v: Value) {
        let dst = dst as usize;
        match dst.cmp(&self.stack.len()) {
            Ordering::Equal => self.stack.push(v),
            Ordering::Less => {
                self.stack[dst] = v;
            }
            Ordering::Greater => panic!("fail in set_stack"),
        }
    }
}
