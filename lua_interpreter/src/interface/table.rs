use std::collections::HashMap;

use crate::exp_desc::ExpDesc;

use super::{ Value, ByteCode };

/** 数据结构:
    对外表现为统一的散列表，其索引可以是数字、字符串、或者除了Nil和Nan以外的其他所有Value类型。但为了性能考虑，对于数字类型又有特殊的处理，即使用数组来存储连续数字索引的项。
 */
pub struct Table {
    pub array: Vec<Value>,
    pub map: HashMap<Value, Value>,
}

impl Table {
    pub fn new(array_len: usize, hm_len: usize) -> Self {
        return Table {
            array: Vec::with_capacity(array_len),
            map: HashMap::with_capacity(hm_len),
        };
    }
}

pub enum TableEntry {
    /* 枚举在rust中可以被看作是一个函数类型 , 因此 ByteCode::SetTable就可以看做是一个函数，其参数类型就是fn(u8,u8,u8)->ByteCode --> 所以可以使用一个 <function sign> 来表示一个 <enum shape> */
    Map((fn(u8, u8, u8) -> ByteCode, fn(u8, u8, u8) -> ByteCode, usize)),
    Array(ExpDesc),
}
