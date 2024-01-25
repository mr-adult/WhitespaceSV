# WhitespaceSV

## Overview

Whitespace Separated Value (WSV) is a file format that has been designed to address to problems with the Comma Separated Value (CSV) file format. It can be parsed unambiguously and does not require any configuration for a parser to parse it.

This crate provides a rust-based implementation of the [WSV standard](https://dev.stenway.com/WSV/Index.html). This implementation is as close to zero-copy as possible in eager (non-lazy or standard) parsing, only allocating memory in cases where escape sequences must be replaced. There are only a handful of APIs exposed in the crate, but they should be able to handle all of your use cases.


## Parsing

In order to parse a WSV file using this crate, simply call one of the provided parsing functions. There are currently 3, so pick the one that makes sense for your use case. Most use cases should probably use the standard parse() function.
1. [parse_with_col_count](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse_with_col_count.html) - use this API if it is safe to parse your WSV eagerly (it fits in memory) and your WSV is a standard table with a known number of columns. This will avoid unnecessary reallocations of the Vecs involved in parsing.
2. [parse_lazy](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse_lazy.html) - use this API if you have a large input that should only be loaded in pieces (presumably because it doesn't fit in memory). This API will lazily parse the input line-by-line. If you need to parse at a value-by-value level, use WSVLazyTokenizer directly
3. [parse](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse.html) - use this for all other cases.


### Lazy Parsing

This trait supports lazy parsing via iterators. By creating an iterator pipeline, you can process files that do not fit into memory. As an example, let's say I have a 300 gigabyte file where what I'd really like is the sum of each line of that file. I could set up an iterator pipeline to read the WSV and output the sums back into WSV with the code that follows.

Note that the example code is still eagerly evaluating each line of the WSV. If you need finer-grain lazy parsing, use this crate's WSVLazyTokenizer directly to accomplish whatever you need.

```rust
use whitespacesv::{parse_lazy, WSVWriter};

// pretend that this input is some iterator 
// handling buffering the characters in a 300 
// gigabyte file.
let input = String::new();
let chars = input.chars();

let lines_lazy = parse_lazy(chars).map(|line| {
    // You probably want to handle errors in your case
    // unless you are guaranteed to have valid WSV.
    let sum = line.unwrap()
        .into_iter()
        // We're counting None as 0 in my case, 
        // so flat_map the Nones out.
        .flat_map(|opt| opt)
        .map(|value| value.parse::<i32>().unwrap_or(0))
        .sum::<i32>();

    // The writer needs a 2D iterator of Option<String>, 
    // so wrap the value in a Some and .to_string() it.
    // Also wrap in a Vec to make it a 2D iterator
    vec![Some(sum.to_string())]
});
// CAREFUL: Don't call .collect() here or we'll run out of memory!


// The WSVWriter when using ColumnAlignment::Packed 
// (the default) is also lazy, so we can pass our
// result in directly.
for ch in WSVWriter::new(lines_lazy) {
    // Your code to dump the output to a file goes here.
    print!("{}", ch)
}
```


## Writing

There are two ways to use the API provided to write a WSV file. 
1. The WSVWriter to_string() method - this allows you to align your columns to the left or right as you please. Most use cases should use this.
2. The WSVWriter Iterator implementation. This allows you to lazily evaluate values. If you need to write value stores that are too large to fit in memory, use this. This implementation does not respect column alignment and is built for pure speed.

### to_string()

This API will surround strings with quotes _only if necessary_. The values in this 2D IntoIterator structure must be Options where the inner value is a type that implements
1. AsRef<str>, 
2. From<&'static str>, and 
3. ToString. 

The &str, Cow<'_, str>, String, and &String types are all supported with these type constraints.

Some examples of types that are supported via the WSVWriter::new() API:
- `LinkedList<LinkedList<Option<Cow<'_, str>>>>`
- `Vec<Vec<impl Iterator<&'_ str>>>`
- `Iter<Iter<Option<String>>>` where Iter is any type that implements Iterator.

```rust
use whitespacesv::{WSVWriter, ColumnAlignment};
// Build up the testing value set. This API accepts any
// type that implements IntoIterator, so LinkedList,
// VecDeque and many others are accepted as well.
// In this example, we're using mapped iterators.
let values = vec![
    vec!["1", "2", "3"], // In this example, each value is &str,
    vec!["4", "5", "6"], // but String and Cow<'_, str> also work
    vec!["My string with a \n character"],
    vec!["My string with many \"\"\" characters"],
];

let values_as_opts = values
    .into_iter()
    .map(|row| row.into_iter().map(|value| Some(value)));

let wsv = WSVWriter::new(values_as_opts)
    // The default alignment is packed, but left and 
    // right aligned are also options in cases where 
    // your .wsv file will be looked at by people and 
    // not just machines.
    .align_columns(ColumnAlignment::Left)
    .to_string();

/// Output:
/// 1                                       2 3
/// 4                                       5 6
/// "My string with a "/" character"       
/// "My string with many """""" characters"
println!("{}", wsv);
```

### Iterator

This implementation of the WSVWriter allows you to write incredibly large files by taking advantage of the lazy evaluation of iterators. By passing iterators into the WSVWriter and using the Iterator implementation that WSVWriter provides, you can write as big of files as you can fit on disk space. As an example, let's say I need to print 4,294,967,295 rows of the sequence 0 through 9 to my terminal in the WSV format. I can accomplish this by using the code as follows:

Note: This API will surround strings with quotes only if necessary.

```rust
use whitespacesv::WSVWriter;

let values = (0..u32::MAX).map(|_| (0..10).into_iter().map(|val| Some(val.to_string())));
// NOTE: column alignment is not respected when using this iterator implementation.
for ch in WSVWriter::new(values) {
    print!("{}", ch);
    // This is so that my computer doesn't fry when running unit tests.
    break;
}
```