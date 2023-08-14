use crate::{Adapter, ResultBoxedError};

use redis::{self, aio::ConnectionManager, AsyncCommands};
use std::sync::Mutex;

pub struct RedisAdapter {
    connection: Mutex<redis::Connection>,
    connection_manager: Mutex<ConnectionManager>,
}

impl RedisAdapter {
    /// Panics if unable to establish redis connection
    pub async fn new() -> Self {
        let redis_url = std::env::var("REDIS_URL").expect("Missing env REDIS_URL");
        let client = redis::Client::open(redis_url).expect("Failed to connect to redis");

        // Connection for pubsub, until ConnectionManager implements pubsub
        let client_clone = client.clone();
        let connection = client_clone
            .get_connection()
            .expect("Failed to get redis connection");

        // @TODO: Figure out best retry exponent/factor
        let connection_manager = ConnectionManager::new_with_backoff(client, 10, 2, 5)
            .await
            .expect("Failed to create ConnectionManager");

        Self {
            connection: Mutex::new(connection),
            connection_manager: Mutex::new(connection_manager),
        }
    }
}

impl Adapter for RedisAdapter {
    /// Attempts to receive a message from redis pubsub.
    /// Blocks until a message is received.
    fn recv(&self) -> ResultBoxedError<Option<(String, String, Option<usize>)>> {
        let msg = self.connection.lock().unwrap().as_pubsub().get_message()?;
        let payload: String = msg.get_payload()?;

        match payload.split(':').collect::<Vec<&str>>()[..] {
            [device, "CLEAR", count] => {
                // Clear the internal queue
                let count: usize = count.parse()?;
                Ok(Some((device.to_string(), "CLEAR".to_string(), Some(count))))
            }
            [device, command] => {
                // Return command
                Ok(Some((
                    device.to_string(),
                    command.to_string(),
                    Option::None,
                )))
            }
            _ => Ok(Option::None),
        }
    }

    /// Sends a command to all listeners to clear x commands from internal queue
    fn clear(&self, device: &str, count: usize) {
        let command = format!("{}:CLEAR:{}", device, count);
        let mut connection = self.connection_manager.lock().unwrap();
        connection.publish::<&str, &str, usize>("", &command);
    }
}
