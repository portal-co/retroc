#![no_std]

use core::fmt::Display;

use alloc::{collections::btree_map::BTreeMap, string::String, vec::Vec};
use nom::error::ParseError;
extern crate alloc;
pub trait ByteMetaParser<T, ParseErrType> {
    fn from_bytes_and_meta<'a, 'b>(
        &self,
        bytes: &'a [u8],
        meta: &'b str,
    ) -> Result<(&'a [u8], &'b str, T), nom::Err<ParseErrType>>;
}
pub trait FileParser<T, ParseErrType> {
    fn from_bytes_and_meta<'a, 'b>(
        &self,
        bytes: &'a [u8],
        meta: &'b str,
        registry: &FileRegistry<'_, T, ParseErrType>,
    ) -> Result<(&'a [u8], &'b str, T), nom::Err<ParseErrType>>;
}
pub struct FileRegistry<'a, T, Err> {
    pub parsers: BTreeMap<String, &'a (dyn FileParser<T, Err> + 'a)>,
}
impl<'a, T, Err> Default for FileRegistry<'a, T, Err> {
    fn default() -> Self {
        Self {
            parsers: BTreeMap::new(),
        }
    }
}
impl<'a, T, Err: From<ErrorKind>> FileRegistry<'a, T, Err> {
    pub fn register_parser(
        &mut self,
        extension: String,
        parser: &'a (dyn FileParser<T, Err> + 'a),
    ) {
        self.parsers.insert(extension, parser);
    }
    pub fn parse_file<'a2, 'b>(
        &self,
        mut bytes: &'a [u8],
        mut meta: &'b str,
    ) -> Result<(&'a [u8], &'b str, Vec<T>), nom::Err<Err>> {
        let mut all = Vec::default();
        loop {
            meta = match meta.strip_prefix("arch ") {
                Some(rest) => rest,
                None => return Ok((bytes, meta, all)),
            };
            let x;
            (meta, x) = match meta.split_once('\n') {
                Some((a, b)) => (a, b),
                None => return Err(nom::Err::Error(Err::from(ErrorKind::ArchIncomplete))),
            };
            let Some(x) = self.parsers.get(x).cloned() else {
                return Err(nom::Err::Error(Err::from(ErrorKind::NoParser)));
            };
            let t;
            (bytes, meta, t) = x.from_bytes_and_meta(bytes, meta, self)?;
            all.push(t);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    ArchIncomplete,
    NoParser,
    Nom(nom::error::ErrorKind),
}
impl Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ErrorKind::ArchIncomplete => write!(f, "Architecture information incomplete"),
            ErrorKind::NoParser => write!(f, "No parser found for given architecture"),
            ErrorKind::Nom(error_kind) => write!(f, "Nom error: {}", error_kind.description()),
        }
    }
}
impl<I> ParseError<I> for ErrorKind {
    fn from_error_kind(_input: I, kind: nom::error::ErrorKind) -> Self {
        ErrorKind::Nom(kind)
    }
    fn append(_input: I, kind: nom::error::ErrorKind, _other: Self) -> Self {
        ErrorKind::Nom(kind)
    }
}
