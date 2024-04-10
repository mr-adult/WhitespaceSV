# WhitespaceSV

## Overview

Whitespace Separated Value (WSV) is a file format that has been designed to address to problems with the Comma Separated Value (CSV) file format. It can be parsed unambiguously and does not require any configuration for a parser to parse it.

This crate provides a rust-based implementation of the [WSV standard](https://dev.stenway.com/WSV/Index.html). This implementation aims to be as efficient as possible. In eager (non-lazy or standard) parsing, this parser is as close to zero copy as possible. It will only allocate strings in cases where escape sequences must be replaced. There are only a handful of APIs exposed in the crate, but they should be able to handle all of your use cases.


## Patch Notes

### 1.0.2
Fixes [a panic when writing jagged arrays](https://github.com/mr-adult/WhitespaceSV/issues/1)


## Parsing

In order to parse a WSV file using this crate, simply call one of the provided parsing functions. There are currently 3, so pick the one that makes sense for your use case. Most use cases should probably use the standard parse() function.
1. [parse_with_col_count](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse_with_col_count.html) - use this API if it is safe to parse your WSV eagerly (it fits in memory) and your WSV is a standard table with a known number of columns. This will avoid unnecessary reallocations of the Vecs involved in parsing.
2. [parse_lazy](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse_lazy.html) - use this API if you have a large input that should only be loaded in pieces (presumably because it doesn't fit in memory). This API will lazily parse the input line-by-line. If you need to parse at a value-by-value level, use [WSVLazyTokenizer](https://docs.rs/whitespacesv/latest/whitespacesv/struct.WSVLazyTokenizer.html) directly for full control.
3. [parse](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse.html) - use this for all other cases.

### Eager Parsing

There's not a lot to say here. The eager parsing APIs work about how you would expect. They return a Result where if parsing succeeded, the value is the Ok variant with a 2D Vec of Option<Cow<'_, str>>. The values in the Cow have handled the following cases:
1. `"/"` escape sequences have been replaced with `\n`
2. `""` escape sequences have been replaced with `"`
3. Any wrapping quotes will be removed. Ex. `"hello, world!"` would become `hello, world`
4. `-` values will be returned as None variants of the Option enum. Everything else will be a Some variant.

As an example, the input below would return [[Some(1), None], [Some(3), Some(String(This is a string " with escape sequences \n))]] (quotes around the string value and `\` escape sequences have been removed for clarity)
```whitespacesv
1 -
3 "This is a string "" with escape sequences "/""
```

### Lazy Parsing

On top of standard parsing, this crate supports lazy parsing via iterators. By creating an iterator pipeline, you can process files that do not fit into memory. As an example, let's say I have a 300 gigabyte file where what I'd really like is the sum of each line of that file. I could set up an iterator pipeline to read the WSV and output the sums back into WSV with the code that follows.

Note that the example code is still eagerly evaluating each line of the WSV. If you need finer-grain lazy parsing, use this crate's [WSVLazyTokenizer](https://docs.rs/whitespacesv/latest/whitespacesv/struct.WSVLazyTokenizer.html) directly to accomplish whatever you need.

The lazy parse API and WSVLazyTokenizer accept an Iterator of `char`s, so some useful resources to obtain this include the following:
- [the utf8-chars crate](https://crates.io/crates/utf8-chars)
- [from_utf16 in the standard library](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf16) (nightly)
- [from_utf16le in the standard library](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf16le) (nightly)
- [from_utf16be in the standard library](https://doc.rust-lang.org/std/string/struct.String.html#method.from_utf16be) (nightly)
- [decode_utf32 from the widestring crate](https://docs.rs/widestring/latest/widestring/fn.decode_utf32.html) for utf-32

```rust
use std::fs::File;
use std::io::BufReader;
// I recommend you pull in the utf8-chars crate as a 
// dependency if you need lazy parsing of utf-8
use whitespacesv::{parse_lazy, WSVWriter};
use utf8_chars::BufReadCharsExt;

let mut reader = 
    BufReader::new(
        File::open("./my_very_large_file.txt")
            .unwrap());

let chars = reader.chars().map(|ch| ch.unwrap());

let lines_lazy = parse_lazy(chars).map(|line| {
    // For this example we will assume we have valid WSV
    let sum = line
        .unwrap()
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
    print!("{}", ch);
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
- `Vec<Vec<Option<&'_ str>>>`
- `VecDeque<Vec<Option<&String>>>`
- `Iter<Iter<Option<String>>>` where Iter is any type that implements Iterator.

```rust
use whitespacesv::{WSVWriter, ColumnAlignment};

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

This implementation of the WSVWriter allows you to write incredibly large files by taking advantage of the lazy evaluation of iterators. By passing iterators into the WSVWriter and using the Iterator implementation that WSVWriter provides, you can write as big of files as you can fit on disk space. As an example, let's say I need to print 4,294,967,295 (u32::MAX) rows of the sequence 0 through 9 to my terminal in the WSV format. I can accomplish this by using the code as follows:

Note: This API will also only surround strings with quotes if necessary.

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