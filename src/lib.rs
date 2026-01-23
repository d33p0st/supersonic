#![allow(internal_features)]
#![cfg_attr(all(feature = "nightly", nightly), feature(core_intrinsics))]

#![allow(dead_code)]
#![allow(unused)]

use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(all(not(nightly), feature = "nightly"))]
compile_error!("The `nightly` feature requires a nightly compiler");


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

pub mod mpmc;