use std::{ env, fs::File, io::BufReader };

use crate::parse::ParseProto;
mod vm;
mod lex;
mod global;
mod parse;
mod interface;
mod exp_desc;

/** 程序入口,接受一个lua文件地址,然后解释执行 */
fn main() {
    println!("开始执行🚀");
    println!("------------------------");
    /* 获取源文件 */
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("找不到执行文件");
        return;
    }
    /* 执行代码:
    1.词法解析 -> Token
    2.语法解析 -> 字节码 + 常量表
    3.vm解释执行字节码 -> 调用本地rust函数进行执行
    4.得出结果
    */

    let file: File = File::open(&args[1]).unwrap(); /* read arg */
    let proto = ParseProto::load(BufReader::new(file)); /* load file with ParseProto  */
    vm::ExeState::new().execute(&proto); /* vm execute to result  */
}
