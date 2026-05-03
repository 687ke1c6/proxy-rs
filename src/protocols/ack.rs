use proxy_rs_derive::StreamCodec;


#[derive(StreamCodec)]
pub struct Ack {
    pub ack: u8,
    pub msg: String,
}

impl Ack {
    pub fn no_ack(ack: u8, message: Option<String>) -> Self {
        let msg = if let Some(m) = message {m} else {String::new()};
        if ack == 0 {
            panic!("Failed ack should be > 0")
        }        
        Ack { 
            ack, 
            msg
        }
    }
    pub fn ack() -> Self {
        Ack { 
            ack: 0, 
            msg: String::new()
        }
    }
}