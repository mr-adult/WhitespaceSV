#![doc = include_str!("../README.md")]

use std::borrow::Cow;
use std::mem::take;
use std::str::CharIndices;

const NEWLINE: char = '\u{000A}';

/// Parses the contents of a .wsv (whitespace separated value) file.
/// The result is either a 2 dimensional vec where the outer layer is
/// the line and the inner layer is the column. '-' values will be
/// converted to 'None' and other values will be 'Some'
///
/// For example, given the wsv file:
/// ```wsv
/// 1 -
/// 3 4
/// ```
/// the returned value would be [[Some(1), None], [Some(3), Some(4)]]
///
/// The source text will be sanitized. That is to say:
/// 1. All `"/"` escape characters within strings will be replaced with
/// \n inside the string.
/// 2. All "" (two double-quote character) escape sequences within strings
/// will be replaced with " (one double-quote character)
/// 3. Any wrapping quotes around a string will be removed. Ex. "hello"
/// will just be hello in the output.
pub fn parse(source_text: &str) -> Result<Vec<Vec<Option<Cow<'_, str>>>>, WSVError> {
    // Just use the vec default size of 0.
    parse_with_col_count(source_text, 0)
}

/// Same as parse (see the documentation there for behavior details),
/// but accepts an expected column count to avoid unnecessary reallocations
/// of the Vecs.
pub fn parse_with_col_count(
    source_text: &str,
    col_count: usize,
) -> Result<Vec<Vec<Option<Cow<'_, str>>>>, WSVError> {
    let mut result = Vec::new();
    result.push(Vec::with_capacity(col_count));
    let mut last_line_num = 0;

    for fallible_token in WSVTokenizer::new(source_text) {
        let token = fallible_token?;
        match token {
            WSVToken::LF => {
                result.push(Vec::with_capacity(col_count));
                last_line_num += 1;
            }
            WSVToken::Null => {
                result[last_line_num].push(None);
            }
            WSVToken::Value(value) => {
                result[last_line_num].push(Some(value));
            }
            WSVToken::Comment(_) => {}
        }
    }

    // We pushed extra vecs on eagerly every time we saw an
    // LF, so pop the last one if it was empty.
    if result[last_line_num].len() == 0 {
        result.pop();
    }

    Ok(result)
}

/// A struct for writing values to a .wsv file.
pub struct WSVWriter<'values, OuterIter, InnerIter>
where
    OuterIter: IntoIterator<Item = InnerIter>,
    InnerIter: IntoIterator<Item = Option<&'values str>>,
{
    align_columns: ColumnAlignment,
    values: OuterIter,
}

impl<'values, OuterIter, InnerIter> WSVWriter<'values, OuterIter, InnerIter>
where
    OuterIter: IntoIterator<Item = InnerIter>,
    InnerIter: IntoIterator<Item = Option<&'values str>>,
{
    pub fn new(values: OuterIter) -> Self {
        Self {
            align_columns: ColumnAlignment::default(),
            values: values,
        }
    }

    pub fn align_columns(mut self, alignment: ColumnAlignment) -> Self {
        self.align_columns = alignment;
        self
    }

    pub fn to_string(self) -> String {
        match self.align_columns {
            ColumnAlignment::Left | ColumnAlignment::Right => {
                let vecs = self
                    .values
                    .into_iter()
                    .map(|inner| inner.into_iter().collect::<Vec<Option<&'values str>>>())
                    .collect::<Vec<Vec<Option<&'values str>>>>();

                let mut max_col_widths = Vec::new();
                for line in vecs.iter() {
                    for (i, col) in line.iter().enumerate() {
                        let mut col_len = 0;
                        for _ in col.unwrap_or("-").chars() {
                            col_len += 1;
                        }

                        match max_col_widths.get_mut(i) {
                            None => max_col_widths.push(col_len),
                            Some(max) => {
                                if *max < col_len {
                                    *max = col_len
                                }
                            }
                        }
                    }
                }

                Self::to_string_inner(vecs, self.align_columns, Some(max_col_widths))
            }
            ColumnAlignment::Packed => {
                Self::to_string_inner(self.values, ColumnAlignment::Packed, None)
            }
        }
    }

    fn to_string_inner<
        Outer: IntoIterator<Item = Inner>,
        Inner: IntoIterator<Item = Option<&'values str>>,
    >(
        iters: Outer,
        alignment: ColumnAlignment,
        max_col_widths: Option<Vec<usize>>,
    ) -> String {
        let mut result = String::new();
        for line in iters {
            for (i, col) in line.into_iter().enumerate() {
                if i != 0 {
                    result.push_str(" ");
                }

                let value = col.unwrap_or("-");
                let str_to_push = match alignment {
                    ColumnAlignment::Packed => Cow::Borrowed(value),
                    ColumnAlignment::Left => {
                        let mut value_string = value.to_string();
                        for _ in value.len()..max_col_widths.as_ref().unwrap()[i] {
                            value_string.push(' ');
                        }
                        Cow::Owned(value_string)
                    }
                    ColumnAlignment::Right => {
                        let mut value_string = "".to_string();
                        for _ in value.len()..=max_col_widths.as_ref().unwrap()[i] {
                            value_string.push(' ');
                        }
                        value_string.push_str(col.unwrap_or("-"));
                        Cow::Owned(value_string)
                    }
                };

                result.push('"');
                for ch in str_to_push.chars() {
                    if ch == '\n' {
                        result.push('"');
                        result.push('/');
                        result.push('"');
                    } else if ch == '"' {
                        result.push('"');
                        result.push('"');
                    } else {
                        result.push(ch);
                    }
                }
                result.push('"');
            }
            result.push('\n')
        }

        result
    }
}

#[derive(Default)]
pub enum ColumnAlignment {
    Left,
    Right,
    #[default]
    Packed,
}

pub struct WSVTokenizer<'wsv> {
    source: &'wsv str,
    chars: CharIndices<'wsv>,
    peeked: Option<(usize, char)>,
    current_location: Location,
}

