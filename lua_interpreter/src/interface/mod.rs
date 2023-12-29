pub mod table;

use std::{ fmt::{ self }, rc::Rc, cell::RefCell, hash::Hash, mem };
const SHORT_STR_MAX: usize = 14; // sizeof(一个Value的对齐长度(Value类型的大小是2个字节)) - 1(Enum的tag长度) - 1(用于表示string的len)
const MID_STR_MAX: usize = 48 - 1; // 48(预估的中等字符串长度,对齐) - 1(用于表示string的len)

use crate::vm;

/** 用于区分是constant取值操作还是stack取值 */
pub enum ConstStack {
    Const(usize),
    Stack(usize),
}

/** ### ByteCode表示字节码
    不同的字节码表示vm在解析的时候会采取不同的方式来进行解释执行
    
    所谓解释型语言解释的就是字节码,对字节码进行解释,然后执行
 */
#[derive(Debug)]
pub enum ByteCode {
    GetGlobal(u8, u8) /* <获取>装载好的全局变量/函数 : 入栈位置|变量名在constants的index   */,
    LoadConst(u8, u8) /* <存储>全局变量 : 入栈位置|变量名在constants的index */,
    LoadNil(u8) /* 存储nil : 入栈位置 */,
    LoadBool(u8, bool) /* 存储boolean : 入栈位置|bool情况 */,
    LoadInt(u8, i64) /* 存储int : 入栈位置|int情况 */,
    Call(u8, u8) /* 函数调用 : 函数在栈的位置|参数个数 */,
    Move(u8, u8) /* 数据移动,表示数据从调用栈(后)|移向(前)进行替代的行为 
    局部变量通过栈索引访问，而全局变量要实时查找全局变量表，也 就是Move和GetGlobal这两个字节码的区别 */,
    SetGlobalConst(u8, u8) /* 设置全局常量 : 常量名|常量位置 */,
    SetGlobal(u8, u8) /* 设置全局变量 : 常量位置|入栈位置  */,
    SetGlobalGlobal(u8, u8) /* 设置全局字面量 */,
    NewTable(u8, u8, u8) /* 新建一个table : 入栈位置|数组部分长度|Hash部分长度  */,
    SetTable(u8, u8, u8) /* <栈> 获取数据生成table : table入栈位置|key|value  */,
    SetField(u8, u8, u8) /* <栈> 设置字符串常量 : table入栈位置|key|value  */,
    SetInt(u8, u8, u8) /* <栈> 设置字符串常量 : table入栈位置|key|value */,
    SetTableConst(u8, u8, u8) /* <常量表> 获取数据生成table : table入栈位置|key|value  */,
    SetFieldConst(u8, u8, u8) /* <常量表> 设置字符串常量 : table入栈位置|key|value   */,
    SetIntConst(u8, u8, u8) /* <常量表> 设置字符串常量 : table入栈位置|key|value */,
    SetList(u8, u8) /* 把array插入到table上 : table入栈位置|array长度  */,
}

/** ### Value表示lua支持的值 */
#[derive(Clone)]
pub enum Value {
    Nil /* null */,
    Boolean(bool) /* Boolean */,
    Integer(i64) /* Integer */,
    Float(f64) /* Float */,
    Function(fn(&mut vm::ExeState) -> i32) /* Function */,
    ShortStr(u8, [u8; SHORT_STR_MAX]) /* 短长度字符串,长度为 SHORT_STR_MAX */,
    MidStr(Rc<(u8, [u8; MID_STR_MAX])>) /* 中等长度字符串,长度为 MID_STR_MAX */,
    LongStr(Rc<Vec<u8>>) /* 不限制长度字符串 */,
    Table(Rc<RefCell<table::Table>>) /* Table */,
}

