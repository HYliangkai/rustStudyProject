use crate::vm::ExeState;

pub fn lib_print(state: &mut ExeState) -> i32 {
    /* 只接受一个参数,所以说的 func_index +1  */
    println!("{:?}", state.stack[state.func_index + 1]);
    return 0; /* 返回0表示不返回任何数据 */
}
