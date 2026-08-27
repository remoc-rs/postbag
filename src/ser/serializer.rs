use std::io::Write;

use serde::{Serialize, ser};

use crate::{
    FALSE, MORE, NO_MORE, NONE, SOME, SPECIAL_LEN, TRUE, UNKNOWN_LEN,
    cfg::{Cfg, Version},
    error::{Error, Result},
    id::{ID_BLOCK, ID_LEN, ID_LEN_NAME, ident_number},
    recoverable::Recoverable,
    ser::skippable::SkipWrite,
    varint::*,
};

/// Serializer
pub struct Serializer<W, const WITH_IDENTS: bool> {
    output: SkipWrite<W>,
    remaining_depth: usize,
    version: Version,
    /// Whether the value about to be serialized reaches the end of the
    /// enclosing skippable block, so that it need not state its own length.
    ///
    /// Set immediately before serializing such a value and taken by the
    /// `serialize_*` method that runs next. Every value nested inside another
    /// is serialized through [`Self::recurse`], which sets the flag from its
    /// argument, so a nested value has the property only where the enclosing
    /// one deliberately passes it on.
    owns_block: bool,
}

impl<W: Write, const WITH_IDENTS: bool> Serializer<W, WITH_IDENTS> {
    /// Creates a new serializer using the specified configuration.
    pub fn new(write: W, cfg: Cfg<WITH_IDENTS>) -> Self {
        Self {
            output: SkipWrite::new(write),
            remaining_depth: cfg.depth_limit(),
            version: cfg.version(),
            owns_block: false,
        }
    }

    /// Executes `f` with the nesting depth counter increased by one.
    ///
    /// Fails with [`Error::RecursionLimit`] when the configured depth limit
    /// would be exceeded. This bounds stack usage, since serialization of
    /// nested data is recursive.
    ///
    /// `owns_block` states whether the nested value reaches the end of the
    /// enclosing skippable block; see [`Self::owns_block`]. It is `false` for
    /// every value that has something written after it, which is all of them
    /// except the single child of a construct that writes nothing of its own
    /// afterwards.
    pub(crate) fn recurse<T>(&mut self, owns_block: bool, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let Some(remaining) = self.remaining_depth.checked_sub(1) else {
            return Err(Error::RecursionLimit);
        };

        self.remaining_depth = remaining;
        self.owns_block = owns_block;
        let res = f(self);
        self.remaining_depth += 1;

        res
    }

    /// Whether the value being serialized reaches the end of its block, and
    /// so can leave out its own length. Clears the flag.
    fn takes_block(&mut self) -> bool {
        std::mem::take(&mut self.owns_block)
    }

    /// Whether a field value may leave out its own length.
    ///
    /// Only in the `Full` configuration, which is the only one that wraps
    /// field values in a block that states where they end.
    fn field_owns_block(&self) -> bool {
        WITH_IDENTS && !self.version.is_0_4()
    }

    /// Whether the tag of an enum variant carries the flag stating whether a
    /// block holding its payload follows.
    fn variant_is_tagged(&self, owns_block: bool) -> bool {
        self.field_owns_block() && !owns_block
    }

    /// Get the writer.
    pub fn finalize(self) -> W {
        self.output.into_inner()
    }

    fn write_usize(&mut self, data: usize) -> Result<()> {
        let value = u64::try_from(data).map_err(|_| Error::UsizeOverflow)?;
        self.write_u64(value)
    }

    fn write_u128(&mut self, data: u128) -> Result<()> {
        let mut buf = [0u8; varint_max::<u128>()];
        let used_buf = varint_u128(data, &mut buf);
        self.output.write(used_buf)?;
        Ok(())
    }

    fn write_u64(&mut self, data: u64) -> Result<()> {
        let mut buf = [0u8; varint_max::<u64>()];
        let used_buf = varint_u64(data, &mut buf);
        self.output.write(used_buf)?;
        Ok(())
    }

    fn write_u32(&mut self, data: u32) -> Result<()> {
        let mut buf = [0u8; varint_max::<u32>()];
        let used_buf = varint_u32(data, &mut buf);
        self.output.write(used_buf)?;
        Ok(())
    }

