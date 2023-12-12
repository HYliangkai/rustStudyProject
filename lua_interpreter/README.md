#用Rust实现一个 [lua解释器](https://wubingzheng.github.io/build-lua-in-rust/zh/)



### 解释型语言的执行过程

![image-20231124173337342](https://chzky-1312081881.cos.ap-nanjing.myqcloud.com/note/image-20231124173337342.png)

Lua的官方实现步骤: 

![image-20231124173551588](https://chzky-1312081881.cos.ap-nanjing.myqcloud.com/note/image-20231124173551588.png)

由此可以明确我们的解释器的主要功能组件：词法分析、语法分析和虚拟机.可以把词法分析和语法分析合并称为“解析”过程，而虚拟机是“执行”的过程，那么字节码就是连接这两个过程的纽带。解析和执行两个过程相对独立。接下来我们就以字节码作为突破口，开始实现我们的解释器。

### 字节码

解释器流程分为解析和执行两个阶段。那么我们就可以从字节码入手：

- 先确定字节码，
- 然后让解析过程（词法分析和语法分析）努力生成这套字节码，
- 再让执行过程（虚拟机）努力执行这套字节码

#### Lua官方的字节码

Lua官方实现自带一个非常好用的工具，`luac`，即Lua Compiler，把源代码翻译为字节码并输出。是我们这个项目的最得力助手。看下其对"hello, world!"程序的输出：

```
$ luac -l hello_world.lua

main <hello_world.lua:0,0> (5 instructions at 0x600000d78080)
0+ params, 2 slots, 1 upvalue, 0 locals, 2 constants, 0 functions
    1	[1]	VARARGPREP	0
    2	[1]	GETTABUP 	0 0 0	; _ENV "print"
    3	[1]	LOADK    	1 1	; "hello, world!"
    4	[1]	CALL     	0 2 1	; 1 in 0 out
    5	[1]	RETURN   	0 1 1	; 0 out
```

输出的前面2行看不懂，先忽略。后面应该就是字节码了，还有注释，太棒了。不过还是看不懂。查看Lua的[官方手册](https://www.lua.org/manual/5.4/)，但是发现找不到任何关于字节码的说明。原来Lua的语言标准只是定义了语言的特性，而字节码属于“具体实现”的部分，就像解释器代码里的变量命名一样，并不属于Lua标准的定义范围。事实上完全兼容Lua 5.1的Luajit项目就用了一套[完全不一样的字节码](https://github.com/LuaJIT/LuaJIT/blob/v2.1/src/lj_bc.h)。我们甚至可以不用字节码来实现解释器，呃，扯远了。既然手册没有说明，那就只能查看Lua官方实现的[代码注释](https://github.com/lua/lua/blob/v5.4.0/lopcodes.h#L196)。这里只介绍上面出现的5个字节码：

1. VARARGPREP，暂时用不到，忽略。
2. GETTABUP，这个有些复杂，可以暂时理解为：加载全局变量到栈上。3个参数分别是作为目标地址的栈索引（0）、忽略、全局变量名在常量表里的索引（0）。后面注释里列出了全局变量名是"print"。
3. LOADK，加载常量到栈上。2个参数分别是作为目的地址的栈索引（1），和作为加载源的常量索引（1）。后面注释里列出了常量的值是"hello, world!"。
4. CALL，函数调用。3个参数分别是函数的栈索引（0）、参数个数、返回值个数。后面注释说明是1个参数，0个返回值。
5. RETURN，暂时用不到，忽略。

连起来再看一下，就是

- 首先把名为`print`的全局变量加载到栈（0）位置；
- 然后把字符串常量`"hello, world!"`加载到栈（1）位置；
- 然后执行栈（0）位置的函数，并把栈（1）位置作为参数。

执行时的栈示意图如下：

```
  +-----------------+
0 | print           | <- 函数
  +-----------------+
1 | "hello, world!" |
  +-----------------+
  |                 |
```

我们目前只要实现上述的2、3、4这三个字节码即可。

#### 字节码的定义

首先参考Lua官方实现的格式定义。[源码](https://github.com/lua/lua/blob/v5.4.0/lopcodes.h#L13)里有对字节码格式的注释：

```
  We assume that instructions are unsigned 32-bit integers.
  All instructions have an opcode in the first 7 bits.
  Instructions can have the following formats:

        3 3 2 2 2 2 2 2 2 2 2 2 1 1 1 1 1 1 1 1 1 1 0 0 0 0 0 0 0 0 0 0
        1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0
iABC          C(8)     |      B(8)     |k|     A(8)      |   Op(7)     |
iABx                Bx(17)               |     A(8)      |   Op(7)     |
iAsBx              sBx (signed)(17)      |     A(8)      |   Op(7)     |
iAx                           Ax(25)                     |   Op(7)     |
isJ                           sJ(25)                     |   Op(7)     |

  A signed argument is represented in excess K: the represented value is
  the written unsigned value minus K, where K is half the maximum for the
  corresponding unsigned argument.
```

字节码用32bit的无符号整数表示。其中7bit是命令，其余25bit是参数。字节码一共5种格式，每种格式的参数不同。如果你喜欢这种精确到bit的控制感，也许会立即想到各种位操作，可能已经开始兴奋了。不过先不着急，先来看下Luajit的字节码格式：

```
A single bytecode instruction is 32 bit wide and has an 8 bit opcode field and
several operand fields of 8 or 16 bit. Instructions come in one of two formats:

+---+---+---+---+
| B | C | A | OP|
|   D   | A | OP|
+---+---+---+---+
```

也是32bit无符号整数，但字段的划分只精确到字节，而且只有2种格式，比Lua官方实现简单很多。在C语言里，通过定义匹配的struct和union，就可以较方便地构造和解析字节码，从而避免位操作。

既然**Lua语言没有规定字节码的格式**，那我们也可以**设计自己的字节码格式**。像这种**不同类型命令，每个命令有独特关联参数的场景，最适合使用Rust的enum**，用tag做命令，用关联的值做参数：

```rust
#[derive(Debug)]
pub enum ByteCode {
    GetGlobal(u8, u8),
    LoadConst(u8, u8),
    Call(u8, u8),
}
```

### 两个表

除了字节码，我们还需要两个表。

一个是**常量表**，在解析过程中存储所有遇到的常量，生成的字节码通过索引参数来引用对应的常量；在执行过程中虚拟机通过字节码里的参数来读取表中的常量。在这个例子里，遇到两个常量，一个是全局变量`print`的名字，另外一个是字符串常量"hello, world!"。这也就是上述luac的输出第2行里`2 constants`的意思了。

另一个是**全局变量表**，根据变量名称保存全局变量。虚拟机执行时，先通过字节码中参数查询常量表里的全局变量名，然后再根据名字查询全局变量表。全局变量表只在执行过程中使用（添加，读取，修改），而跟解析过程无关。

**区别:**

+ 常量是在解析的过程中可以生成的,而变量是执行的过程中生成的

### Lua的 值

Lua是动态类型语言，**“类型”是跟值绑定**，而**不是跟变量绑定**。比如下面代码第一行，等号前面变量n包含的信息是：“名字是n”；等号后面包含的信息是：“类型是整数”和“值是10”。所以在第2行还是可以把n赋值为字符串的值。

```lua
local n = 10
n = "hello" -- OK
```

作为对比，下面是静态类型语言Rust。第一行，等号前面包含的信息是：“名字是n” 和 “类型是i32”；等号后面的信息是：“值是10”。可以看到“类型”信息从变量的属性变成了值的属性。所以后续就不能把n赋值为字符串的值。

```rust
let mut n: i32 = 10;
n = "hello"; // !!! Wrong
```

所以动态语言和静态语言的区别就是在于 **类型的绑定不同** 

下面两个图分别表示动态类型和静态类型语言中，变量、值和类型之间的关系：

```
    变量                   值                     变量                   值
  +--------+          +----------+           +----------+         +----------+
  | 名称：n |--\------>| 类型：整数 |           | 名称：n   |-------->| 值： 10  |
  +--------+  |       | 值： 10   |           | 类型：整数 |   |     +---------+
              |       +----------+           +----------+    X
              |                                              |
              |       +------------+                         |    +------------+
              \------>| 类型：字符串 |                         \--->| 值："hello" |
                      | 值："hello" |                              +------------+
                      +------------+

            动态类型                                      静态类型
          类型跟值绑定                                   类型跟变量绑定
```

综上，Lua的值是包含了类型信息的。这也非常适合用enum来定义：

### 开始

目前要实现的极简解释器是非常简单的，代码很少，我当初也是把代码都写在一个文件里。不过可以预见的是，这个项目的代码量会随着功能的增加而增加。所以为了避免后续再拆文件的改动，我们直接创建多个文件：

- 程序入口：`main.rs`；
- 三个组件：词法分析 `lex.rs`、语法分析 `parse.rs`、和虚拟机 `vm.rs`；
- 两个概念：字节码 `byte_code.rs`、值`value.rs`。

## 变量

### 局部变量

局部变量如何管理，如何存储，如何访问？先参考下luac的结果：

```
main <local.lua:0,0> (6 instructions at 0x6000006e8080)
0+ params, 3 slots, 1 upvalue, 1 local, 2 constants, 0 functions
    1	[1]	VARARGPREP	0
    2	[1]	LOADK    	0 0	; "hello, world!"
    3	[2]	GETTABUP 	1 0 1	; _ENV "print"
    4	[2]	MOVE     	2 0
    5	[2]	CALL     	1 2 1	; 1 in 0 out
    6	[2]	RETURN   	1 1 1	; 0 out
```

跟上一章的直接打印"hello, world!"的程序对比，有几个区别：

- 输出的第2行里的`1 local`，说明有1个局部变量。不过这只是一个说明，跟下列字节码没关系。
- LOADK，加载常量到栈的索引0处。对应源码第[1]行，即定义局部变量。由此可见变量是存储在栈上，并在执行过程中赋值。
- GETTABUP的目标地址是1（上一章里是0），也就是把`print`加载到位置1，因为位置0用来保存局部变量。
- MOVE，新字节码，用于栈内值的复制。2个参数分别是目的索引和源索引。这里就是把索引0的值复制到索引2处。就是把局部变量a作为print的参数。

由此可知，执行过程中局部变量存储在栈上。在上一章里，栈只是用于**函数调用**，现在又多了**存储局部变量**的功能。相对而言局部变量是更持久的，只有在当前block结束后才失效。而函数调用是在函数返回后就失效。

#### 1. 定义局部变量

增加局部变量的处理。首先定义局部变量表`locals`。在[值和类型](https://wubingzheng.github.io/build-lua-in-rust/zh/ch01-03.value_and_type.html)一节里说明，Lua的变量只包含变量名信息，而没有类型信息，所以这个表里只保存变量名即可，定义为`Vec<String>`。另外，此表只在语法分析时使用，而在虚拟机执行时不需要，所以不用添加到`ParseProto`中。

现在要新增的定义局部变量语句的简化格式如下：

```
local Name = exp
```

这里面也包括`exp`。所以把这部分提取为一个函数`load_exp()`。那么定义局部变量对应的语法分析代码如下：

```rust
 Token::Local => { // local name = exp
        let var = if let Token::Name(var) = lex.next() {
            var  // can not add to locals now
        } else {
            panic!("expected variable");
        };

        if lex.next() != Token::Assign {
            panic!("expected `=`");
        }

        load_exp(&mut byte_codes, &mut constants, lex.next(), locals.len());

        // add to locals after load_exp()
        locals.push(var);
    }
```

#### 变量赋值
一个已经赋值的变量进行一个新的数据赋值:
`Name = exp`
等号=左边（左值）目前就是2类，局部变量和全局变量；右边就是前面章节的表达式exp，大致分为3类：常量、局部变量、和全局变量。所以这就是一个2*3的组合：

local = const，把常量加载到栈上指定位置，对应字节码LoadNil、LoadBool、LoadInt和LoadConst等。

local = local，复制栈上值，对应字节码Move。

local = global，把栈上值赋值给全局变量，对应字节码GetGlobal。

global = const，把常量赋值给全局变量，需要首先把常量加到常量表中，然后通过字节码SetGlobalConst完成赋值。

global = local，把局部变量赋值给全局变量，对应字节码SetGlobal。

global = global，把全局变量赋值给全局变量，对应字节码SetGlobalGlobal。