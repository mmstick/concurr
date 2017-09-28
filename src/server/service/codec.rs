use super::{JobEvent, ResponseEvent};
use bytes::BytesMut;
use std::io;
use tokio_io::codec::{Decoder, Encoder};

pub struct ConcurrCodec;

impl Decoder for ConcurrCodec {
    type Item = JobEvent;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<JobEvent>> {
        if let Some(i) = buf.iter().position(|&b| b == b'\n') {
            // remove the serialized frame from the buffer.
            let argument = buf.split_to(i - 1);
            // Also remove the '\r\n'
            buf.split_to(2);

            if argument.len() < 5 {
                Err(io::Error::new(io::ErrorKind::Other, "invalid call"))
            } else {
                match &argument[..3] {
                    b"com" => JobEvent::get_command(&argument[4..]),
                    b"inp" => JobEvent::get_input(&argument[4..]),
                    b"get" => JobEvent::get_option(&argument[4..]),
                    b"del" => JobEvent::del_command(&argument[4..]),
                    _ => Err(io::Error::new(io::ErrorKind::Other, "invalid instruction")),
                }
            }
        } else {
            Ok(None)
        }
    }
}

impl Encoder for ConcurrCodec {
    type Item = ResponseEvent;
    type Error = io::Error;

    fn encode(&mut self, msg: ResponseEvent, buf: &mut BytesMut) -> io::Result<()> {
        buf.extend(msg.to_string().as_bytes());
        buf.extend(b"\n");
        Ok(())
    }
}