    fn write_u16(&mut self, data: u16) -> Result<()> {
        let mut buf = [0u8; varint_max::<u16>()];
        let used_buf = varint_u16(data, &mut buf);
        self.output.write(used_buf)?;
        Ok(())
    }

    /// Writes an identifier and block flag.
    fn write_identifier(&mut self, ident: &str, block: bool) -> Result<()> {
        match ident_number(ident) {
            Some(id) => {
                // Identifier encoded as number
                let mut id = ID_LEN_NAME + id;
                if block {
                    id |= ID_BLOCK;
                }
                self.output.write(&[id])?;
            }
            None => {
                let len = ident.len();
                if len < ID_LEN.into() {
                    // Identifier encoded as string shorter than ID_LEN
                    let mut len = len as u8;
                    if block {
                        len |= ID_BLOCK;
                    }
                    self.output.write(&[len])?;
                } else {
                    // Identifier encoded as string
                    let mut id = ID_LEN;
                    if block {
                        id |= ID_BLOCK;
                    }
                    self.output.write(&[id])?;
                    self.write_usize(len)?;
                }

                self.output.write(ident.as_bytes())?;
            }
        }

        Ok(())
    }
}

impl<'a, W, const WITH_IDENTS: bool> ser::Serializer for &'a mut Serializer<W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSerializer<'a, W, WITH_IDENTS>;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = VariantSerializer<'a, W, WITH_IDENTS>;
    type SerializeMap = MapSerializer<'a, W, WITH_IDENTS>;
    type SerializeStruct = Self;
    type SerializeStructVariant = VariantSerializer<'a, W, WITH_IDENTS>;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.serialize_u8(if v { TRUE } else { FALSE })
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_u8(v.to_le_bytes()[0])
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        let zzv = zig_zag_i16(v);
        self.write_u16(zzv)
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        let zzv = zig_zag_i32(v);
        self.write_u32(zzv)
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        let zzv = zig_zag_i64(v);
        self.write_u64(zzv)
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        let zzv = zig_zag_i128(v);
        self.write_u128(zzv)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        Ok(self.output.write(&[v])?)
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.write_u16(v)
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.write_u32(v)
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.write_u64(v)
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        self.write_u128(v)
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        let buf = v.to_bits().to_le_bytes();
        Ok(self.output.write(&buf)?)
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        let buf = v.to_bits().to_le_bytes();
        Ok(self.output.write(&buf)?)
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0u8; 4];
        let strsl = v.encode_utf8(&mut buf);
        strsl.serialize(self)
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        // The length is the number of UTF-8 bytes and nothing else, so a value
        // reaching the end of its block has already been given it.
        if !self.takes_block() {
            self.write_usize(v.len())?;
        }
        self.output.write(v.as_bytes())?;
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        if !self.takes_block() {
            self.write_usize(v.len())?;
        }
        Ok(self.output.write(v)?)
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_u8(NONE)
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // `Some` writes its tag and then the value, so the value still reaches
        // the end of the block.
        let owns_block = self.takes_block();
        self.serialize_u8(SOME)?;
        self.recurse(owns_block, |ser| value.serialize(ser))
    }

    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_variant(
        self, _name: &'static str, variant_index: u32, variant: &'static str,
    ) -> Result<()> {
        self.takes_block();

        if WITH_IDENTS {
            self.write_identifier(variant, false)?;
        } else {
            self.write_u32(variant_index)?;
        }
        Ok(())
    }

    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let owns_block = self.takes_block();

        // A recoverable value must be enclosed in a block of its own, so that
        // deserialization can skip over it once it fails. Where the value
        // already reaches the end of the enclosing block, that block bounds it
        // just as well and nothing is added, making the representation the
        // same as without the wrapper.
        let block = name == Recoverable::NEWTYPE_NAME && !owns_block && !self.version.is_0_4();
        if block {
            self.output.start_skippable();
        }

        // A newtype struct writes nothing of its own, so the value keeps the
        // whole block. Leaving out its own length is only supported where a
        // block encloses each field value to begin with, so a block opened
        // here is handed on as such only there.
        let value_owns_block = if block { self.field_owns_block() } else { owns_block };
        self.recurse(value_owns_block, |ser| value.serialize(ser))?;

        if block {
            self.output.end_skippable()?;
        }

        Ok(())
    }

    fn serialize_newtype_variant<T>(
        self, _name: &'static str, variant_index: u32, variant: &'static str, value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let owns_block = self.takes_block();
        let block = self.variant_is_tagged(owns_block);

        if WITH_IDENTS {
            if block {
                self.write_identifier(variant, true)?;
                self.output.start_skippable();
            } else {
                self.write_identifier(variant, false)?;
            }
        } else {
            self.write_u32(variant_index)?;
        }

        // The identifier comes first and the payload still reaches the end of
        // a block, whether that is the enclosing one or the one just opened.
        self.recurse(block || owns_block, |ser| value.serialize(ser))?;

        if block {
            self.output.end_skippable()?;
        }

        Ok(())
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        match len {
            Some(SPECIAL_LEN) => {
                self.write_usize(SPECIAL_LEN)?;
                self.write_usize(SPECIAL_LEN)?;
            }
            Some(len) => self.write_usize(len)?,
            None => {
                self.write_usize(SPECIAL_LEN)?;
                self.write_usize(UNKNOWN_LEN)?;
                self.output.start_skippable();
            }
        }

        Ok(SeqSerializer { serializer: self, len })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(self)
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeTupleStruct> {
        Ok(self)
    }

    fn serialize_tuple_variant(
        self, _name: &'static str, variant_index: u32, variant: &'static str, len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        let owns_block = self.takes_block();
        let block = self.variant_is_tagged(owns_block) && len > 0;

        if WITH_IDENTS {
            self.write_identifier(variant, block)?;
        } else {
            self.write_u32(variant_index)?;
        }

        if block {
            self.output.start_skippable();
        }

        Ok(VariantSerializer { serializer: self, block })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        match len {
            Some(SPECIAL_LEN) => {
                self.write_usize(SPECIAL_LEN)?;
                self.write_usize(SPECIAL_LEN)?;
            }
            Some(len) => self.write_usize(len)?,
            None => {
                self.write_usize(SPECIAL_LEN)?;
                self.write_usize(UNKNOWN_LEN)?;
                self.output.start_skippable();
            }
        }

        Ok(MapSerializer { serializer: self, len })
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
        // The block a field value sits in already says where the fields end,
        // and in `Full` no field is empty, so the count can be left out.
        if !self.takes_block() {
            self.write_usize(len)?;
        }

        if !WITH_IDENTS {
            self.output.start_skippable();
        }

        Ok(self)
    }

    fn serialize_struct_variant(
        self, _name: &'static str, variant_index: u32, variant: &'static str, len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        let owns_block = self.takes_block();
        let block = self.variant_is_tagged(owns_block) && len > 0;

        if WITH_IDENTS {
            self.write_identifier(variant, block)?;
        } else {
            self.write_u32(variant_index)?;
        }

        if block {
            self.output.start_skippable();
        }

        // As for a plain struct: the body reaches the end of a block, so it
        // states no field count. Under `Slim` there is no such
        // block and the count is what says where the fields end.
        if !self.field_owns_block() && !owns_block {
            self.write_usize(len)?;
        }

        if !WITH_IDENTS {
            self.output.start_skippable();
        }

        Ok(VariantSerializer { serializer: self, block: block || !WITH_IDENTS })
    }
}

