/** ### ExpDesc : temp ast  */
#[derive(Debug, PartialEq)]
pub enum ExpDesc {
    Nil,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(Vec<u8>) /* 字符串 */,
    Local(usize) /* 临时变量 */,
    Global(usize) /* 全局变量 */,
}


