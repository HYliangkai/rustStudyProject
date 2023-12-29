use std::{ collections::HashMap, cmp::Ordering, io::Read, rc::Rc, cell::RefCell };
use crate::{ interface::{ Value, ByteCode, table::Table }, global::lib_print, parse::ParseProto };

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
    pub fn execute<R: Read>(&mut self, proto: &ParseProto<R>) {
        /* proto.constants作为常量表存储在proto中而不是虚拟机的global中 */
        /* 虚拟机执行就是解析语法分析产生的字节码 */
        println!("constants is : {:?}", proto.constants);
        println!("----and----");
        println!("bytecodes is : {:?}", proto.byte_codes);
        println!("------------------------");
        for code in proto.byte_codes.iter() {
            /* 解析字节码 */
            match *code {
                /* 第一个参数是目标栈索引,第二个参数是全局变量名在全局变量中的索引 */
                ByteCode::GetGlobal(dst, name) => {
                    let name: &str = (&proto.constants[name as usize]).into();
                    let val = self.globals.get(name).unwrap_or(&Value::Nil).clone();
                    self.set_stack(dst.into(), val);
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
                    /* 将Value里面的数据转化成 &str */
                    let name = (&proto.constants[name as usize]).into();
                    let value = self.stack[src as usize].clone();
                    self.globals.insert(name, value);
                }
                /* 设置全局常量 : 区别是数据都从constants获取 */
                ByteCode::SetGlobalConst(name, src) => {
                    let name = (&proto.constants[name as usize]).into();
                    let value = proto.constants[src as usize].clone();
                    self.globals.insert(name, value);
                }
                /* 设置全局字面量 :  */
                ByteCode::SetGlobalGlobal(name, src) => {
                    let name = (&proto.constants[name as usize]).into();
                    let src: &str = (&proto.constants[src as usize]).into();
                    let value = self.globals.get(src).unwrap_or(&Value::Nil).clone();
                    self.globals.insert(name, value);
                }
                ByteCode::NewTable(idx, al, ml) => {
                    let table = Value::Table(
                        Rc::new(RefCell::new(Table::new(al as usize, ml as usize)))
                    );
                    self.set_stack(idx, table);
                }
                ByteCode::SetTable(idx, key, value) => {
                    let key = self.stack[key as usize].clone();
                    let value = self.stack[value as usize].clone();
                    //取出table进行insert
                    if let Value::Table(table) = &self.stack[idx as usize] {
                        table.borrow_mut().map.insert(key, value);
                    } else {
                        panic!("table in stack is error place");
                    }
                }
                ByteCode::SetField(idx, key, value) => {
                    let key = proto.constants[key as usize].clone();
                    let value = self.stack[value as usize].clone();
                    if let Value::Table(table) = &self.stack[idx as usize] {
                        table.borrow_mut().map.insert(key, value);
                    } else {
                        panic!("table in stack is error place");
                    }
                }
                ByteCode::SetList(idx, arr_len) => {
                    let ivalue = (idx as usize) + 1;
                    let value = self.stack[idx as usize].clone();
                    if let Value::Table(table) = value {
                        /* 取出  ivalue ~ ivalue + arr_len 的数据并且获得可变引用 */
                        let values = self.stack.drain(ivalue..ivalue + (arr_len as usize));
                        table.borrow_mut().array.extend(values);
                    } else {
                        panic!("table in stack is error place");
                    }
                }
                _ => panic!("暂时不支持更多字节码"),
            }
        }
    }

    /** ### 入栈操作,进行位置覆盖 : 
    在 stack的dst位置载入Value */
    fn set_stack(&mut self, dst: u8, v: Value) {
        let dst = dst as usize;
        match dst.cmp(&self.stack.len()) {
            Ordering::Equal => self.stack.push(v),
            Ordering::Less => {
                self.stack[dst] = v;
            }
            Ordering::Greater => panic!("栈溢出异常"),
        }
    }
}
