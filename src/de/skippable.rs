//! Skippable blocks reader.

use std::{
    io::{ErrorKind, Read},
    mem,
};

use crate::{
    Error, Result,
    varint::{max_of_last_byte, varint_max},
};

/// How much is allocated up front for a read whose length the input claims
/// but has not yet delivered.
const READ_CHUNK: usize = 64 * 1024;

/// Reader that allows blocks to be (partially) skipped.
pub struct SkipRead<R> {
    stack: SkipStack<R>,
    /// How many skippable blocks are open.
    depth: usize,
    /// Kind of the first failure of the contained reader, if it has failed.
    failure: Option<ErrorKind>,
}

impl<R: Read> SkipRead<R> {
    /// Creates a new skip stack.
    /// A reader over the bytes of exactly one value.
    ///
    /// The bytes are presented as an already-open block of that length, so a
    /// value reaching the end of its block is bounded by them and cannot read
    /// beyond.
    pub fn new_value(inner: R, len: usize) -> Self {
        let stack = SkipStack::SkipBlock(SkipBlock::exact(SkipStack::Base(inner), len));
        Self { stack, depth: 1, failure: None }
    }

    pub fn new(inner: R) -> Self {
        Self { stack: SkipStack::Base(inner), depth: 0, failure: None }
    }

    /// Notes a failure of the contained reader and passes the result on.
    ///
    /// Every read goes through here, and [`Error::Io`] arises nowhere but from
    /// the contained reader, so this sees every failure of it.
    fn note_failure<T>(&mut self, res: Result<T>) -> Result<T> {
        if let Err(Error::Io(err)) = &res {
            self.failure.get_or_insert(err.kind());
        }
        res
    }

    /// Read one byte.
    ///
    /// Most of what is read is single bytes — every byte of every varint,
    /// every tag, every block length — so this must not go through the
    /// buffer-returning path and allocate for each one.
    pub fn read_u8(&mut self) -> Result<u8> {
        let res = self.stack.read_byte();
        self.note_failure(res)
    }

    /// Read `cnt` bytes.
    pub fn read(&mut self, cnt: usize) -> Result<Vec<u8>> {
        let res = self.stack.read(cnt);
        self.note_failure(res)
    }

    /// Opens a skippable block.
    ///
    /// Must be paired with a call to [`Self::end_skippable`].
    pub fn start_skippable(&mut self) {
        let this = mem::replace(&mut self.stack, SkipStack::Dummy);
        self.stack = SkipStack::SkipBlock(SkipBlock::new(this));
        self.depth += 1;
    }

    /// Opens a block that holds nothing and reads no length from the input.
    ///
    /// Must be paired with a call to [`Self::end_skippable`].
    pub fn start_empty_block(&mut self) {
        let this = mem::replace(&mut self.stack, SkipStack::Dummy);
        self.stack = SkipStack::SkipBlock(SkipBlock::exact(this, 0));
        self.depth += 1;
    }

    /// Finishes a skippable block.
    ///
    /// Remaining contents of the block are skipped if not yet read.
    pub fn end_skippable(&mut self) -> Result<()> {
        match mem::replace(&mut self.stack, SkipStack::Dummy) {
            SkipStack::Base(_) => panic!("no skip block is open"),
            SkipStack::SkipBlock(sb) => self.stack = self.note_failure(sb.finish())?,
            SkipStack::Dummy => unreachable!(),
        }

        self.depth -= 1;
        Ok(())
    }

    /// How many skippable blocks are open.
    ///
    /// Passed to [`Self::pop_to`] to return to this state.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Closes every skippable block opened since the stack was `depth` deep,
    /// skipping over whatever is left of each.
    pub fn pop_to(&mut self, depth: usize) -> Result<()> {
        assert!(depth <= self.depth, "cannot return to a depth that is not open");

        if let Some(kind) = self.failure {
            return Err(Error::Io(kind.into()));
        }

        while self.depth > depth {
            self.end_skippable()?;
        }

        Ok(())
    }

    /// Returns the contained reader.
    pub fn into_inner(self) -> R {
        self.stack.into_inner()
    }

    /// Opens a skippable block, reads all its contents, and closes it.
    ///
    /// Returns the raw bytes contained within the skippable block.
    pub fn read_skippable_block(&mut self) -> Result<Vec<u8>> {
        self.start_skippable();
        let SkipStack::SkipBlock(sb) = &mut self.stack else { unreachable!() };
        let res = sb.read_all();
        let data = self.note_failure(res)?;
        self.end_skippable()?;
        Ok(data)
    }

    /// Whether the innermost open skippable block has no bytes left.
    ///
    /// Used to find the end of a run of values that reaches the end of the
    /// block, in place of a count written before it.
    pub fn block_exhausted(&mut self) -> Result<bool> {
        let res = match &mut self.stack {
            SkipStack::SkipBlock(sb) => sb.exhausted(),
            SkipStack::Base(_) | SkipStack::Dummy => unreachable!("no block to be at the end of"),
        };
        self.note_failure(res)
    }

