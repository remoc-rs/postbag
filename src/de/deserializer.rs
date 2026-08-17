use std::{borrow::Cow, collections::HashMap, io::Read, marker::PhantomData};

use serde::de::{
    self, DeserializeSeed, IntoDeserializer, Visitor,
    value::{StrDeserializer, U32Deserializer},
};

use crate::{
    FALSE, MORE, NO_MORE, NONE, SOME, SPECIAL_LEN, TRUE, UNKNOWN_LEN,
    cfg::{Cfg, Version},
    de::skippable::SkipRead,
    error::{Error, Result},
    id::{ID_BLOCK, ID_LEN, ID_LEN_NAME, numbered_ident},
    recoverable::Recoverable,
    varint::{max_of_last_byte, varint_max},
};

/// How many bytes follow a UTF-8 leading byte, or `None` if it does not
/// begin a character.
fn utf8_following_bytes(lead: u8) -> Option<usize> {
    match lead {
        0x00..=0x7f => Some(0),
        0xc2..=0xdf => Some(1),
        0xe0..=0xef => Some(2),
        0xf0..=0xf4 => Some(3),
        _ => None,
    }
}

/// The first character of `bytes`, which must begin with one.
fn first_char(bytes: &[u8]) -> Result<char> {
    str::from_utf8(bytes).map_err(|_| Error::BadChar)?.chars().next().ok_or(Error::BadChar)
}

/// Deserializer.
pub struct Deserializer<'de, R, const WITH_IDENTS: bool> {
    input: SkipRead<R>,
    remaining_depth: usize,
    version: Version,
    /// Whether the value about to be deserialized reaches the end of the
    /// enclosing skippable block, so that it did not state its own length.
    owns_block: bool,
    _de: PhantomData<&'de ()>,
}

impl<'de, R, const WITH_IDENTS: bool> Deserializer<'de, R, WITH_IDENTS>
where
    R: Read,
{
    /// Obtain a Deserializer from a reader, using the specified configuration.
    pub fn new(read: R, cfg: Cfg<WITH_IDENTS>) -> Self {
        Self::with_depth_limit(read, cfg.depth_limit(), cfg.version())
    }

    /// Obtain a Deserializer from a reader, using the specified nesting depth limit.
    fn with_depth_limit(read: R, depth_limit: usize, version: Version) -> Self {
        Deserializer {
            input: SkipRead::new(read),
            remaining_depth: depth_limit,
            version,
            owns_block: false,
            _de: PhantomData,
        }
    }

    /// Obtain a Deserializer over the bytes of exactly one field value.
    ///
    /// The value fills them, so it may have left out a length of its own; the
    /// bytes are treated as an open block so that reading to the end of the
    /// value stays bounded by them.
    fn for_field_value(read: R, len: usize, depth_limit: usize, version: Version) -> Self {
        Deserializer {
            input: SkipRead::new_value(read, len),
            remaining_depth: depth_limit,
            version,
            owns_block: WITH_IDENTS && !version.is_0_4(),
            _de: PhantomData,
        }
    }

    /// Returns the reader.
    pub fn finalize(self) -> R {
        self.input.into_inner()
    }
}

impl<'de, R: Read, const WITH_IDENTS: bool> Deserializer<'de, R, WITH_IDENTS> {
    /// Executes `f` with the nesting depth counter increased by one.
    ///
    /// Fails with [`Error::RecursionLimit`] when the configured depth limit
    /// would be exceeded. This bounds stack usage caused by deeply nested
    /// (in particular recursive) types, which would otherwise allow untrusted
    /// input to abort the process by overflowing the stack.
    ///
    /// `owns_block` states whether the nested value reaches the end of the
    /// enclosing skippable block; see [`Self::owns_block`]. It must be passed
    /// exactly where the serializer passes it, or the two read and write
    /// different bytes.
    fn recurse<T>(&mut self, owns_block: bool, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let Some(remaining) = self.remaining_depth.checked_sub(1) else {
            return Err(Error::RecursionLimit);
        };

        self.remaining_depth = remaining;
        self.owns_block = owns_block;
        let res = f(self);
        self.remaining_depth += 1;

        res
    }

