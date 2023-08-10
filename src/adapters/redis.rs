use crate::Adapter;

use redis::{self, Commands};
use std::{convert::From, error::Error};

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
    fn recv<R: From<String>>(&mut self) -> Result<Option<(String, R)>, Box<dyn Error>> {
        let msg = self.connection.as_pubsub().get_message()?;
        let payload: String = msg.get_payload()?;

        match payload.split(':').collect::<Vec<&str>>()[..] {
            [device, "CLEAR", count] => {
                // Clear queue
                // @TODO: Error handling & retry?
                let redis_url = std::env::var("REDIS_URL").expect("Missing env REDIS_URL");
                let client = redis::Client::open(redis_url).unwrap();
                let mut con = client.get_connection().unwrap();
                let command = format!("{}:CLEAR:{}", device, count);
                con.publish::<&str, &str, u8>("COMMANDS", &command).unwrap();

                Ok(Option::None)
            }
            [device, command] => {
                // Return command
                Ok(Some((device.to_string(), R::from(command.to_string()))))
            }
            _ => {
                // @TODO: Log error?
                Ok(Option::None)
            }
        }
    }
}