pub struct SeqSerializer<'a, W, const WITH_IDENTS: bool> {
    serializer: &'a mut Serializer<W, WITH_IDENTS>,
    len: Option<usize>,
}

impl<'a, W, const WITH_IDENTS: bool> ser::SerializeSeq for SeqSerializer<'a, W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline(never)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if self.len.is_none() && !self.serializer.version.is_0_4() {
            self.serializer.output.write(&[MORE])?;
        }

        self.serializer.recurse(false, |ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        if self.len.is_none() {
            if !self.serializer.version.is_0_4() {
                self.serializer.output.write(&[NO_MORE])?;
            }
            self.serializer.output.end_skippable()?;
        }

        Ok(())
    }
}

impl<W, const WITH_IDENTS: bool> ser::SerializeTuple for &mut Serializer<W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline(never)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        (**self).recurse(false, |ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<W, const WITH_IDENTS: bool> ser::SerializeTupleStruct for &mut Serializer<W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline(never)]
    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        (**self).recurse(false, |ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

/// Serializer for the payload of a tuple or struct enum variant.
///
/// Holds whether a block was opened that has to be closed when the variant
/// ends: the one wrapping the payload, or, in `Slim`, the one a struct variant
/// has always had.
pub struct VariantSerializer<'a, W, const WITH_IDENTS: bool> {
    serializer: &'a mut Serializer<W, WITH_IDENTS>,
    block: bool,
}