    /// Whether the value being deserialized reaches the end of its block, so
    /// that it did not state its own length. Clears the flag.
    fn takes_block(&mut self) -> bool {
        std::mem::take(&mut self.owns_block)
    }

    /// Whether a field value left out its own length.
    fn field_owns_block(&self) -> bool {
        WITH_IDENTS && !self.version.is_0_4()
    }

    /// Whether the tag of an enum variant carries the flag stating whether a
    /// block holding its payload follows.
    fn variant_is_tagged(&self, owns_block: bool) -> bool {
        self.field_owns_block() && !owns_block
    }

    /// Whether another element of a sequence or map whose length was not
    /// stated follows.
    ///
    /// Under [`Version::Postbag0_4`] this reproduces what 0.4 did, including that
    /// a sequence whose elements read nothing never reaches an end.
    fn has_uncounted_element(&mut self) -> Result<bool> {
        if self.version.is_0_4() {
            // What 0.4 wrote: no announcements, so the sequence ends wherever
            // its block does.
            return Ok(!self.input.block_exhausted()?);
        }

        match self.input.read_u8()? {
            NO_MORE => Ok(false),
            MORE => Ok(true),
            _ => Err(Error::BadLen),
        }
    }

    /// Reads a run of bytes that states its own length, or, when it reaches
    /// the end of its block, runs to that end.
    fn read_len_prefixed(&mut self) -> Result<Vec<u8>> {
        if self.takes_block() {
            self.input.read_rest()
        } else {
            let sz = self.read_varint_usize()?;
            self.input.read(sz)
        }
    }

    fn read_varint_usize(&mut self) -> Result<usize> {
        let value = self.read_varint_u64()?;
        usize::try_from(value).map_err(|_| Error::UsizeOverflow)
    }