/// A tokenizer for the .wsv (whitespace separated value)
/// file format. This struct implements Iterator, so to
/// extract the tokens use your desired iterator method
/// or a standard for loop.
impl<'wsv> WSVTokenizer<'wsv> {
    /// Creates a .wsv tokenizer from .wsv source text.
    pub fn new(source_text: &'wsv str) -> Self {
        Self {
            source: source_text,
            chars: source_text.char_indices(),
            peeked: None,
            current_location: Location::default(),
        }
    }

    fn match_string(&mut self) -> Option<Result<WSVToken<'wsv>, WSVError>> {
        if self.match_char('"').is_none() {
            return None;
        }
        let mut chunks = Vec::with_capacity(1);
        let mut chunk_start = None;
        loop {
            if self.match_char('"').is_some() {
                let mut end_location = match self.peek_location() {
                    None => self.source.len(),
                    Some(pos) => pos.byte_index,
                };

                if self.match_char('"').is_some() {
                    // a quote is ascii, so subtracting 2 bytes should always be safe.
                    end_location -= 2;
                    chunks.push(&self.source[chunk_start.unwrap_or(end_location)..end_location]);
                    chunks.push("\"");
                } else if self.match_char('/').is_some() {
                    if self.match_char('"').is_none() {
                        return Some(Err(WSVError {
                            err_type: WSVErrorType::InvalidStringLineBreak,
                            location: self.current_location.clone(),
                        }));
                    }
                    chunks.push("\n")
                } else {
                    // a quote is ascii, so subtracting 1 bytes should always be safe.
                    end_location -= 1;
                    chunks.push(&self.source[chunk_start.unwrap_or(end_location)..end_location]);
                    break;
                }
            } else if let Some(NEWLINE) = self.peek() {
                if let Some(NEWLINE) = self.peek() {
                    return Some(Err(WSVError {
                        err_type: WSVErrorType::StringNotClosed,
                        location: self
                            .peek_location()
                            .expect("BUG: peek_location() return Some()"),
                    }));
                }
            } else if let None = chunk_start {
                chunk_start = Some(match self.peek_location() {
                    None => self.source.len(),
                    Some(val) => val.byte_index,
                });
            } else {
                self.match_char_if(&mut |_| true);
            }
        }

