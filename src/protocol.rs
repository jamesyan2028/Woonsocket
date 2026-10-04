user serde::{Serialize, Deserialize}

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pud id: u64,
    pub workType: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub id: u64,
    pub workTime: u64,
    pub payload: Vec<u8>,
}

