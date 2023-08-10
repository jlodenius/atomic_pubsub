use crate::{Adapter, ResultBoxedError};

use redis::{self, Commands};
use std::convert::From;

pub struct RedisAdapter {
    connection: redis::Connection,
}

impl Adapter for RedisAdapter {
    /// Panics if unable to establish redis connection
    fn new() -> Self {
        let redis_url = std::env::var("REDIS_URL").expect("Missing env REDIS_URL");
        let client = redis::Client::open(redis_url).expect("Failed to connect to redis");
        let connection = client
            .get_connection()
            .expect("Failed to get redis connection");

        Self { connection }
    }

    /// Attempts to receive a message from redis pubsub.
    /// Blocks until a message is received.
    fn recv<R: From<String>>(&mut self) -> ResultBoxedError<Option<(String, R)>> {
        let msg = self.connection.as_pubsub().get_message()?;
        let payload: String = msg.get_payload()?;

        match payload.split(':').collect::<Vec<&str>>()[..] {
            [device, "CLEAR", count] => {
                // Clear the internal queue
                let count: usize = count.parse()?;
                Ok(Option::None)
            }
            [device, command] => {
                // Return command
                Ok(Some((device.to_string(), R::from(command.to_string()))))
            }
            _ => {
                // @TODO: Log invalid_payload or smth?
                Ok(Option::None)
            }
        }
    }

    /// Sends a command to all listeners to clear x commands from internal queue
    fn clear(&self, device: &str, count: usize) {
        // @TODO: Error handling & retry?
        let redis_url = std::env::var("REDIS_URL").expect("Missing env REDIS_URL");
        let client = redis::Client::open(redis_url).unwrap();
        let mut con = client.get_connection().unwrap();
        let command = format!("{}:CLEAR:{}", device, count);
        con.publish::<&str, &str, u8>("COMMANDS", &command).unwrap();
    }
}
