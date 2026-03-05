use std::io::{Error, ErrorKind};
use ciborium_io::{Read, Write};

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct Writer<'a> {
    buf: &'a mut Vec<u8>,
}

impl<'a> Write for Writer<'a> {
    type Error = Error;

    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        if !data.is_empty() {
            self.buf.extend_from_slice(data);
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.buf.push(0x0);
        Ok(())
    }
}

#[allow(unused)]
impl<'a> Writer<'a> {
    pub(crate) fn new(input: &'a mut Vec<u8>) -> Self {
        Self { buf: input }
    }
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Read for Reader<'a> {
    type Error = Error;

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        if self.data.len() < self.pos {
            return Err(Self::Error::from(ErrorKind::UnexpectedEof));
        }

        let remaining_len = self.data.len() - self.pos;
        if remaining_len >= buf.len() {
            buf.copy_from_slice(&self.data[self.pos..self.pos + buf.len()]);
            self.pos += buf.len();
        } else {
            buf.copy_from_slice(&self.data[self.pos..]);
            self.pos = self.data.len();
        }
        Ok(())
    }
}

#[allow(unused)]
impl<'a> Reader<'a> {
    pub(crate) fn new(input: &'a [u8]) -> Self {
        Self {
            data: input,
            pos: 0
        }
    }
}