    fn read_varint_u16(&mut self) -> Result<u16> {
        let mut out = 0;
        for i in 0..varint_max::<u16>() {
            let val = self.input.read_u8()?;
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

    fn read_varint_u32(&mut self) -> Result<u32> {
        let mut out = 0;
        for i in 0..varint_max::<u32>() {
            let val = self.input.read_u8()?;
            let carry = (val & 0x7F) as u32;
            out |= carry << (7 * i);

            if (val & 0x80) == 0 {
                if i == varint_max::<u32>() - 1 && val > max_of_last_byte::<u32>() {
                    return Err(Error::BadVarint);
                } else {
                    return Ok(out);
                }
            }
        }
        Err(Error::BadVarint)
    }

    fn read_varint_u64(&mut self) -> Result<u64> {
        let mut out = 0;
        for i in 0..varint_max::<u64>() {
            let val = self.input.read_u8()?;
            let carry = (val & 0x7F) as u64;
            out |= carry << (7 * i);

            if (val & 0x80) == 0 {
                if i == varint_max::<u64>() - 1 && val > max_of_last_byte::<u64>() {
                    return Err(Error::BadVarint);
                } else {
                    return Ok(out);
                }
            }
        }
        Err(Error::BadVarint)
    }

    fn read_varint_u128(&mut self) -> Result<u128> {
        let mut out = 0;
        for i in 0..varint_max::<u128>() {
            let val = self.input.read_u8()?;
            let carry = (val & 0x7F) as u128;
            out |= carry << (7 * i);

            if (val & 0x80) == 0 {
                if i == varint_max::<u128>() - 1 && val > max_of_last_byte::<u128>() {
                    return Err(Error::BadVarint);
                } else {
                    return Ok(out);
                }
            }
        }
        Err(Error::BadVarint)
    }

    /// Reads an identifier and block flag.
    fn read_identifier(&mut self) -> Result<(Cow<'static, str>, bool)> {
        let mut id = self.input.read_u8()?;

        let block = id & ID_BLOCK != 0;
        id &= !ID_BLOCK;

        let ident = if id >= ID_LEN_NAME {
            let ident = numbered_ident(id - ID_LEN_NAME).ok_or(Error::BadIdentifier)?;
            Cow::Borrowed(ident)
        } else {
            let len = if id == ID_LEN { self.read_varint_usize()? } else { id.into() };
            let bytes = self.input.read(len)?;
            let ident = String::from_utf8(bytes).map_err(|_| Error::BadIdentifier)?;
            Cow::Owned(ident)
        };

        Ok((ident, block))
    }
}

struct SeqAccess<'a, 'b, R, const WITH_IDENTS: bool> {
    deserializer: &'a mut Deserializer<'b, R, WITH_IDENTS>,
    len: Option<usize>,
}

impl<'a, 'b: 'a, R: Read, const WITH_IDENTS: bool> serde::de::SeqAccess<'b>
    for SeqAccess<'a, 'b, R, WITH_IDENTS>
{
    type Error = Error;

    #[inline(never)]
    fn next_element_seed<V: DeserializeSeed<'b>>(&mut self, seed: V) -> Result<Option<V::Value>> {
        match &mut self.len {
            Some(0) => Ok(None),
            Some(len) => {
                *len -= 1;
                let data = DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
                Ok(Some(data))
            }
            None => {
                if !self.deserializer.has_uncounted_element()? {
                    return Ok(None);
                }

                Ok(Some(DeserializeSeed::deserialize(seed, &mut *self.deserializer)?))
            }
        }
    }

    fn size_hint(&self) -> Option<usize> {
        self.len
    }
}

struct StructSeqAccess<'a, 'b, R, const WITH_IDENTS: bool> {
    deserializer: &'a mut Deserializer<'b, R, WITH_IDENTS>,
    len: usize,
}

impl<'a, 'b: 'a, R: Read, const WITH_IDENTS: bool> serde::de::SeqAccess<'b>
    for StructSeqAccess<'a, 'b, R, WITH_IDENTS>
{
    type Error = Error;

    #[inline(never)]
    fn next_element_seed<V: DeserializeSeed<'b>>(&mut self, seed: V) -> Result<Option<V::Value>> {
        assert!(!WITH_IDENTS);

        if self.len > 0 {
            self.len -= 1;
            let data = DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

/// Streaming MapAccess for struct fields in Full mode.
///
/// Reads field identifiers and values directly from the wire without
/// buffering, using skippable blocks for forward compatibility.
struct StructFieldAccess<'a, 'b, R, const WITH_IDENTS: bool> {
    deserializer: &'a mut Deserializer<'b, R, WITH_IDENTS>,
    /// How many fields are left, or `None` when no count was written and the
    /// fields instead run to the end of the enclosing block.
    len: Option<usize>,
}

impl<'a, 'b: 'a, R: Read, const WITH_IDENTS: bool> serde::de::MapAccess<'b>
    for StructFieldAccess<'a, 'b, R, WITH_IDENTS>
{
    type Error = Error;

    #[inline(never)]
    fn next_key_seed<K: DeserializeSeed<'b>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        match &mut self.len {
            Some(0) => return Ok(None),
            Some(len) => *len -= 1,
            // No field is empty in `Full`, so running out of block is the end
            // of the fields and nothing else.
            None if self.deserializer.input.block_exhausted()? => return Ok(None),
            None => (),
        }

        let value = DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
        Ok(Some(value))
    }

    #[inline(never)]
    fn next_value_seed<V: DeserializeSeed<'b>>(&mut self, seed: V) -> Result<V::Value> {
        assert!(WITH_IDENTS);

        self.deserializer.input.start_skippable();
        // The block just started ends where this value does, so the value may
        // have left out a length of its own.
        self.deserializer.owns_block = self.deserializer.field_owns_block();
        let value = DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
        self.deserializer.owns_block = false;
        self.deserializer.input.end_skippable()?;

        Ok(value)
    }

