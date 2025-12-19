#![feature(decl_macro)]
#![allow(dead_code)]

use std::sync::Arc;
use tokio::sync::RwLock;


#[derive(Clone)]
pub enum Candidate<T> {
    Value(T),
    Arc(Arc<RwLock<T>>)
}

#[derive(Clone)]
pub enum BincodeConfiguration {
    Standard,
    Legacy,
}

pub macro future($coroutine: expr) {
    futures::executor::block_on($coroutine)
}

pub macro drop($($x:expr),* $(,)?) {
    $( std::mem::drop($x); )*
}


#[cfg(feature = "sequence")]
pub mod sequence;