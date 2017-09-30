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
                // Match the corresponding instruction to it's event.
                match &argument[..3] {
                    // Signals to create a command.
                    b"com" => JobEvent::get_command(&argument[4..]),
                    // Signals to process an input.
                    b"inp" => JobEvent::get_input(&argument[4..]),
                    // Signals to obtain some information about the server.
                    b"get" => JobEvent::get_option(&argument[4..]),
                    // Signals to remove a job from the command pool.
                    b"del" => JobEvent::del_command(&argument[4..]),
                    // The client has sent an invalid instruction.
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
