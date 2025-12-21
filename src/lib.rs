#![allow(dead_code)]
#![allow(unused)]

use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(all(feature = "nightly", not(nightly)))]
compile_error!("The `nightly` feature requires a nightly compiler");


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

#[macro_export]
macro_rules! future {
    ($coroutine: expr) => {
        futures::executor::block_on($coroutine)
    };
}

#[macro_export]
macro_rules! drop {
    ($($x:expr),* $(,)?) => {
        $( std::mem::drop($x); )*
    };
}

#[cfg(feature = "sequence")]
pub mod sequence;