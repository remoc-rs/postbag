use std::io::Write;

use serde::{Serialize, ser};

use crate::{
    FALSE, ID_COUNT, ID_LEN, ID_LEN_NAME, NONE, SOME, SPECIAL_LEN, TRUE, UNKNOWN_LEN,
    cfg::Cfg,
    error::{Error, Result},
    ser::skippable::SkipWrite,
    varint::*,
};

/// Serializer
pub struct Serializer<W, const WITH_IDENTS: bool> {
    output: SkipWrite<W>,
    remaining_depth: usize,
}

impl<W: Write, const WITH_IDENTS: bool> Serializer<W, WITH_IDENTS> {
    /// Creates a new serializer using the specified configuration.
    pub fn new(write: W, cfg: Cfg<WITH_IDENTS>) -> Self {
        Self { output: SkipWrite::new(write), remaining_depth: cfg.depth_limit() }
    }

    /// Executes `f` with the nesting depth counter increased by one.
    ///
    /// Fails with [`Error::RecursionLimit`] when the configured depth limit
    /// would be exceeded. This bounds stack usage, since serialization of
    /// nested data is recursive.
    pub(crate) fn recurse<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let Some(remaining) = self.remaining_depth.checked_sub(1) else {
            return Err(Error::RecursionLimit);
        };

        self.remaining_depth = remaining;
        let res = f(self);
        self.remaining_depth += 1;

        res
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

    fn write_identifier(&mut self, ident: &str) -> Result<()> {
        match ident.strip_prefix("_").and_then(|s| s.parse::<usize>().ok()) {
            Some(id) if id < ID_COUNT => {
                self.write_usize(ID_LEN_NAME + id)?;
            }
            _ => {
                let len = ident.len();
                if len < ID_LEN {
                    self.write_usize(len)?;
                } else {
                    self.write_usize(ID_LEN)?;
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
    type SerializeTupleVariant = Self;
    type SerializeMap = MapSerializer<'a, W, WITH_IDENTS>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

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
        self.write_usize(v.len())?;
        self.output.write(v.as_bytes())?;
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.write_usize(v.len())?;
        Ok(self.output.write(v)?)
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_u8(NONE)
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_u8(SOME)?;
        self.recurse(|ser| value.serialize(ser))
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
        if WITH_IDENTS {
            self.write_identifier(variant)?;
        } else {
            self.write_u32(variant_index)?;
        }
        Ok(())
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.recurse(|ser| value.serialize(ser))
    }

    fn serialize_newtype_variant<T>(
        self, _name: &'static str, variant_index: u32, variant: &'static str, value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        if WITH_IDENTS {
            self.write_identifier(variant)?;
        } else {
            self.write_u32(variant_index)?;
        }
        self.recurse(|ser| value.serialize(ser))?;

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
        self, _name: &'static str, variant_index: u32, variant: &'static str, _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        if WITH_IDENTS {
            self.write_identifier(variant)?;
        } else {
            self.write_u32(variant_index)?;
        }

        Ok(self)
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
        self.write_usize(len)?;

        if !WITH_IDENTS {
            self.output.start_skippable();
        }

        Ok(self)
    }

    fn serialize_struct_variant(
        self, _name: &'static str, variant_index: u32, variant: &'static str, len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        if WITH_IDENTS {
            self.write_identifier(variant)?;
        } else {
            self.write_u32(variant_index)?;
        }

        self.write_usize(len)?;

        if !WITH_IDENTS {
            self.output.start_skippable();
        }

        Ok(self)
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
        self.serializer.recurse(|ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        if self.len.is_none() {
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
        (**self).recurse(|ser| value.serialize(ser))
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
        (**self).recurse(|ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<W, const WITH_IDENTS: bool> ser::SerializeTupleVariant for &mut Serializer<W, WITH_IDENTS>
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
        (**self).recurse(|ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        Ok(())
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
        self.serializer.recurse(|ser| key.serialize(ser))
    }

    #[inline(never)]
    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serializer.recurse(|ser| value.serialize(ser))
    }

    fn end(self) -> Result<()> {
        if self.len.is_none() {
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
            self.write_identifier(key)?;
            self.output.start_skippable();
        }

        (**self).recurse(|ser| value.serialize(ser))?;

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

impl<W, const WITH_IDENTS: bool> ser::SerializeStructVariant for &mut Serializer<W, WITH_IDENTS>
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
            self.write_identifier(key)?;
            self.output.start_skippable();
        }

        (**self).recurse(|ser| value.serialize(ser))?;

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
