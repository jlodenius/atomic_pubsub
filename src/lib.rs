//!
//! @TODO: Write a better desc, this sucks
//!
//! Atomic Pubsub is created to be a simple _atomic_ pubsub library.
//! Atomic in the sense that multiple threads/pods should be able to
//! subscribe to the same queue but only one subscriber will ever execute
//! any given command published to the underlying queue.
//!

#![allow(dead_code)]

mod adapters;

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::hash::Hash;

pub trait Adapter {
    fn new() -> Self;
    fn recv<R: From<String>>(&mut self) -> Result<Option<(String, R)>, Box<dyn Error>>;
}

pub struct AtomicPubsub<R, T, A>
where
    R: Eq,
    R: PartialEq,
    R: Hash,
{
    adapter: A,
    commands: HashMap<String, VecDeque<R>>,
    resolvers: HashMap<R, Box<dyn Fn() -> T>>,
}

/// R: Resolver enum
/// T: Return type (unnecessary?)
/// A: Adapter
impl<R, T, A> AtomicPubsub<R, T, A>
where
    R: Eq,
    R: PartialEq,
    R: Hash,
    R: From<String>,
    A: Adapter,
{
    /// Start listening for incoming commands
    pub fn listen(&mut self) {
        loop {
            let res = self.adapter.recv();
            if res.is_err() {
                // Do something about error?
                return;
            }
            // Insert command to internal queue
            if let Some((device, cmd)) = res.unwrap() {
                self.commands
                    .entry(device)
                    .or_insert(VecDeque::with_capacity(5))
                    .push_back(cmd);
            }
        }
    }

    /// Execute commands for a specified device
    fn exec(&mut self, device: &str) {
        if let Some(queue) = self.commands.get(device) {
            queue.iter().for_each(|cmd| {
                self.resolvers.get(cmd).unwrap()();
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type ReturnType = bool;

    #[derive(Eq, Hash, PartialEq)]
    enum Resolvers {
        PowerOff,
    }

    #[test]
    fn it_kinda_works() {
        let test_resolver = || true;
        let adapter = adapters::redis::RedisAdapter::new();

        let aps: AtomicPubsub<Resolvers, ReturnType, adapters::redis::RedisAdapter> =
            AtomicPubsub {
                adapter,
                commands: HashMap::new(),
                resolvers: vec![(
                    Resolvers::PowerOff,
                    Box::new(test_resolver) as Box<dyn Fn() -> ReturnType>,
                )]
                .into_iter()
                .collect(),
            };
        let the_resolver = aps.resolvers.get(&Resolvers::PowerOff).unwrap();
        assert_eq!(the_resolver(), true);
    }
}
