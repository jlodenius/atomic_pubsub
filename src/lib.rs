//!
//! @TODO: Write crate docs
//!
#![allow(dead_code)]

mod adapters;

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::hash::Hash;
use std::sync::Mutex;

pub type ResultBoxedError<T> = Result<T, Box<dyn Error>>;

pub trait Adapter {
    fn new() -> Self;
    fn recv<R: From<String>>(
        &mut self,
        clear_internal_queue: Box<dyn FnOnce(&str, usize) -> ResultBoxedError<()>>,
    ) -> ResultBoxedError<Option<(String, R)>>;
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
    pub fn listen(&'static mut self) {
        // Closure to be executed from within adapter
        let clear_queue_closure = |device: &str, count: usize| -> ResultBoxedError<()> {
            let mut commands = self.commands.lock()?;
            if let Some(queue) = commands.get_mut(device) {
                if queue.len() - count == 0 {
                    // Clear entire queue
                    commands.remove(device);
                } else {
                    // Clear executed commands
                    queue.drain(..count);
                }
            }
            Result::Ok(())
        };

        loop {
            let res = self.adapter.recv(Box::new(clear_queue_closure));
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

        let aps: AtomicPubsub<Resolvers, ReturnType, adapters::redis::RedisAdapter> =
            AtomicPubsub {
                adapter,
                commands: Mutex::new(HashMap::new()),
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
