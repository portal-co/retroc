#![no_std]

use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    vec::{self, Vec},
};
extern crate alloc;
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Reg {
    A,
    X,
    Y,
}
pub mod block;