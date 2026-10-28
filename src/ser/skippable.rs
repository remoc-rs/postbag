//! Skippable blocks writer.

use std::{
    io::{Result, Write},
    mem,
};

use crate::varint::{varint_max, varint_u16};

/// Writer that allows block to be (partially) skipped during reading.
pub struct SkipWrite<W>(SkipStack<W>);

impl<W: Write> SkipWrite<W> {
    /// Creates a new skip writer.
    pub fn new(inner: W) -> Self {
        Self(SkipStack::Base(inner))
    }

    /// Write bytes.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.0.write(data)
    }

    /// Opens a skippable block.
    ///
    /// Must be paired with a call to [`Self::end_skippable`].
    pub fn start_skippable(&mut self) {
        let this = mem::replace(&mut self.0, SkipStack::Dummy);
        self.0 = SkipStack::SkipBlock(SkipBlock::new(this));
    }

    /// Finishes a skippable block.
    pub fn end_skippable(&mut self) -> Result<()> {
        match mem::replace(&mut self.0, SkipStack::Dummy) {
            SkipStack::Base(_) => panic!("no skip block is open"),
            SkipStack::SkipBlock(sb) => self.0 = sb.finish()?,
            SkipStack::Dummy => unreachable!(),
        }
        Ok(())
    }

    /// Returns the contained writer.
    pub fn into_inner(self) -> W {
        self.0.into_inner()
    }
}

enum SkipStack<W> {
    Base(W),
    SkipBlock(SkipBlock<W>),
    Dummy,
}

impl<W: Write> SkipStack<W> {
    fn write(&mut self, data: &[u8]) -> Result<()> {
        match self {
            Self::Base(inner) => inner.write_all(data),
            Self::SkipBlock(sb) => sb.write(data),
            Self::Dummy => unreachable!(),
        }
    }

    fn into_inner(self) -> W {
        match self {
            SkipStack::Base(inner) => inner,
            SkipStack::SkipBlock(sb) => sb.inner.into_inner(),
            SkipStack::Dummy => unreachable!(),
        }
    }
}

struct SkipBlock<W> {
    inner: Box<SkipStack<W>>,
    buf: Vec<u8>,
}

impl<W: Write> SkipBlock<W> {
    const MAX_LEN: usize = u16::MAX as usize;

    fn new(inner: SkipStack<W>) -> Self {
        Self { inner: Box::new(inner), buf: Vec::new() }
    }

    /// Writes data, passing every full block on directly.
    ///
    /// Only the buffered start of a full block and the data not filling one
    /// are kept in the buffer, so that large data is not copied into the
    /// buffer of each enclosing block.
    fn write(&mut self, mut data: &[u8]) -> Result<()> {
        while self.buf.len() + data.len() >= Self::MAX_LEN {
            let n = Self::MAX_LEN - self.buf.len();
            self.write_len(Self::MAX_LEN)?;
            if !self.buf.is_empty() {
                self.inner.write(&self.buf)?;
                self.buf.clear();
            }
            self.inner.write(&data[..n])?;
            data = &data[n..];
        }

        self.buf.extend_from_slice(data);
        Ok(())
    }

    fn write_len(&mut self, len: usize) -> Result<()> {
        let mut len_buf = [0; varint_max::<u16>()];
        let len_buf = varint_u16(len.try_into().unwrap(), &mut len_buf);
        self.inner.write(len_buf)
    }

    fn flush_buf(&mut self) -> Result<()> {
        self.write_len(self.buf.len())?;
        self.inner.write(&self.buf)
    }

    fn finish(mut self) -> Result<SkipStack<W>> {
        assert_ne!(self.buf.len(), Self::MAX_LEN);

        self.flush_buf()?;
        Ok(*self.inner)
    }
}