    fn size_hint(&self) -> Option<usize> {
        self.len
    }
}

/// SeqAccess that provides pre-buffered field data in the expected order.
///
/// This allows using `visit_seq` instead of `visit_map` for struct
/// deserialization in Full mode, which produces significantly less
/// monomorphized code at the cost of buffering all field data in memory.
///
/// Activate with `RUSTFLAGS="--cfg postbag_fast_compile"`.
struct BufferedFieldSeqAccess<'de, const WITH_IDENTS: bool> {
    field_data: Vec<Option<Vec<u8>>>,
    index: usize,
    remaining_depth: usize,
    version: Version,
    _phantom: PhantomData<&'de ()>,
}

impl<'de, const WITH_IDENTS: bool> BufferedFieldSeqAccess<'de, WITH_IDENTS> {
    /// Reads all wire fields from the deserializer and reorders them to
    /// match the expected field declaration order. Unknown fields are
    /// silently dropped (forward compatibility).
    ///
    /// This constructor is deliberately NOT generic over any Visitor type
    /// so that it is monomorphized only once per (R, CFG) pair, avoiding
    /// code duplication across the many `deserialize_struct` instantiations.
    #[inline(never)]
    fn new<R: Read>(
        deser: &mut Deserializer<'_, R, WITH_IDENTS>, fields: &'static [&'static str], len: Option<usize>,
    ) -> Result<Self> {
        // Build index: field name -> position in expected order.
        let field_index: HashMap<&'static str, usize> =
            fields.iter().enumerate().map(|(i, &name)| (name, i)).collect();

        // Read wire fields and place directly into the right slot. Without a
        // count the fields run to the end of the block; no field is empty in
        // `Full`, so that boundary is unambiguous.
        let mut field_data: Vec<Option<Vec<u8>>> = vec![None; fields.len()];
        let mut remaining = len;
        loop {
            match &mut remaining {
                Some(0) => break,
                Some(n) => *n -= 1,
                None if deser.input.block_exhausted()? => break,
                None => (),
            }

            let (ident, _) = deser.read_identifier()?;
            let raw = deser.input.read_skippable_block()?;
            if let Some(&idx) = field_index.get(ident.as_ref()) {
                field_data[idx] = Some(raw);
            }
            // Unknown fields (forward compat) are silently dropped.
        }

        Ok(Self {
            field_data,
            index: 0,
            remaining_depth: deser.remaining_depth,
            version: deser.version,
            _phantom: PhantomData,
        })
    }
}

impl<'de, const WITH_IDENTS: bool> serde::de::SeqAccess<'de> for BufferedFieldSeqAccess<'de, WITH_IDENTS> {
    type Error = Error;

    #[inline(never)]
    fn next_element_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<Option<V::Value>> {
        // Skip over unfilled alias slots.
        //
        // Serde includes both aliases and canonical names in the `fields`
        // array passed to `deserialize_struct`, but `visit_seq` expects
        // exactly one element per actual struct field. Alias entries that
        // did not match any wire field are `None` and must be skipped.
        while self.index < self.field_data.len() {
            let idx = self.index;
            self.index += 1;

            if let Some(raw) = self.field_data[idx].take() {
                // The remaining depth budget must be carried over into the
                // sub-deserializer, otherwise nested structs would each start
                // with a fresh budget and the limit could not be enforced.
                let mut deser = Deserializer::<&[u8], WITH_IDENTS>::for_field_value(
                    raw.as_slice(),
                    raw.len(),
                    self.remaining_depth,
                    self.version,
                );
                let value = DeserializeSeed::deserialize(seed, &mut deser)?;
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    fn size_hint(&self) -> Option<usize> {
        let remaining = self.field_data[self.index..].iter().filter(|s| s.is_some()).count();
        Some(remaining)
    }
}

struct MapAccess<'a, 'b, R, const WITH_IDENTS: bool> {
    deserializer: &'a mut Deserializer<'b, R, WITH_IDENTS>,
    len: Option<usize>,
}

impl<'a, 'b: 'a, R: Read, const WITH_IDENTS: bool> serde::de::MapAccess<'b>
    for MapAccess<'a, 'b, R, WITH_IDENTS>
{
    type Error = Error;

    #[inline(never)]
    fn next_key_seed<K: DeserializeSeed<'b>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        match &mut self.len {
            Some(0) => Ok(None),
            Some(len) => {
                *len -= 1;
                let data = DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
                Ok(Some(data))
            }
            None => {
                if !self.deserializer.has_uncounted_element()? {
                    return Ok(None);
                }

                Ok(Some(DeserializeSeed::deserialize(seed, &mut *self.deserializer)?))
            }
        }
    }

    #[inline(never)]
    fn next_value_seed<V: DeserializeSeed<'b>>(&mut self, seed: V) -> Result<V::Value> {
        DeserializeSeed::deserialize(seed, &mut *self.deserializer)
    }

