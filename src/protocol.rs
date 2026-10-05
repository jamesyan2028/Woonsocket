use serde::{Deserialize, Serialize};
use woonsocket_work::Work;

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub id: u64,
    pub work: Work,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub id: u64,
    pub server_time_ns: u64,
    pub server_queue_ns: u64,
    pub payload: Vec<u8>,
}
