//! Sources of the bytes to deserialize.

use std::io::{self, BufRead, ErrorKind, Read};

/// How much is allocated up front for a read whose length the input claims
/// but has not yet delivered, when the input does not tell how much it holds.
///
/// Larger reads allocate further as the data arrives, so the allocation never
/// exceeds about twice the data delivered.
const READ_CHUNK: usize = 64 * 1024;

/// Source of the bytes to deserialize.
pub trait Input {
    /// Fills `buf` completely.
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()>;

    /// Appends `ct` bytes to `buf`.
    fn read_into(&mut self, buf: &mut Vec<u8>, ct: usize) -> io::Result<()>;

    /// Reads and discards `ct` bytes.
    fn skip(&mut self, ct: usize) -> io::Result<()>;

    /// How many bytes the input delivers at most, if known.
    fn limit(&self) -> Option<usize>;
}

/// Input from a reader.
pub struct ReadInput<R>(pub R);

impl<R: Read> Input for ReadInput<R> {
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.0.read_exact(buf)
    }

    fn read_into(&mut self, buf: &mut Vec<u8>, ct: usize) -> io::Result<()> {
        let end = buf.len() + ct;
        while buf.len() < end {
            let chunk = (end - buf.len()).min(READ_CHUNK);
            buf.reserve(chunk);
            let n = self.0.by_ref().take(chunk as u64).read_to_end(buf)?;
            if n < chunk {
                return Err(ErrorKind::UnexpectedEof.into());
            }
        }
        Ok(())
    }

    fn skip(&mut self, ct: usize) -> io::Result<()> {
        let n = io::copy(&mut self.0.by_ref().take(ct as u64), &mut io::sink())?;
        if n < ct as u64 {
            return Err(ErrorKind::UnexpectedEof.into());
        }
        Ok(())
    }

    fn limit(&self) -> Option<usize> {
        None
    }
}

/// Input from a buffered reader, delivering at most `limit` bytes if known.
///
/// The data is copied directly out of the buffer of the reader, so that memory
/// need not be initialized before reading into it.
pub struct BufReadInput<R> {
    inner: R,
    limit: Option<usize>,
}

impl<R: BufRead> BufReadInput<R> {
    /// Input from the buffered reader, delivering at most `limit` bytes if known.
    pub fn new(inner: R, limit: Option<usize>) -> Self {
        Self { inner, limit }
    }

    /// Passes the data of the next `ct` bytes to `f`, in pieces as buffered by the reader.
    fn consume_with(&mut self, mut ct: usize, mut f: impl FnMut(&[u8])) -> io::Result<()> {
        if self.limit.is_some_and(|limit| ct > limit) {
            return Err(ErrorKind::UnexpectedEof.into());
        }

        while ct > 0 {
            let data = match self.inner.fill_buf() {
                Ok(data) => data,
                Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                Err(err) => return Err(err),
            };
            if data.is_empty() {
                return Err(ErrorKind::UnexpectedEof.into());
            }

            let n = data.len().min(ct);
            f(&data[..n]);
            self.inner.consume(n);
            ct -= n;
            if let Some(limit) = &mut self.limit {
                *limit -= n;
            }
        }

        Ok(())
    }
}

impl<R: BufRead> Input for BufReadInput<R> {
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut pos = 0;
        self.consume_with(buf.len(), |data| {
            buf[pos..pos + data.len()].copy_from_slice(data);
            pos += data.len();
        })
    }

    fn read_into(&mut self, buf: &mut Vec<u8>, ct: usize) -> io::Result<()> {
        if self.limit.is_none() {
            buf.reserve(ct.min(READ_CHUNK));
        }
        self.consume_with(ct, |data| buf.extend_from_slice(data))
    }

    fn skip(&mut self, ct: usize) -> io::Result<()> {
        self.consume_with(ct, |_| ())
    }

    fn limit(&self) -> Option<usize> {
        self.limit
    }
}