    fn size_hint(&self) -> Option<usize> {
        self.len
    }
}

impl<'de, R: Read, const WITH_IDENTS: bool> de::Deserializer<'de> for &mut Deserializer<'de, R, WITH_IDENTS> {
    type Error = Error;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::DeserializeAnyUnsupported)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let val = match self.input.read_u8()? {
            FALSE => false,
            TRUE => true,
            _ => return Err(Error::BadBool),
        };
        visitor.visit_bool(val)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.input.read_u8()? as i8)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u16()?;
        visitor.visit_i16(de_zig_zag_i16(v))
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u32()?;
        visitor.visit_i32(de_zig_zag_i32(v))
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u64()?;
        visitor.visit_i64(de_zig_zag_i64(v))
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u128()?;
        visitor.visit_i128(de_zig_zag_i128(v))
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.input.read_u8()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u16()?;
        visitor.visit_u16(v)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u32()?;
        visitor.visit_u32(v)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u64()?;
        visitor.visit_u64(v)
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let v = self.read_varint_u128()?;
        visitor.visit_u128(v)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let bytes = self.input.read(4)?;
        visitor.visit_f32(f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap())))
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let bytes = self.input.read(8)?;
        visitor.visit_f64(f64::from_bits(u64::from_le_bytes(bytes.try_into().unwrap())))
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // A field that was a char may since have become a string. A reader
        // still expecting a char takes its first character rather than
        // failing, in the same spirit as skipping a field it does not know:
        // a peer that has not been updated stays able to read the message.
        let character = if self.takes_block() {
            if self.input.block_exhausted()? {
                return Err(Error::BadChar);
            }

            // Only the character itself is read. Anything after it is left to
            // be skipped when the block ends, so a field widened to a long
            // string costs a reader that only wants the first character
            // nothing to ignore.
            let lead = self.input.read_u8()?;
            let following = utf8_following_bytes(lead).ok_or(Error::BadChar)?;

            let mut buf = [lead, 0, 0, 0];
            for byte in buf[1..=following].iter_mut() {
                *byte = self.input.read_u8()?;
            }

            first_char(&buf[..=following])?
        } else {
            // Here the length belongs to the value rather than to a block, so
            // all of it has to be consumed to leave the reader in the right
            // place, however much of it is wanted.
            let sz = self.read_varint_usize()?;
            first_char(&self.input.read(sz)?)?
        };

        visitor.visit_char(character)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let bytes = self.read_len_prefixed()?;
        let str_sl = String::from_utf8(bytes).map_err(|_| Error::BadString)?;

        visitor.visit_string(str_sl)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let bytes = self.read_len_prefixed()?;
        visitor.visit_byte_buf(bytes)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // Mirrors `serialize_some`: the tag comes first and the value still
        // reaches the end of the block.
        let owns_block = self.takes_block();
        match self.input.read_u8()? {
            NONE => visitor.visit_none(),
            SOME => self.recurse(owns_block, |de| visitor.visit_some(de)),
            _ => Err(Error::BadOption),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let owns_block = self.takes_block();

        if name != Recoverable::NEWTYPE_NAME {
            return self.recurse(owns_block, |de| visitor.visit_newtype_struct(de));
        }

        // A recoverable value is bounded by a block.
        let depth = self.input.depth();
        let block = !owns_block;
        if block {
            self.input.start_skippable();
        }

        let value_owns_block = if block { self.field_owns_block() } else { owns_block };
        let res = self.recurse(value_owns_block, |de| visitor.visit_newtype_struct(de));

        // A value that failed stopped wherever it was and the visitor has
        // turned the failure into a recovered value. Thus we still need
        // to discard blocks that have remained open and clear flags.
        self.input.pop_to(depth)?;
        self.owns_block = false;

        res
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let len = match self.read_varint_usize()? {
            SPECIAL_LEN => match self.read_varint_usize()? {
                SPECIAL_LEN => Some(SPECIAL_LEN),
                UNKNOWN_LEN => {
                    self.input.start_skippable();
                    None
                }
                _ => return Err(Error::BadLen),
            },
            len => Some(len),
        };

        let value = self.recurse(false, |de| visitor.visit_seq(SeqAccess { deserializer: de, len }))?;

        if len.is_none() {
            self.input.end_skippable()?;
        }

        Ok(value)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.recurse(false, |de| visitor.visit_seq(SeqAccess { deserializer: de, len: Some(len) }))
    }

    fn deserialize_tuple_struct<V>(self, _name: &'static str, len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let len = match self.read_varint_usize()? {
            SPECIAL_LEN => match self.read_varint_usize()? {
                SPECIAL_LEN => Some(SPECIAL_LEN),
                UNKNOWN_LEN => {
                    self.input.start_skippable();
                    None
                }
                _ => return Err(Error::BadLen),
            },
            len => Some(len),
        };

        let value = self.recurse(false, |de| visitor.visit_map(MapAccess { deserializer: de, len }))?;

        if len.is_none() {
            self.input.end_skippable()?;
        }

        Ok(value)
    }

    fn deserialize_struct<V>(
        self, _name: &'static str, fields: &'static [&'static str], visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // A struct filling a field's block leaves out its field count, since
        // the block says where the fields end. Never in `Slim`, whose fields
        // carry no identifier and may be empty.
        let len = if self.takes_block() { None } else { Some(self.read_varint_usize()?) };

        if WITH_IDENTS {
            if cfg!(postbag_fast_compile) {
                // Buffered path: eagerly buffer all field data and reorder to match
                // the expected field declaration order, then use `visit_seq`.
                // Produces significantly less monomorphized code at the cost of
                // buffering the entire struct payload in memory.
                self.recurse(false, |de| {
                    let access = BufferedFieldSeqAccess::<WITH_IDENTS>::new(de, fields, len)?;
                    visitor.visit_seq(access)
                })
            } else {
                // Streaming path (default): read field identifiers and values
                // directly from the wire using `visit_map` with skippable blocks.
                self.recurse(false, |de| visitor.visit_map(StructFieldAccess { deserializer: de, len }))
            }
        } else {
            let len = len.expect("slim structs always state their field count");
            self.input.start_skippable();
            let value = self.recurse(false, |de| visitor.visit_seq(StructSeqAccess { deserializer: de, len }))?;
            self.input.end_skippable()?;
            Ok(value)
        }
    }

    fn deserialize_enum<V>(
        self, _name: &'static str, _variants: &'static [&'static str], visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // The variant identifier comes first and its payload still reaches
        // the end of the block, mirroring `serialize_newtype_variant` and
        // `serialize_struct_variant`.
        let owns_block = self.takes_block();
        self.recurse(owns_block, |de| visitor.visit_enum(de))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        match self.read_identifier()? {
            (Cow::Borrowed(ident), _) => visitor.visit_str(ident),
            (Cow::Owned(ident), _) => visitor.visit_string(ident),
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

/// Reader for the payload of an enum variant.
pub struct EnumVariant<'a, 'b, R, const WITH_IDENTS: bool> {
    deserializer: &'a mut Deserializer<'b, R, WITH_IDENTS>,
    block: bool,
}

impl<'a, 'b, R: Read, const WITH_IDENTS: bool> EnumVariant<'a, 'b, R, WITH_IDENTS> {
    fn finish(self) -> Result<()> {
        self.deserializer.owns_block = false;

        if self.block {
            self.deserializer.input.end_skippable()?;
        }

        Ok(())
    }
}

impl<'a, 'b: 'a, R: Read, const WITH_IDENTS: bool> serde::de::VariantAccess<'b>
    for EnumVariant<'a, 'b, R, WITH_IDENTS>
{
    type Error = Error;

    #[inline(never)]
    fn unit_variant(self) -> Result<()> {
        // Nothing is read, so a payload that is there is skipped whole. This
        // is the path a `#[serde(other)]` fallback takes.
        self.finish()
    }

    #[inline(never)]
    fn newtype_variant_seed<V: DeserializeSeed<'b>>(self, seed: V) -> Result<V::Value> {
        let value = DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
        self.finish()?;
        Ok(value)
    }

    #[inline(never)]
    fn tuple_variant<V: Visitor<'b>>(self, len: usize, visitor: V) -> Result<V::Value> {
        let value = serde::de::Deserializer::deserialize_tuple(&mut *self.deserializer, len, visitor)?;
        self.finish()?;
        Ok(value)
    }

    #[inline(never)]
    fn struct_variant<V: Visitor<'b>>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value> {
        let value = serde::de::Deserializer::deserialize_struct(&mut *self.deserializer, "", fields, visitor)?;
        self.finish()?;
        Ok(value)
    }
}

impl<'a, 'de, R: Read, const WITH_IDENTS: bool> serde::de::EnumAccess<'de>
    for &'a mut Deserializer<'de, R, WITH_IDENTS>
{
    type Error = Error;
    type Variant = EnumVariant<'a, 'de, R, WITH_IDENTS>;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self::Variant)> {
        let owns_block = self.takes_block();

        let (v, block) = if WITH_IDENTS {
            let (ident, has_block) = self.read_identifier()?;
            let deserializer: StrDeserializer<Error> = ident.as_ref().into_deserializer();
            (DeserializeSeed::deserialize(seed, deserializer)?, has_block)
        } else {
            let varint = self.read_varint_u32()?;
            let deserializer: U32Deserializer<Error> = varint.into_deserializer();
            (DeserializeSeed::deserialize(seed, deserializer)?, false)
        };

        let opened = self.variant_is_tagged(owns_block);
        if opened {
            // Variant does not own surrounding block.
            if block {
                // And will be followed by its own block.
                self.input.start_skippable();
            } else {
                // And will not be followed by its own block, but deserializer code expects
                // a block it can close when done reading the contents; thus open an empty
                // block.
                self.input.start_empty_block();
            }
        }

        // The payload reaches the end of a block, whether the enclosing one or
        // the one just opened.
        self.owns_block = opened || owns_block;

        Ok((v, EnumVariant { deserializer: self, block: opened }))
    }
}

fn de_zig_zag_i16(n: u16) -> i16 {
    ((n >> 1) as i16) ^ (-((n & 0b1) as i16))
}

fn de_zig_zag_i32(n: u32) -> i32 {
    ((n >> 1) as i32) ^ (-((n & 0b1) as i32))
}

fn de_zig_zag_i64(n: u64) -> i64 {
    ((n >> 1) as i64) ^ (-((n & 0b1) as i64))
}

fn de_zig_zag_i128(n: u128) -> i128 {
    ((n >> 1) as i128) ^ (-((n & 0b1) as i128))
}