/* 实现字符串的自动转换 */
/* @Key : how to  u8 change String */
impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        return vec_to_short_mid_str(&value).unwrap_or(Value::LongStr(Rc::new(value)));
    }
}
impl From<&[u8]> for Value {
    fn from(value: &[u8]) -> Self {
        return vec_to_short_mid_str(value).unwrap_or(Value::LongStr(Rc::new(value.to_vec())));
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        /* 先实现Vec<u8>,再实现String就是顺便的事情 */
        return value.into_bytes().into();
    }
}
impl From<f64> for Value {
    fn from(value: f64) -> Self {
        return Value::Float(value);
    }
}
impl From<i64> for Value {
    fn from(value: i64) -> Self {
        return Value::Integer(value);
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        return Value::Boolean(value);
    }
}
impl From<Option<()>> for Value {
    fn from(_value: Option<()>) -> Self {
        return Value::Nil;
    }
}
/** 反向转化,用于vm的的转化 */
impl From<&Value> for String {
    fn from(value: &Value) -> Self {
        /* 将value反向转化成String */
        return match value {
            Value::ShortStr(len, buf) => String::from_utf8_lossy(&buf[..*len as usize]).to_string(),
            /* 抽象中的抽象! &s.1[..s.0 as usize] */
            Value::MidStr(s) => String::from_utf8_lossy(&s.1[..s.0 as usize]).to_string(),
            Value::LongStr(s) => String::from_utf8_lossy(&s).to_string(),
            _ => panic!("不支持的转化类型"),
        };
    }
}
impl<'a> From<&'a Value> for &'a str {
    fn from(value: &'a Value) -> Self {
        return match value {
            Value::ShortStr(len, buf) => std::str::from_utf8(&buf[..*len as usize]).unwrap(),
            Value::MidStr(s) => std::str::from_utf8(&s.1[..s.0 as usize]).unwrap(),
            Value::LongStr(s) => std::str::from_utf8(&s).unwrap(),
            _ => panic!("不支持的转化类型"),
        };
    }
}
impl<'a> From<&'a Value> for &'a [u8] {
    fn from(value: &'a Value) -> Self {
        match value {
            Value::ShortStr(len, buf) => &buf[0..*len as usize],
            Value::MidStr(s) => &s.1[0..s.0 as usize],
            Value::LongStr(vec8) => vec8,
            _ => panic!("不受支持的数据类型"),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Float(n) => write!(f, "{n:?}"),
            Value::Function(_) => write!(f, "function"),
            Value::ShortStr(len, buf) => {
                let str = String::from_utf8_lossy(&buf[..*len as usize]).to_string();
                write!(f, "{str}")
            }
            Value::MidStr(s) => {
                let str = String::from_utf8_lossy(&s.1[0..s.0 as usize]).to_string();
                write!(f, "{str}")
            }
            Value::LongStr(s) => {
                let str = String::from_utf8_lossy(s).to_string();
                write!(f, "{str}")
            }
            Value::Table(t) => {
                let t = t.borrow(); /* borrow获取不可变引用 : 对RefCell 进行解包 */
                write!(f, "table : len {} - {}", t.array.len(), t.map.len())
            }
        }
    }
}
/** 实现Display  - 实现print函数显示效果 */
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Float(n) => write!(f, "{n:?}"),
            Value::ShortStr(len, buf) =>
                write!(f, "{}", String::from_utf8_lossy(&buf[..*len as usize])),
            Value::MidStr(s) => write!(f, "{}", String::from_utf8_lossy(&s.1[..s.0 as usize])),
            Value::LongStr(s) => write!(f, "{}", String::from_utf8_lossy(&s)),
            Value::Table(t) => write!(f, "table: {:?}", Rc::as_ptr(t)),
            Value::Function(_) => write!(f, "function"),
        }
    }
}
/** 实现两个Value比较 , 只需要满足自反性的相等即可 */
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Boolean(l0), Self::Boolean(r0)) => *l0 == *r0,
            (Self::Integer(l0), Self::Integer(r0)) => *l0 == *r0,
            (Self::Float(l0), Self::Float(r0)) => *l0 == *r0,
            (Self::Function(l0), Self::Function(r0)) => std::ptr::eq(l0, r0),
            _ => false,
        }
    }
}
/** Eq 主要是告诉Value<自反性、对称性和传递性>的相等,不需要实现fn只需要写好即可 */
impl Eq for Value {}
/** 实现Hash ,由于是在原基本类型上进一步包转,所以只要解包然后调用.hash()即可 */
impl Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        /* return the hash value */
        return match self {
            Value::Nil => () /* Nil不能作为table key */,
            Value::Boolean(b) => b.hash(state),
            Value::Integer(i) => i.hash(state),
            Value::Float(f) => unsafe {
                /* Rust 中的浮点类型 f32 和 f64 都支持 NaN。 然而由于NaN之间是不相等的,所以不同的NaN获取.hash()值不想等,所以不满足hash()的定义(即相同的数据获取的hash值是相等的),因此不实现.hash() */
                mem::transmute::<f64, i64>(*f).hash(state) /* 进行不安全的类型转化 */
            }
            Value::Function(f) => f.hash(state),
            Value::ShortStr(l, b) => b[0..*l as usize].hash(state),
            Value::MidStr(s) => s.1[0..s.0 as usize].hash(state),
            Value::LongStr(v) => v.hash(state),
            Value::Table(t) =>
                t
                    .as_ptr()
                    .hash(
                        state
                    ) /* t.as_ptr() : 将Rc的引用转化为指针,简而言之就是取出指针的值,从而作为Table的hash-key */,
        };
    }
}

/* 短/中长度的字符串转化 */
fn vec_to_short_mid_str(u8s: &[u8]) -> Option<Value> {
    let len = u8s.len();
    if len <= SHORT_STR_MAX {
        let mut buf = [0; SHORT_STR_MAX];
        buf[..len].copy_from_slice(u8s);
        return Some(Value::ShortStr(len as u8, buf));
    } else if len <= MID_STR_MAX {
        let mut buf = [0; MID_STR_MAX];
        buf[..len].copy_from_slice(u8s);
        return Some(Value::MidStr(Rc::new((len as u8, buf))));
    }
    return None;
}

/** ### Token表示在词法分析中会遇到的字符串情况 */
#[derive(Debug, PartialEq)]
pub enum Token {
    Name(String) /* 定义的名 */,
    String(Vec<u8>) /* 字符串文本: 以Vec<u8>为基准 */,
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
