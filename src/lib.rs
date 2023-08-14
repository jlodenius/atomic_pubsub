//!
//! Extremely WIP
//!
#![allow(dead_code)]

pub mod adapters;

use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::sync::Mutex;

// Type aliases
pub type ResultBoxedError<T> = Result<T, Box<dyn Error>>;
pub type ResolverFn<'a, T> = Box<dyn Fn(&str, &mut T) + Send + Sync + 'a>;

pub trait Adapter {
    fn recv(&self) -> ResultBoxedError<Option<(String, String, Option<usize>)>>;
    fn clear(&self, device: &str, count: usize);
}

pub struct AtomicPubsub<'a, A, T>
where
    A: Adapter,
{
    adapter: A,
    resolvers: HashMap<&'a str, ResolverFn<'a, T>>,
    commands: Mutex<HashMap<String, VecDeque<String>>>,
}

impl<'a, A, T> AtomicPubsub<'a, A, T>
where
    A: Adapter,
{
    pub fn new(adapter: A, resolvers: Vec<(&'a str, ResolverFn<'a, T>)>) -> Self {
        let resolvers = resolvers.into_iter().collect();
        Self {
            adapter,
            resolvers,
            commands: Mutex::new(HashMap::new()),
        }
    }
    /// Start listening for incoming commands
    pub fn listen(&self) {
        // @TODO: Run in thread that restarts on panic
        loop {
            let res = self.adapter.recv();
            if res.is_err() {
                // @TODO: Handle/log errors?
                return;
            }
            match res.unwrap() {
                // Clear internal queue
                Some((device, _, Some(count))) => {
                    if let Ok(mut commands) = self.commands.lock() {
                        if let Some(queue) = commands.get_mut(&device) {
                            if queue.len() - count == 0 {
                                commands.remove(&device);
                            } else {
                                queue.drain(..count);
                            }
                        }
                    }
                }
                // Insert command to internal queue
                Some((device, cmd, None)) => {
                    self.commands
                        .lock()
                        .unwrap()
                        .entry(device)
                        .or_insert(VecDeque::with_capacity(5))
                        .push_back(cmd);
                }
                // Invalid payload, Log smth?
                _ => {}
            }
        }
    }

    /// Execute commands for a specified device
    pub fn exec(&mut self, device: &str, extra: &mut T) {
        let guard = self.commands.lock().unwrap();
        let Some(queue) = guard.get(device) else { return };
        queue.iter().for_each(|cmd| {
            let resolver = self.resolvers.get(cmd.as_str()).unwrap();
            resolver(device, extra);
        });
        // Send clear command
        self.adapter.clear(device, queue.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_kinda_works() {
        let adapter = adapters::redis::RedisAdapter::new().await;
        let aps: AtomicPubsub<_, i32> = AtomicPubsub::new(
            adapter,
            vec![
                (
                    "POWER_ON",
                    Box::new(|x, y| print!("POWER_ON ---> {} {}", x, y)),
                ),
                (
                    "POWER_OFF",
                    Box::new(|x, y| print!("POWER_OFF ---> {} {}", x, y)),
                ),
            ],
        );
        let the_resolver = aps.resolvers.get("POWER_OFF").unwrap();
        assert_eq!(the_resolver("some_device", &mut 5), ());
    }
}
