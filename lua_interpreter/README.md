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

#### 定义局部变量

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

## 字符串优化

1. 由于字符串不支持`Copy trait` ; 所以每次赋值String都要使用使用clone()进行复制赋值,占用内存空间;

   所以需要使用`Rc<String>`来进行多引用,解决内存问题

2. Lua中的字符串有个特点，是**只读**的！如果要对字符串做处理，比如截断、连接、替换等，都会**生成新的字符串**。而**Rust的String是为可变字符串设计**的，所以用来表示只读字符串有点浪费，比如可以省掉元数据里的`cap`字段，也不用为了可能的修改而预留内存。比如上述例子里，"hello, world!"长度只有13，但申请了16的内存块。Rust中**更适合表示只读字符串的是`&str`**，即`String`的[slice](https://kaisery.github.io/trpl-zh-cn/ch04-03-slices.html)。但`&str`是个引用，并没有对字符串的所有权，而需要依附在某个字符串上。不过它有不是字符串String的引用（字符串的引用是`&String`），直观看上去，应该是`str`的引用。那`str`是个什么？好像从来没有单独出现过。

   事实上，`str`确实不能独立存在，必须跟随引用（比如`&str`）或者指针（比如`Box(str)`）。这种属于[动态大小类型](https://kaisery.github.io/trpl-zh-cn/ch19-04-advanced-types.html#动态大小类型和-sized-trait)。

   而`Rc`也是一种指针，所以就可以定义`Rc<str>`。定义如下：

   ```rust
   #[derive(Clone)]
   struct Value {
       String(Rc<str>),
   ```

   内存布局如下：

   ```
           栈             堆
       |        |
       +--------+
       |t|      |
       |-+------|
       |   Rc   +----+-->+--------+--------+-------------+
       |--------|    |   |count=2 | weak=0 |hello, world!|
       | len=13 |    |   +--------+--------+-------------+
       +--------+    |
       :        :    |
       :        :    |
       +--------+    |
       |t|      |    |
       |-+------|    |
       |   Rc   +----/
       +--------+
       | len=13 |
       +--------+
       |        |
   ```

   这个方案看上去非常好！相对于上面的`Rc<String>`方案，这个方案去掉了没用的`cap`字段，无需预留内存，而且还省去了一层指针跳转。但这个方案也有2个问题：

   首先，创建字符串时需要复制内容。之前的方案只需要复制字符串的元数据部分即可，只有3个字的长度。而这个方案要把字符串内容复制到新创建的Rc包内。想象要创建一个1M长的字符串，这个复制就很影响性能了。

   其次，就是在栈上占用2个字的空间。虽然在最早的直接使用String的方案里占用3个字的空间，问题更严重，但是可以理解为我们现在的标准提高了。目前，Value里的其他类型都最多只占用1个字（加上tag就一共是2个字），可以剧透的是后续要增加的表、UserData等类型也都只占用1个字，所以如果单独因为字符串类型而让Value的大小从2变成3，那就是浪费了。不仅占用更多内存，而且还对CPU缓存不友好。

   这个问题的关键就在于`len`跟随`Rc`一起，而不是跟随数据一起。如果能把`len`放到堆上，比如在图中`weak`和"hello, world!"之间，那就完美了。对于C语言这是很简单的，但Rust并不支持。原因在于`str`是动态大小类型。那如果选一个固定大小类型的，是不是就可以实现？比如数组。

### `[使用Rc<(u8, [u8; 47])]>`

Rust中的数组是有内在的大小信息的，比如`[u8; 10]`和`[u8; 20]`的大小就分别是10和20，这个长度是编译时期就知道的，无需跟随指针存储。两个长度不同的数组就是不同的类型，比如`[u8; 10]`和`[u8; 20]`就是不同的类型。所以数组是固定大小类型，可以解决上一小节的问题，也就是栈上只需要1个word即可。

既然是固定长度，那就只能存储小于这个长度的字符串，所以这个方案不完整，只能是是一个性能优化的补充方案。不过Lua中遇到的字符串大部分都很短，至少我的经验如此，所以这个优化还是很有意义的。为此我们需要定义2种字符串类型，一个是固定长度数组，用于优化短字符串，另一个是之前的`Rc<String>`方案，用于存储长字符串。固定长度数组的第一个字节用来表示字符串的实际长度，所以数组可以拆成2部分。我们先假设使用总长度48的数组（1个字节表示长度，47个字节存储字符串内容），则定义如下：

```rust
struct Value {
    FixStr(Rc<(u8, [u8; 47])>), // len<=47
    String(Rc<String>), // len>47
```

### 最终方案

我们依次使用并分析了`String`、`Rc<String>`、`Rc<str>`、`Rc<(u8, [u8; 47])>`和内联`(u8, [u8; 14])`等几种方案。各有优缺点。合理的做法是区分对待长短字符串，用**短字符串优化，用长字符串兜底**。可选的3个方案：

- 为了保证`Value`类型的长度，长字符串只能使用`Rc<String>`。
- 对于短字符串，最后的内联方案完全不用堆上内存，优化效果最好。
- 倒数第2个固定长度数组方案，属于上述两个方案的折中，略显鸡肋。不过缺点也只有一个，就是引入更大的复杂性，字符串需要处理3种类型。下一节通过泛型来屏蔽这3种类型，就解决了这个缺点。

最终方案如下：

```rust
const SHORT_STR_MAX: usize = 14;  // sizeof(Value) - 1(tag) - 1(len)
const MID_STR_MAX: usize = 48 - 1;

struct Value {
    ShortStr(u8, [u8; SHORT_STR_MAX]),
    MidStr(Rc<(u8, [u8; MID_STR_MAX])>),
    LongStr(Rc<Vec<u8>>),
```

### 区分长短字符串后，也带来两个新问题 ：

1. 生成字符串类型`Value`时，要根据字符串长度来选择`ShortStr`、`MidStr`还是`LongStr`。这个选择应该是自动实现的，而不应该由调用者实现，否则一是麻烦二是可能出错。比如现在语法分析的代码中出现多次的 `self.add_const(Value::String(var))` 语句，就需要改进。
2. 字符串，顾名思义是“字符”组成，但`ShortStr`和`MidStr`都是由`u8`组成，区别在哪里？`u8`如何正确表达Unicode？如何处理非法字符？

## UniCode 和 UTF-8 解析问题 :
Unicode对世界上大部分文字进行了统一的编码。其中为了跟ASCII码兼容，对ASCII字符集的编码保持一致。比如英文字母p的ASCII和Unicode编码都是0x70，按照Unicode的方式写作U+0070。中文你的Unicode编码是U+4F60。

Unicode只是对文字编了号，至于计算机怎么存储就是另外一回事。最简单的方式就是按照Unicode编码直接存储。由于Unicode目前已经支持14万多个文字（仍然在持续增加），那至少需要3个字节来表示，所以英文字母p就是00 00 70，中文你就是00 4F 60。这种方式的问题是，对于ASCII部分也需要3个字节表示，（对于英文而言）造成浪费。所以就有其他的编码方式，UTF-8就是其中一种。UTF-8是一种变长编码，比如每个ASCII字符只占用1个字节，比如英文字母p编码仍然是0x70，按照UTF-8的方式写作\x70；而每个中文占3个字节，比如中文你的UTF-8编码是\xE4\xBD\xA0

Lua中关于字符串的介绍：We can specify any byte in a short literal string。也就是说Lua的字符串可以表示任意数据。与其叫字符串，不如说就是一串连续的数据，而并不关心数据的内容。

而Rust字符串String类型的介绍：A UTF-8–encoded, growable string。简单明了。两个特点：UTF-8编码，可增长。Lua的字符串是不可变的，Rust的可增长，但这个区别不是现在要讨论的。现在关注的是前一个特点，即UTF-8编码，也就是说Rust字符串不能存储任意数据。通过Rust的字符串的定义，可以更好的观察到这点：

```rust
pub struct String {
    vec: Vec<u8>,
}
```
可以看到String就是对Vec<u8>类型的封装。正是通过这个封装，保证了vec中的数据是合法的UTF-8编码，而不会混进任意数据。如果允许任意数据，那直接定义别名type String = Vec<u8>;就行了。

综上，Rust的字符串String只是Lua字符串的子集；跟Lua字符串类型相对应的Rust类型不是String，而是可以存储任意数据的Vec<u8>。



四种数据结构的关系如下:

```
           slice
  &[u8] <---------> Vec<u8>
                      ^
                      |封装
           slice      |
  &str  <---------> String
```

- `String`是对`Vec<u8>`的一层封装。可以通过`into_bytes()`返回封装的`Vec<u8>`。
- `&str`是`String`的slice（可以认为是引用？）。
- `&[u8]`是`Vec<u8>`的slice。

所以`String`和`&str`可以分别依赖`Vec<u8>`和`&[u8]`。而且看上去`Vec<u8>`和`&[u8]`之间也可以相互依赖，即只直接实现其中之一到`Value`的转换即可。不过这样会损失性能。分析如下：

- 源类型：`Vec<u8>`是拥有所有权的，而`&[u8]`没有。
- 目的类型：`Value::ShortStr/MidStr`只需要复制字符串内容（分别到Value和Rc内部），无需获取源数据的所有权。而`Value::LongStr`需要获取`Vec`的所有权。

2个源类型，2个目的类型，可得4种转换组合：

```
         | Value::ShortStr/MidStr | Value::LongStr
---------+------------------------+-----------------
 &[u8]   |  1.复制字符串内容        | 2.创建Vec，申请内存
 Vec<u8> |  3.复制字符串内容        | 4.转移所有权
```

如果我们直接实现`Vec<u8>`，而对于`&[8]`就先通过`.to_vec()`创建`Vec<u8>`再间接转换为`Value`。那么对于上述第1种情况，本来只需要复制字符串内容即可，而通过`.to_vec()`创建的Vec就浪费了。

如果我们直接实现`&[8]`，而对于`Vec<u8>`就先通过引用来转换为`&[u8]`再间接转换为`Value`。那么对于上述的第4种情况，就要先取引用转换为`&u[8]`，然后再通过`.to_vec()`创建Vec来获得所有权。多了一次无谓的创建。

所以为了效率，还是直接实现`Vec<u8>`和`&[u8]`到`Value`的转换。不过，也许编译器会优化这些的，上述考虑都是瞎操心。但是，这可以帮助我们更深刻理解`Vec<u8>`和`&[u8]`这两个类型，和Rust所有权的概念。最终转换代码如下：

```rust
// convert &[u8], Vec<u8>, &str and String into Value
impl From<&[u8]> for Value {
    fn from(v: &[u8]) -> Self {
        vec_to_short_mid_str(v).unwrap_or(Value::LongStr(Rc::new(v.to_vec())))
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        s.as_bytes().into() // &[u8]
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        vec_to_short_mid_str(&v).unwrap_or(Value::LongStr(Rc::new(v)))
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        s.into_bytes().into() // Vec<u8>
    }
}

fn vec_to_short_mid_str(v: &[u8]) -> Option<Value> {
    let len = v.len();
    if len <= SHORT_STR_MAX {
        let mut buf = [0; SHORT_STR_MAX];
        buf[..len].copy_from_slice(&v);
        Some(Value::ShortStr(len as u8, buf))

    } else if len <= MID_STR_MAX {
        let mut buf = [0; MID_STR_MAX];
        buf[..len].copy_from_slice(&v);
        Some(Value::MidStr(Rc::new((len as u8, buf))))

    } else {
        None
    }
}
```

## GC  
如何使用Rust构建GC系统,是一个很有意思的地方

Lua语言是一门自动管理内存的语言，通过垃圾回收来自动释放不在使用的内存。垃圾回收主要有两种实现途径：标记-清除（mark-and-sweep）和引用计数（reference counting，即RC）。有时候RC并不被认为是GC，所以狭义的GC特指前者，即标记-清除的方案。

相比而言，RC有两个缺点：

无法判断循环引用，进而导致内存泄漏。这点是很致命的。其实Rust中的Rc也有这个问题。Rust对此的策略是：由程序员来避免循环引用。

性能相比GC较差。这点倒不是绝对的，但貌似是主流观点。主要原因在于每次clone或drop操作，都需要更新引用计数器，进而影响CPU缓存。

基于以上原因，主流语言都不会采用RC方案，而是采用GC方案，包括Lua的官方实现版本。但是我们在本章字符串的定义中仍然选择了用Rc，也就是采用RC方案，这是因为GC的两个缺点：

实现复杂。虽然实现一个简单的GC方案可能比较简单，但是如果要追求性能就非常难。很多语言（比如Python、Go、Lua）的GC也都是在多个版本持续改进。很难一步到位。

用Rust实现更复杂。本来Rust语言的最大特色就是自动内存管理。而GC方案这种手动内存管理的功能就跟Rust的这一特性相违背，会使得更加复杂。网络上有很多关于用Rust实现GC的讨论和项目（比如1、2、3、4等），明显已经超出Rust初学者的能力范围。

所以简单来说使用rust写好的RC方案(虽然 这是最初级的垃圾收集算法)来替代GC方案,是一个很好的选择

# 实现表

Lua的表，对外表现为统一的散列表，其索引可以是数字、字符串、或者除了Nil和Nan以外的其他所有Value类型。但为了性能考虑，对于数字类型又有特殊的处理，即使用数组来存储连续数字索引的项。所以在实现里，表其实是由两部分组成：数组和散列表。为此我们定义表：

```rust
pub struct Table {
    pub array: Vec<Value>,
    pub map: HashMap<Value, Value>,
}
```

后续为了支持元表的特性，还会增加其他字段，这里暂且忽略。

Lua语言中的表（以及以后介绍的线程、UserData等）类型并不代表对象数据本身，而[只是对象数据的引用](https://www.lua.org/manual/5.4/manual.html#2.1)，所有对表类型的操作都是操作的引用。比如表的赋值，只是拷贝了表的引用，而非“深拷贝”整个表的数据。所以在`Value`中定义的表类型就不能是`Table`，而必须是引用或者指针。在上一章定义[字符串类型](https://wubingzheng.github.io/build-lua-in-rust/zh/ch03-01.string_type.html)时，引入了`Rc`并讨论了引用和指针。基于同样的原因，这次也采用指针`Rc`对`Table`进行封装。除此之外，这里还需要引入`RefCell`以提供[内部可变性](https://kaisery.github.io/trpl-zh-cn/ch15-05-interior-mutability.html)。综上，表类型定义如下：

```rust
pub enum Value {
    Table(Rc<RefCell<Table>>),
}
```

## 表的构造形式

表的构造支持3种类型：列表式、记录式、和通用式。分别见如下示例代码：

```lua
local key = "kkk"
print { 100, 200, 300;  -- list style
        x="hello", y="world";  -- record style
        [key]="vvv";  -- general style
}
```

先来看下Lua官方实现中是如何处理表的构造的。luac的输出如下：

```shell
$ luac -l test_lua/table.lua

main <test_lua/table.lua:0,0> (14 instructions at 0x600001820080)
0+ params, 6 slots, 1 upvalue, 1 local, 7 constants, 0 functions
    1	[1]	VARARGPREP	0
    2	[1]	LOADK    	0 0	; "kkk"
    3	[2]	GETTABUP 	1 0 1	; _ENV "print"
    4	[2]	NEWTABLE 	2 3 3	; 3
    5	[2]	EXTRAARG 	0
    6	[2]	LOADI    	3 100
    7	[2]	LOADI    	4 200
    8	[2]	LOADI    	5 300
    9	[3]	SETFIELD 	2 2 3k	; "x" "hello"
    10	[3]	SETFIELD 	2 4 5k	; "y" "world"
    11	[4]	SETTABLE 	2 0 6k	; "vvv"
    12	[5]	SETLIST  	2 3 0
    13	[2]	CALL     	1 2 1	; 1 in 0 out
    14	[5]	RETURN   	1 1 1	; 0 out
```

跟表的构造相关的字节码是第4到第12行：

- 第4行，NEWTABLE，用以创建一个表。一共3个参数，分别是新表在栈上位置，数组部分长度，和散列表部分长度。
- 第5行，看不懂，暂时忽略。
- 第6，7，8行，三个LOADI，分别加载数组部分的值100,200,300到栈上，供后面使用。
- 第9，10行，字节码SETFIELD，分别向散列表部分插入x和y。
- 第11行，字节码SETTABLE，向散列表部分插入key。
- 第12行，SETLIST，把上述第6-8行加载到栈上的数据，一次性插入到数组中。

每个字节码的执行对应的栈情况如下：

```
           |       |        /<--- 9.SETFILED
           +-------+        |<---10.SETFILED
4.NEWTABLE |  { }  |<----+--+<---11.SETTABLE
           +-------+     |
   6.LOADI |  100  |---->|
           +-------+     |12.SETLIST
   7.LOADI |  200  |---->|
           +-------+     |
   8.LOADI |  300  |---->/
           +-------+
           |       |
```

首先可以看到，表的构造是在虚拟机执行过程中，通过插入逐个成员，实时构造出来的。

回到表的构造，对于数组部分和散列表部分的处理方式是不同的：

- 数组部分，是先把值依次加载到栈上，最后一次性插入到数组中；
- 散列表部分，是每次直接插入到散列表中。

一个是批量的一个是逐次的。采用不同方式的原因猜测如下：

- 数组部分如果也逐一插入，那么插入某些类型的表达式就需要2条字节码。比如对于全局变量，就需要先用`GetGlobal`字节码加载到栈上，然后再用一个类似`AppendTable`的字节码插入到数组中，那么插入N个值最多就需要2N条字节码。如果批量插入，N个值就只需要N+1条字节码。所以批量插入更适合数组部分。
- 而对于散列表部分，每条数据有key和value两个值，如果也采用批量的方式，把两个值都加载到栈上就需要2条字节码。而如果是逐个插入，很多情况下只需要1条字节码即可。比如上述示例代码中的后面3项都只分别对应1条字节码。这么一来，批量的方式反而需要更多字节码了，所以逐个插入更适合散列表部分。

这一节按照Lua官方实现方法，对应增加下面等4个字节码：

```rust
pub enum ByteCode {
    NewTable(u8, u8, u8),
    SetTable(u8, u8, u8),  // key在栈上
    SetField(u8, u8, u8),  // key是字符串常量
    SetList(u8, u8),
```

不过中间的两个字节码并不支持值是常量的情况，只支持栈上索引。我们在后面小节会加入对常量的优化。


## 基于栈或基于寄存器的VM

通过栈顶来操作参数的方式，称为**基于栈**的虚拟机。很多脚本语言如Java、Python等的虚拟机都是基于栈的。而在字节码中直接索引参数的方式（比如`SetTable 2 0 1`），称为**基于寄存器**的虚拟机。这里的“寄存器”并不是计算机CPU中的寄存器，而是一个虚拟的概念，比如在我们的Lua解释器中，就是用栈和常量表来实现的寄存器。Lua是第一个（官方的虚拟机）基于寄存器的主流语言。



基于寄存器的虚拟机需要一种新类型来保存中间结果。为此我们引入`ExpDesc`（名字来自Lua官方实现代码）：

```rust,ignore
#[derive(Debug, PartialEq)]
enum ExpDesc {
    Nil,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(Vec<u8>),
    Local(usize), // on stack, including local and temprary variables
    Global(usize), // global variable
}
```

现在看上去其类型，就是表达式目前支持的类型，只是把`Token::Name`拆成了`Local`和`Global`，为此引入这个类型有点小题大做。但在下一节支持表的读写时，以及后续的运算表达式、条件跳转等语句时，ExpDesc就会大显身手！

原来的解析过程是从Token直接生成字节码：

```
    Token::Integer  ->  ByteCode::LoadInt
    Token::String   ->  ByteCode::LoadConst
    Token::Name     ->  ByteCode::Move | ByteCode::GetGlobal
    ...
```

现在中间增加了ExpDesc这层，解析过程就变成：

```
    Token::Integer  ->  ExpDesc::Integer  ->  ByteCode::LoadInt
    Token::String   ->  ExpDesc::String   ->  ByteCode::LoadConst
    Token::Name     ->  ExpDesc::Local    ->  ByteCode::Move
    Token::Name     ->  ExpDesc::Global   ->  ByteCode::GetGlobal
    ...
```

## ExpDesc

ExpDesc是非常重要的，这里换个角度再介绍一次。

[第1.1节](./ch01-01.principles.md)基础的编译原理中介绍了通用的编译流程：

```
       词法分析           语法分析         语义分析
字符流 --------> Token流 --------> 语法树 --------> 中间代码 ...
```

我们仍然用上面的加法代码来举例：

```lua
local r
local a = 1
local b = 2
r = a + b
```

按照上述通用编译流程，对于最后一行的加法语句，语法分析会得到语法树：

```
    |
    V
    +
   / \
  a   b
```

然后在语义分析时，先看到`+`，得知这是一条加法的语句，于是可以很直接地生成字节码：`Add ? 1 2`。其中`?`是加法的目标地址，由赋值语句处理，这里忽略；`1`和`2`分别是两个加数的栈索引。

但我们目前的做法，也是Lua官方实现的做法，是省略了“语义分析”这一步，从语法分析直接生成中间代码，边分析边生成代码。那么在语法分析时，就不能像上述语义分析那样有全局的视角。比如对于加法语句`a+b`，在读到`a`时，还不知道这是一条加法语句，只能先存起来。读到`+`时才确定是加法语句，然后再读第二个加数，然后生成字节码。我们为此引入了`ExpDesc`这个中间层。所以ExpDesc就相当于是通用流程中的“语法树”的作用。只不过语法树是全局的，而ExpDesc是局部的，而且是最小粒度的局部。

```
       词法分析               语法分析
字符流 --------> Token流 ----(ExpDesc)---> 中间代码 ...
```

可以直观地看到，Lua的这种方式省去了语义分析步骤，速度应该略快，但由于没有全局视角，所以实现相对复杂。这两种方式更详细的说明和对比已经超出了本文的讨论范围。我们选择按照Lua官方实现的方式，选择语法分析直接生成字节码的方式。

#### 简单来说ExpDesc就是一个局部的AST