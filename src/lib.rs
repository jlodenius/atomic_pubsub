//!
//! @TODO: Write crate docs
//!
#![allow(dead_code)]

pub mod adapters;

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::hash::Hash;
use std::sync::Mutex;

// Type aliases
pub type ResultBoxedError<T> = Result<T, Box<dyn Error>>;
pub type ThreadSafeFn<T> = Box<dyn Fn() -> T + Send + 'static>;

pub trait Adapter {
    fn new() -> Self;
    fn recv<R: From<String>>(&mut self) -> ResultBoxedError<Option<(String, R)>>;
    fn clear(&self, device: &str, count: usize);
}

pub struct AtomicPubsub<R, T, A>
where
    R: Eq,
    R: PartialEq,
    R: Hash,
    R: From<String>,
    A: Adapter,
{
    adapter: A,
    commands: Mutex<HashMap<String, VecDeque<R>>>,
    resolvers: HashMap<R, ThreadSafeFn<T>>,
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
    pub fn new(adapter: A, resolvers: HashMap<R, ThreadSafeFn<T>>) -> Self {
        AtomicPubsub {
            adapter,
            commands: Mutex::new(HashMap::new()),
            resolvers,
        }
    }
    /// Start listening for incoming commands
    pub fn listen(&mut self) {
        // Closure to be executed from within adapter

        loop {
            let res = self.adapter.recv();
            if res.is_err() {
                // Should catch all errors in recv, including potential
                // errors from clear_internal_queue closure
                // @TODO: Handle/log errors?
                return;
            }
            // Insert command to internal queue
            if let Some((device, cmd)) = res.unwrap() {
                self.commands
                    .lock()
                    .unwrap()
                    .entry(device)
                    .or_insert(VecDeque::with_capacity(5))
                    .push_back(cmd);
            }
        }
    }

    /// Execute commands for a specified device
    fn exec(&mut self, device: &str) {
        if let Some(queue) = self.commands.lock().unwrap().get(device) {
            queue.iter().for_each(|cmd| {
                self.resolvers.get(cmd).unwrap()();
            });
            // Send clear command
            self.adapter.clear(device, queue.len());
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
    impl From<String> for Resolvers {
        fn from(_: String) -> Self {
            Self::PowerOff
        }
    }

    #[test]
    fn it_kinda_works() {
        let test_resolver = || true;
        let adapter = adapters::redis::RedisAdapter::new();
        let aps = AtomicPubsub::new(
            adapter,
            vec![(
                Resolvers::PowerOff,
                Box::new(test_resolver) as ThreadSafeFn<ReturnType>,
            )]
            .into_iter()
            .collect(),
        );

        let the_resolver = aps.resolvers.get(&Resolvers::PowerOff).unwrap();
        assert_eq!(the_resolver(), true);
    }
}