    /// Reads the remainder of the innermost open skippable block.
    ///
    /// Only ever called for a value that reaches the end of its block, which
    /// is why there is always a block open: the reader would otherwise have
    /// no boundary to read up to, and reading the rest of the input instead
    /// would swallow whatever follows the value.
    pub fn read_rest(&mut self) -> Result<Vec<u8>> {
        let res = match &mut self.stack {
            SkipStack::SkipBlock(sb) => sb.read_all(),
            SkipStack::Base(_) | SkipStack::Dummy => unreachable!("no block to read to the end of"),
        };
        self.note_failure(res)
    }
}

enum SkipStack<R> {
    Base(R),
    SkipBlock(SkipBlock<R>),
    Dummy,
}

impl<R: Read> SkipStack<R> {
    pub fn read(&mut self, ct: usize) -> Result<Vec<u8>> {
        match self {
            Self::Base(base) => {
                let mut buf = Vec::new();
                while buf.len() < ct {
                    let chunk = (ct - buf.len()).min(READ_CHUNK);
                    let start = buf.len();
                    buf.resize(start + chunk, 0);
                    base.read_exact(&mut buf[start..])?;
                }
                Ok(buf)
            }
            Self::SkipBlock(sb) => sb.read(ct),
            Self::Dummy => unreachable!(),
        }
    }

    fn read_byte(&mut self) -> Result<u8> {
        match self {
            Self::Base(base) => {
                let mut buf = [0u8; 1];
                base.read_exact(&mut buf)?;
                Ok(buf[0])
            }
            Self::SkipBlock(sb) => sb.read_byte(),
            Self::Dummy => unreachable!(),
        }
    }

    fn try_take_varint_u16(&mut self) -> Result<u16> {
        let mut out = 0;
        for i in 0..varint_max::<u16>() {
            let val = self.read_byte()?;
            let carry = (val & 0x7F) as u16;
            out |= carry << (7 * i);

            if (val & 0x80) == 0 {
                if i == varint_max::<u16>() - 1 && val > max_of_last_byte::<u16>() {
                    return Err(Error::BadVarint);
                } else {
                    return Ok(out);
                }
            }
        }
        Err(Error::BadVarint)
    }

    fn into_inner(self) -> R {
        match self {
            SkipStack::Base(base) => base,
            SkipStack::SkipBlock(sb) => sb.inner.into_inner(),
            SkipStack::Dummy => unreachable!(),
        }
    }
}

struct SkipBlock<R> {
    inner: Box<SkipStack<R>>,
    remaining: usize,
    has_next_block: bool,
}

impl<R: Read> SkipBlock<R> {
    const MAX_LEN: usize = u16::MAX as usize;

    fn new(inner: SkipStack<R>) -> Self {
        Self { inner: Box::new(inner), remaining: 0, has_next_block: true }
    }

    /// A block of known length whose bytes are already there, rather than one
    /// that reads its length and continuations from the input.
    fn exact(inner: SkipStack<R>, len: usize) -> Self {
        Self { inner: Box::new(inner), remaining: len, has_next_block: false }
    }

    fn update_remaining(&mut self) -> Result<()> {
        if self.remaining > 0 || !self.has_next_block {
            return Ok(());
        }

        self.remaining = self.inner.try_take_varint_u16()?.into();
        self.has_next_block = self.remaining == Self::MAX_LEN;

        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8> {
        self.update_remaining()?;

        if self.remaining == 0 {
            return Err(Error::EndOfBlock);
        }

        let byte = self.inner.read_byte()?;
        self.remaining -= 1;

        Ok(byte)
    }

    fn read(&mut self, mut ct: usize) -> Result<Vec<u8>> {
        self.update_remaining()?;

        if self.remaining >= ct {
            let buf = self.inner.read(ct)?;
            self.remaining -= ct;
            return Ok(buf);
        }

        let mut buf = Vec::with_capacity(ct.min(READ_CHUNK));
        while ct > 0 {
            self.update_remaining()?;

            if self.remaining == 0 {
                return Err(Error::EndOfBlock);
            }

            let n = ct.min(self.remaining);
            buf.extend(&self.inner.read(n)?);
            self.remaining -= n;
            ct -= n;
        }

        Ok(buf)
    }

    fn finish(mut self) -> Result<SkipStack<R>> {
        loop {
            self.update_remaining()?;

            if self.remaining > 0 {
                self.inner.read(self.remaining)?;
                self.remaining = 0;
            } else {
                break;
            }
        }

        Ok(*self.inner)
    }

    /// Whether the block has no bytes left, following it into a continuation
    /// block if there is one.
    fn exhausted(&mut self) -> Result<bool> {
        self.update_remaining()?;
        Ok(self.remaining == 0)
    }

    fn read_all(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        loop {
            self.update_remaining()?;
            if self.remaining == 0 {
                break;
            }
            let chunk = self.inner.read(self.remaining)?;
            buf.extend(chunk);
            self.remaining = 0;
        }
        Ok(buf)
    }
}