        if chunks.len() == 1 {
            return Some(Ok(WSVToken::Value(Cow::Borrowed(chunks[0]))));
        } else {
            return Some(Ok(WSVToken::Value(Cow::Owned(
                chunks.into_iter().collect::<String>(),
            ))));
        }
    }

    fn match_char_while<F: FnMut(char) -> bool>(&mut self, mut predicate: F) -> Option<&'wsv str> {
        let mut start = None;
        loop {
            match self.match_char_if(&mut predicate) {
                None => break,
                Some((index, _)) => {
                    if let None = start {
                        start = Some(index);
                    }
                }
            }
        }

        let start_val = match start {
            None => return None,
            Some(val) => val,
        };

        // Just get the side effect of setting peeked
        self.peek();
        let end_val = match self.peeked.as_ref() {
            None => self.source.len(),
            Some((index, _)) => *index,
        };

        return Some(&self.source[start_val..end_val]);
    }

    fn match_char(&mut self, ch: char) -> Option<(usize, char)> {
        self.match_char_if(&mut |found_char| ch == found_char)
    }

    fn match_char_if<F: FnMut(char) -> bool>(
        &mut self,
        predicate: &mut F,
    ) -> Option<(usize, char)> {
        if let Some(found_char) = self.peek() {
            if predicate(found_char) {
                let consumed = take(&mut self.peeked);

                match consumed {
                    None => {
                        return None;
                    }
                    Some((i, ch)) => {
                        if ch == NEWLINE {
                            self.current_location.line += 1;
                            self.current_location.col = 0;
                        } else {
                            self.current_location.col += 1;
                        }
                        self.current_location.byte_index = i;
                    }
                }

                return consumed.clone();
            }
        }

        return None;
    }

    fn peek_location(&mut self) -> Option<Location> {
        self.peek_inner();
        match self.peeked.as_ref() {
            None => None,
            Some((i, _)) => {
                let mut peeked_pos = self.current_location.clone();
                peeked_pos.col += 1;
                peeked_pos.byte_index = *i;
                Some(peeked_pos)
            }
        }
    }

    fn peek(&mut self) -> Option<char> {
        match self.peek_inner() {
            None => None,
            Some(peeked) => Some(peeked.1),
        }
    }

    fn peek_inner(&mut self) -> Option<&(usize, char)> {
        if let None = self.peeked.as_ref() {
            self.peeked = self.chars.next();
        }
        self.peeked.as_ref()
    }

    fn is_whitespace(ch: char) -> bool {
        match ch {
            '\u{0009}' | '\u{000B}' | '\u{000C}' | '\u{000D}' | '\u{0020}' | '\u{0085}'
            | '\u{00A0}' | '\u{1680}' | '\u{2000}' | '\u{2001}' | '\u{2002}' | '\u{2003}'
            | '\u{2004}' | '\u{2005}' | '\u{2006}' | '\u{2007}' | '\u{2008}' | '\u{2009}'
            | '\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => true,
            _ => false,
        }
    }
}

