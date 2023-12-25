use std::collections::HashMap;

use super::Value;

/** 数据结构:
    对外表现为统一的散列表，其索引可以是数字、字符串、或者除了Nil和Nan以外的其他所有Value类型。但为了性能考虑，对于数字类型又有特殊的处理，即使用数组来存储连续数字索引的项。
 */
pub struct Table {
    pub array: Vec<Value>,
    pub map: HashMap<Value, Value>,
}
