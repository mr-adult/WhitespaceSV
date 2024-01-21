use std::fs::File;

fn from_bytes<I: Iterator<Item = u8>>(bytes: I) -> ReliableTxtParser<I> {
    todo!()
}

struct ReliableTxtParser<I>
    where I: Iterator<Item = u8> {
    bytes: I
}