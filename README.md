# WhitespaceSV

## Overview

Whitespace Separated Value (WSV) is a file format that has been designed to address to problems with the Comma Separated Value (CSV) file format. It can be parsed unambiguously and does not require any configuration for a parser to parse it.

This crate provides a rust-based implementation of the [WSV standard](https://dev.stenway.com/WSV/Index.html). This implementation is as close to zero-copy as possible, only allocating memory in cases where escape characters must be replaced. There are only a handful of APIs exposed in the crate, but they should be able to handle all of your use cases.


## Parsing

In order to parse a WSV file using this crate, simply call one of the provided parsing functions. There are currently 2, so pick the one that makes sense for your use case.
1. [parse_with_col_count](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse_with_col_count.html) - use this API if your WSV is a standard table with a known number of columns. This will avoid unnecessary reallocations of the Vecs involved in parsing.
2. [parse](https://docs.rs/whitespacesv/latest/whitespacesv/fn.parse.html) - use this for all other cases.


## Writing

There is only one API provided to write a WSV file. It is done as follows. This API will surround _all_ strings with quotes to avoid unnecessary scans of the content, but this is not necessary under the standard. Currently, it only supports writing &str values, but support for Cow<'_, str> and String values are on the roadmap for this crate as well.

```rust
use whitespacesv::{WSVWriter, ColumnAlignment};
// Build up the testing value set. This API accepts any
// type that implements IntoIterator, so LinkedList,
// VecDeque and many others are accepted as well.
// In this example, we're using mapped iterators.
let values = vec![
    vec!["1", "2", "3"],
    vec!["4", "5", "6"],
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
    .align_columns(ColumnAlignment::Packed)
    .to_string();

/// Output:
/// "1" "2" "3"
/// "4" "5" "6"
/// "My string with a "/" character"
/// "My string with many """""" characters"
println!("{}", wsv);
```