impl<'a, W: Write, const WITH_IDENTS: bool> VariantSerializer<'a, W, WITH_IDENTS> {
    fn finish(self) -> Result<()> {
        if self.block {
            self.serializer.output.end_skippable()?;
        }

        Ok(())
    }
}

impl<'a, W, const WITH_IDENTS: bool> ser::SerializeTupleVariant for VariantSerializer<'a, W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline(never)]
    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serializer.recurse(false, |ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        self.finish()
    }
}

pub struct MapSerializer<'a, W, const WITH_IDENTS: bool> {
    serializer: &'a mut Serializer<W, WITH_IDENTS>,
    len: Option<usize>,
}

impl<'a, W, const WITH_IDENTS: bool> ser::SerializeMap for MapSerializer<'a, W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline(never)]
    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if self.len.is_none() && !self.serializer.version.is_0_4() {
            self.serializer.output.write(&[MORE])?;
        }

        self.serializer.recurse(false, |ser| key.serialize(ser))
    }

    #[inline(never)]
    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serializer.recurse(false, |ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        if self.len.is_none() {
            if !self.serializer.version.is_0_4() {
                self.serializer.output.write(&[NO_MORE])?;
            }
            self.serializer.output.end_skippable()?;
        }

        Ok(())
    }
}

impl<W, const WITH_IDENTS: bool> ser::SerializeStruct for &mut Serializer<W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline(never)]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if WITH_IDENTS {
            self.write_identifier(key, false)?;
            self.output.start_skippable();
        }

        // The block just started ends where this value does, so the value can
        // leave out a length of its own.
        let owns_block = self.field_owns_block();
        (**self).recurse(owns_block, |ser| value.serialize(ser))?;

        if WITH_IDENTS {
            self.output.end_skippable()?;
        }

        Ok(())
    }

    fn end(self) -> Result<()> {
        if !WITH_IDENTS {
            self.output.end_skippable()?;
        }

        Ok(())
    }
}

impl<'a, W, const WITH_IDENTS: bool> ser::SerializeStructVariant for VariantSerializer<'a, W, WITH_IDENTS>
where
    W: Write,
{
    type Ok = ();
    type Error = Error;

    #[inline(never)]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if WITH_IDENTS {
            self.serializer.write_identifier(key, false)?;
            self.serializer.output.start_skippable();
        }

        // The block just started ends where this value does, so the value can
        // leave out a length of its own.
        let owns_block = self.serializer.field_owns_block();
        self.serializer.recurse(owns_block, |ser| value.serialize(ser))?;

        if WITH_IDENTS {
            self.serializer.output.end_skippable()?;
        }

        Ok(())
    }

    fn end(self) -> Result<()> {
        self.finish()
    }
}

fn zig_zag_i16(n: i16) -> u16 {
    ((n << 1) ^ (n >> 15)) as u16
}

fn zig_zag_i32(n: i32) -> u32 {
    ((n << 1) ^ (n >> 31)) as u32
}

fn zig_zag_i64(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

fn zig_zag_i128(n: i128) -> u128 {
    ((n << 1) ^ (n >> 127)) as u128
}