impl<'wsv> Iterator for WSVTokenizer<'wsv> {
    type Item = Result<WSVToken<'wsv>, WSVError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_location.col != 0 {
            if let Some(ch) = self.peek() {
                if ch == '"' {
                    return Some(Err(WSVError {
                        err_type: WSVErrorType::InvalidDoubleQuoteAfterValue,
                        location: self
                            .peek_location()
                            .expect("BUG: peek_location() return Some()"),
                    }));
                } else if ch != NEWLINE && !Self::is_whitespace(ch) {
                    return Some(Err(WSVError {
                        err_type: WSVErrorType::InvalidCharacterAfterString,
                        location: self
                            .peek_location()
                            .expect("BUG: peek_location() return Some()"),
                    }));
                }
            }
        }
        self.match_char_while(|ch| Self::is_whitespace(ch));

        let str = self.match_string();
        if str.is_some() {
            return str;
        } else if self.match_char('#').is_some() {
            // Comment
            return Some(Ok(WSVToken::Comment(
                self.match_char_while(|ch| ch != NEWLINE).unwrap_or(""),
            )));
        } else if self.match_char(NEWLINE).is_some() {
            return Some(Ok(WSVToken::LF));
        } else {
            // Value
            match self.match_char_while(|ch| {
                if ch == NEWLINE {
                    return false;
                }
                if ch == '"' {
                    return false;
                }
                if ch == '#' {
                    return false;
                }
                if Self::is_whitespace(ch) {
                    return false;
                }
                return true;
            }) {
                Some(str) => {
                    if str == "-" {
                        return Some(Ok(WSVToken::Null));
                    }
                    return Some(Ok(WSVToken::Value(Cow::Borrowed(str))));
                }
                None => None,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum WSVToken<'wsv> {
    LF,
    Null,
    Value(Cow<'wsv, str>),
    Comment(&'wsv str),
}

#[derive(Debug, Clone)]
pub struct WSVError {
    err_type: WSVErrorType,
    location: Location,
}

impl WSVError {
    pub fn err_type(&self) -> WSVErrorType {
        self.err_type
    }

    pub fn location(&self) -> Location {
        self.location.clone()
    }
}

/// For details on these error types, see the Parser Errors
/// section of [https://dev.stenway.com/WSV/Specification.html](https://dev.stenway.com/WSV/Specification.html)
#[derive(Clone, Copy, Debug)]
pub enum WSVErrorType {
    StringNotClosed,
    InvalidDoubleQuoteAfterValue,
    InvalidCharacterAfterString,
    InvalidStringLineBreak,
}

#[derive(Debug, Default, Clone)]
pub struct Location {
    byte_index: usize,
    line: usize,
    col: usize,
}

impl Location {
    /// The byte index in the source text string.
    pub fn byte_index(&self) -> usize {
        self.byte_index
    }
    /// The line number in the source text.
    pub fn line(&self) -> usize {
        self.line
    }
    /// The column number in the source text.
    pub fn col(&self) -> usize {
        self.col
    }
}

#[cfg(debug_assertions)]
mod tests {
    use super::{parse, WSVWriter};
    use std::{borrow::Cow, fmt::write};

    #[test]
    fn read_and_write() {
        let str = include_str!("../tests/1_stenway.com");
        let result = parse(str)
            .unwrap()
            .into_iter()
            .map(|vec| {
                vec.into_iter()
                    .map(|cow_opt| match cow_opt {
                        None => None,
                        Some(cow) => match cow {
                            Cow::Borrowed(str) => Some(str.to_string()),
                            Cow::Owned(string) => Some(string),
                        },
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let as_refs = result
            .iter()
            .map(|vec| {
                vec.iter()
                    .map(|str_opt| match str_opt {
                        None => None,
                        Some(string) => Some(string.as_str()),
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let result_str = WSVWriter::new(as_refs)
            .align_columns(super::ColumnAlignment::Packed)
            .to_string();

        println!("{}", result_str);
    }

    #[test]
    fn e2e_test() {
        let str = include_str!("../tests/1_stenway.com");
        let result = parse(str);

        match result {
            Err(_) => panic!("Should not have error"),
            Ok(values) => {
                let expected = vec![
                    vec!["a", "U+0061", "61", "0061", "Latin Small Letter A"],
                    vec!["~", "U+007E", "7E", "007E", "Tilde"],
                    vec!["¥", "U+00A5", "C2_A5", "00A5", "Yen Sign"],
                    vec![
                        "»",
                        "U+00BB",
                        "C2_BB",
                        "00BB",
                        "Right-Pointing Double Angle Quotation Mark",
                    ],
                    vec!["½", "U+00BD", "C2_BD", "00BD", "Vulgar Fraction One Half"],
                    vec!["¿", "U+00BF", "C2_BF", "00BF", "Inverted#Question Mark"],
                    vec!["ß", "U+00DF", "C3_9F", "00DF", "Latin Small Letter Sharp S"],
                    vec![
                        "ä",
                        "U+00E4",
                        "C3_A4",
                        "00E4",
                        "Latin Small Letter A with Diaeresis",
                    ],
                    vec![
                        "ï",
                        "U+00EF",
                        "C3_AF",
                        "00EF",
                        "Latin Small Letter I with Diaeresis",
                    ],
                    vec!["œ", "U+0153", "C5_93", "0153", "Latin Small Ligature Oe"],
                    vec!["€", "U+20AC", "E2_82_AC", "20AC", "Euro Sign"],
                    vec![
                        "東",
                        "U+6771",
                        "E6_9D_B1",
                        "6771",
                        "CJK Unified Ideograph-6771",
                    ],
                    vec![
                        "𝄞",
                        "U+1D11E",
                        "F0_9D_84_9E",
                        "D834_DD1E",
                        "Musical Symbol G Clef",
                    ],
                    vec![
                        "𠀇",
                        "U+20007",
                        "F0_A0_80_87",
                        "D840_DC07",
                        "CJK Unified Ideograph-20007",
                    ],
                    vec![
                        "-",
                        "hyphen",
                        "qwro-qweb",
                        "-dasbe",
                        "A hyphen character - represents null",
                    ],
                ];

                let mut expected_iter = expected.into_iter();
                let mut acutal_iter = values.into_iter();

                loop {
                    let expected_line = expected_iter.next();
                    let actual_line = acutal_iter.next();

                    assert_eq!(
                        expected_line.is_some(),
                        actual_line.is_some(),
                        "Line numbers should match"
                    );
                    if expected_line.is_none() || actual_line.is_none() {
                        break;
                    }

                    let mut expected_value_iter = expected_line.unwrap().into_iter();
                    let mut actual_value_iter = actual_line.unwrap().into_iter();
                    loop {
                        let expected_value = expected_value_iter.next();
                        let actual_value = actual_value_iter.next();

                        assert_eq!(
                            expected_value.is_some(),
                            expected_value.is_some(),
                            "Value counts should match"
                        );
                        if expected_value.is_none() || actual_value.is_none() {
                            break;
                        }

                        if expected_value.unwrap() == "-" {
                            assert_eq!(None, actual_value.unwrap(), "'-' should parse to None");
                        } else {
                            let actual_value = actual_value
                                .expect("Actual value to be populated at this poitn.")
                                .expect(
                                    "actual value should parse to Some() if expected is not '-'",
                                );
                            assert_eq!(
                                expected_value.unwrap().to_owned(),
                                actual_value.to_owned(),
                                "string values should match"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn readme_example() {
        use crate::{WSVWriter, ColumnAlignment};
        // Build up the testing value set. This API accepts any
        // type that implements IntoIterator, so LinkedList,
        // VecDeque and many others are accepted as well.
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
            // The default is packed, but left and right are also options
            // if your .wsv file will be looked at by people
            .align_columns(ColumnAlignment::Packed)
            .to_string();

        println!("{}", wsv);
    }
}
