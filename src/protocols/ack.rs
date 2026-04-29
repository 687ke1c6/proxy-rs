use anyhow::Result;
use tokio::io::{AsyncRead, AsyncWrite,  AsyncWriteExt, AsyncReadExt};

pub struct Ack {
    pub ack: u8,
    pub msg: Option<String>,
}

impl Ack {
    pub fn new(ack: u8, msg: Option<String>) -> Self {
        Ack { ack, msg }
    }

    pub async fn write_ack<W: AsyncWrite + Unpin>(&self, stream: &mut W) -> Result<()> {
        stream.write_u8(self.ack).await?;
        match &self.msg {
            Some(msg) => {
                let message_as_bytes = msg.as_bytes();
                stream.write_u16(message_as_bytes.len() as u16).await?;
                stream.write_all(message_as_bytes).await?;
            },
            _ => {}
        }
        Ok(())
    }

    pub async fn read_ack<W: AsyncRead + Unpin>(stream: &mut W) -> Result<Self> {
        let ack = stream.read_u8().await?;
        println!("Received ack: {ack}");
        match ack {
            0 => Ok(Self { ack, msg: None }),
            _ => {
                print!("bad ack");
                let msg_len = stream.read_u16().await? as usize;
                let mut msg_bytes = vec![0u8; msg_len];
                stream.read_exact(&mut msg_bytes).await?;
                Ok(Self { ack, msg: Some(String::from_utf8_lossy(&msg_bytes).to_string()) })
            }
        }
    }
}