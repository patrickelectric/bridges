use std::io;
use std::io::{Error, ErrorKind};

/// Trait for objects that can act as links in a bridge.
///
/// Links are objects that know how to send an receive byte streams to their
/// respective endpoint (e.g., serial communications link, a network host, a
/// file, etc).
pub trait Linkable: io::Read + io::Write {
    /// Consume the data into the buffer.
    fn sink(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.write(buffer)?;
        self.flush()?;
        Ok(written)
    }

    /// Source new data into the buffer.
    fn source(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.read(buffer)
    }
}

/// A type that represents bridge links.
///
/// Conceptually, links are the nodes on a given infrastructure that connect to
/// other links to join two different segments of said infrastructure together.
///
/// Operations on links (e.g., input, output) are from within the links
/// perspective, so input means data coming into the link and output means data
/// going out of it.
pub struct Link<T: Linkable> {
    link: T,
    bytes_in: u64,
    bytes_out: u64,
}

impl<T: Linkable> Link<T> {
    pub fn new(link: T) -> Link<T> {
        Link {
            link: link,
            bytes_in: 0,
            bytes_out: 0,
        }
    }
    /// Have the link send the data in the buffer to it's peer link.
    pub fn output(&mut self, buffer: &[u8]) -> io::Result<()> {
        let length = buffer.len();
        let count = self.link.sink(buffer)?;
        // check if we're probably saturating OS buffers
        assert_eq!(length, count);
        self.bytes_in += count as u64;
        Ok(())
    }

    /// Have the link receive data from it's peer link and store it into the
    /// buffer.
    pub fn input(&mut self, buffer: &mut [u8]) -> io::Result<()> {
        // TODO: handle  std::io::ErrorKind::Interrupted with a retry
        if buffer.len() == 0 {
            return Err(Error::new(ErrorKind::Other, "zero length buffer"));
        }
        let count = self.link.source(buffer)?;
        self.bytes_in += count as u64;
        Ok(())
    }

    /// Total incoming bytes
    pub fn bytes_in(&self) -> u64 {
        self.bytes_in
    }

    /// Total outgoing bytes
    pub fn bytes_out(&self) -> u64 {
        self.bytes_out
    }

    /// Total transferred bytes
    pub fn bytes_total(&self) -> u64 {
        self.bytes_in + self.bytes_out
    }
}
