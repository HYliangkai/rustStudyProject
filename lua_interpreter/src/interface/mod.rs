use std::fmt;

use crate::vm;
/** ### ByteCode表示字节码
    不同的字节码表示vm在解析的时候会采取不同的方式来进行解释执行
    
    所谓解释型语言解释的就是字节码,对字节码进行解释,然后执行
 */
#[derive(Debug)]
pub enum ByteCode {
    GetGlobal(u8, u8) /* <获取>装载好的全局变量/函数 : 入栈位置,变量名在constants的index   */,
    LoadConst(u8, u8) /* <存储>全局变量 : 入栈位置,变量名在constants的index */,
    LoadNil(u8) /* 存储nil : 入栈位置 */,
    LoadBool(u8, bool) /* 存储boolean : 入栈位置,bool情况 */,
    LoadInt(u8, i64) /* 存储int : 入栈位置,int情况 */,
    Call(u8, u8) /* 函数调用 : 函数在栈的位置,参数个数 */,
    Move(u8, u8) /* 数据移动,表示数据从调用栈(后)移向(前)进行替代的行为 
    局部变量通过栈索引访问，而全局变量要实时查找全局变量表，也就是Move和GetGlobal这两个字节码的区别 */,
    SetGlobalConst(u8, u8) /* 设置全局常量 : 常量名,常量位置  */,
    SetGlobal(u8, u8) /* 设置全局变量 */,
    SetGlobalGlobal(u8, u8) /*  */,
}

/** ### Value表示lua支持的值 */
#[derive(Clone)]
pub enum Value {
    Nil /* null */,
    String(String) /* String */,
    Boolean(bool) /* Boolean */,
    Integer(i64) /* Integer */,
    Float(f64) /* Float */,
    Function(fn(&mut vm::ExeState) -> i32) /* Function */,
}
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Float(n) => write!(f, "{n:?}"),
            Value::String(s) => write!(f, "{s}"),
            Value::Function(_) => write!(f, "function"),
        }
    }
}
/** 实现Value比较 */
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::String(l0), Self::String(r0)) => *l0 == *r0,
            (Self::Boolean(l0), Self::Boolean(r0)) => *l0 == *r0,
            (Self::Integer(l0), Self::Integer(r0)) => *l0 == *r0,
            (Self::Float(l0), Self::Float(r0)) => *l0 == *r0,
            (Self::Function(l0), Self::Function(r0)) => std::ptr::eq(l0, r0),
            _ => false,
        }
    }
}

/** ### Token表示在词法分析中会遇到的字符串情况 */
#[derive(Debug, PartialEq)]
pub enum Token {
    Name(String) /* 定义的名 */,
    String(String) /* 字符串文本 */,
    Integer(i64) /* int型 */,
    Float(f64) /* float型 */,
    Eos /* 表示文件结束 */,
    /* keywords */
    And,
    Break,
    Do,
    Else,
    Elseif,
    End,
    False,
    For,
    Function,
    Goto,
    If,
    In,
    Local /* 关键字 : 表示定义一个局部变量  */,
    Nil,
    Not,
    Or,
    Repeat,
    Return,
    Then,
    True,
    Until,
    While,
    Add /* + */,
    Sub /* - */,
    Mul /* * */,
    Div /* / */,
    Mod /* & */,
    Pow /* ^ */,
    Len /* # */,
    BitAnd /* & */,
    BitXor /* ~ */,
    BitOr /* | */,
    ShiftL /* << */,
    ShiftR /* >> */,
    Idiv /* // */,
    Equal /* == */,
    NotEq /* ~= */,
    LesEq /* <= */,
    GreEq /* >= */,
    Less /* < */,
    Greater /* > */,
    Assign /* = */,
    ParL /* ( */,
    ParR /* ) */,
    CurlyL /* { */,
    CurlyR /* } */,
    SqurL /* [ */,
    SqurR /* ] */,
    DoubColon /* :: */,
    SemiColon /* ; */,
    Colon /* : */,
    Comma /* , */,
    Dot /* . */,
    Concat /* .. */,
    Dots /* ... */,
}
