# Postbag Slim Format 1.0

## 1. Scope

This document specifies version 1.0 of the Postbag Slim binary data format.

Postbag encodes one schema-defined value as a byte sequence. The schema determines the
type and structure of every value; type information, record field identifiers, and
variant names are not included in the encoded data. Record fields and variants are
identified by their positions in the schema.

The format does not define transport framing, a version marker, compression, encryption,
or checksums. The [Postbag Full format](POSTBAG-FULL.md) is outside the scope of this
document.

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are to be interpreted as
described in RFC 2119.

## 2. Conventions

- Byte values are written in hexadecimal, for example `0x7d`.
- Byte sequences are written as space-separated hexadecimal pairs.
- `varint(n)` denotes the unsigned variable-length encoding defined in Section 3.1.
- `ε` denotes an empty byte sequence.
- Fixed-width multi-byte values use little-endian byte order.
- Encoded values have no alignment or padding.

## 3. Primitive encodings

### 3.1 Unsigned integers

Unsigned integers wider than 8 bits use unsigned LEB128. Each byte contains seven value
bits, least-significant group first. The high bit is set when another byte follows.

```
byte = (value & 0x7f) | (more ? 0x80 : 0x00)
```

Examples:

| Value | Encoding |
| ---: | --- |
| 0 | `00` |
| 127 | `7f` |
| 128 | `80 01` |
| 300 | `ac 02` |

An encoder MUST use the shortest representation. A decoder MUST reject an encoding that
does not terminate within the target width or uses bits outside that width.

| Width | Maximum bytes | Maximum final byte |
| ---: | ---: | ---: |
| 16 bits | 3 | `0x03` |
| 32 bits | 5 | `0x0f` |
| 64 bits | 10 | `0x01` |
| 128 bits | 19 | `0x03` |

Lengths and counts use 64-bit varints unless stated otherwise.

### 3.2 Signed integers

Signed integers wider than 8 bits are zigzag-mapped to an unsigned integer of the same
width and then encoded as a varint.

```
zigzag(n)   = (n << 1) ^ (n >> (width - 1))
unzigzag(u) = (u >> 1) ^ -(u & 1)
```

Thus `0`, `-1`, `1`, `-2`, and `2` map to `0`, `1`, `2`, `3`, and `4`.

### 3.3 Fixed-width values

| Value type | Encoding |
| --- | --- |
| Boolean | `0x00` for false or `0x01` for true |
| 8-bit unsigned integer | One byte |
| 8-bit signed integer | One two's-complement byte |
| 32-bit float | IEEE 754 binary32 bit pattern, 4 bytes |
| 64-bit float | IEEE 754 binary64 bit pattern, 8 bytes |

A decoder MUST reject any other Boolean value. Floating-point bit patterns are preserved
without normalization.

### 3.4 Strings and characters

A string is encoded as:

```
varint(byte_length) <UTF-8 bytes>
```

The byte sequence MUST be valid UTF-8.

A character uses the string encoding. An encoder MUST encode exactly one Unicode scalar
value. A decoder reads the first Unicode scalar value and MUST reject an empty or invalid
encoding.

### 3.5 Byte strings

A byte string uses the same encoding as a string, without the UTF-8 requirement:

```
varint(byte_length) <bytes>
```

## 4. Blocks

A block is a self-delimiting byte sequence divided into chunks:

```
block := continued-chunk* final-chunk

continued-chunk := varint16(65535) <65535 bytes>
final-chunk     := varint16(length) <length bytes>   ; length < 65535
```

`varint16` is the 16-bit varint encoding from Section 3.1. A chunk length of 65535 means
that another chunk follows. A shorter chunk terminates the block. A content length that is
an exact multiple of 65535 is therefore followed by a zero-length final chunk.

Blocks may be nested. The framing bytes of an inner block count as content of its outer
block.

A decoder MUST reject truncated blocks and reads that cross the boundary of the innermost
block. When closing a block, a decoder MUST consume or skip all remaining chunks and
content in that block.

Blocks occur around every record body and around the body of a sequence or map whose
length is not known before encoding.

## 5. Composite values

### 5.1 Unit

A unit value occupies no bytes:

```
unit := ε
```

### 5.2 Optional values

```
option(T) := 00
           | 01 value(T)
```

`0x00` denotes absence. `0x01` denotes presence and is followed by the contained value.
A decoder MUST reject any other discriminant.

### 5.3 Transparent wrappers

A transparent wrapper contributes no bytes. Its contained value is encoded in its place.

### 5.4 Tuples and fixed-size arrays

Tuple elements are encoded consecutively without a count or framing:

```
tuple(T1, ..., Tn) := value(T1) ... value(Tn)
```

The schema supplies the number and type of elements. Fixed-size arrays use the same
encoding.

### 5.5 Records

A record contains a field count followed by one block holding the field values:

```
record := varint(field_count) block(value(T1) ... value(Tn))
```

The field count is the number of fields actually encoded. Field values occur in schema
order and have no identifiers or individual framing.

A decoder MUST enter the record block before decoding fields and MUST close it afterward.
Closing the block skips any remaining bytes. Because individual fields are not
self-describing or separately framed, a decoder cannot skip an unknown field while
continuing with later fields.

An empty record is encoded as a zero field count followed by an empty block:

```
00 00
```

### 5.6 Variants

A variant begins with its zero-based index, encoded as a 32-bit varint:

```
variant := varint32(variant_index) payload
```

Variant names are not encoded.

| Variant shape | Payload |
| --- | --- |
| Unit | Empty |
| Single value | The value |
| Tuple | Consecutive fields, without a count or block |
| Record | A record as defined in Section 5.5 |

No block is added around a unit, single-value, or tuple payload. A record payload includes
the field count and record block required by the record encoding.

### 5.7 Sequences

A counted sequence is encoded as:

```
sequence(T) := count(n) value(T)^n
```

The count encoding reserves the one-byte value `0x7d`:

```
count(n) := varint(n)   ; n != 125
          | 7d 7d       ; n = 125
```

An uncounted sequence is encoded as:

```
sequence(T) := 7d 00 block((01 value(T))* 00)
```

Within the block, `0x01` announces an element and `0x00` terminates the sequence. A decoder
MUST reject any other announcement byte.

An encoder SHOULD use the counted form when the element count is available before
encoding.

### 5.8 Maps

A counted map is encoded as:

```
map(K, V) := count(n) (value(K) value(V))^n
```

The count is the number of key-value pairs and uses the encoding from Section 5.7.

An uncounted map is encoded as:

```
map(K, V) := 7d 00 block((01 value(K) value(V))* 00)
```

Within the block, `0x01` announces a key-value pair and `0x00` terminates the map. A decoder
MUST reject any other announcement byte.

## 6. Document encoding

A Postbag document is the encoding of exactly one value:

```
document := value(root_type)
```

No header, version marker, byte length, or terminator is included.

## 7. Grammar summary

```
value(T):
  boolean             := 00 | 01
  uint8               := <byte>
  int8                := <two's-complement byte>
  uint16..uint128     := varint(v)
  int16..int128       := varint(zigzag(v))
  float32             := <4 IEEE 754 bytes, little-endian>
  float64             := <8 IEEE 754 bytes, little-endian>
  string              := varint(length) <UTF-8 bytes>
  character           := string
  bytes               := varint(length) <bytes>
  unit                := ε
  option(T)           := 00 | 01 value(T)
  transparent(T)      := value(T)
  tuple(T1..Tn)       := value(T1) ... value(Tn)
  array(T, n)         := value(T)^n
  sequence(T)         := count(n) value(T)^n
                       | 7d 00 block((01 value(T))* 00)
  map(K, V)           := count(n) (value(K) value(V))^n
                       | 7d 00 block((01 value(K) value(V))* 00)
  record              := varint(n) block(value(T1) ... value(Tn))
  variant             := varint32(index) payload

payload:
  unit                := ε
  single(T)           := value(T)
  tuple(T1..Tn)       := value(T1) ... value(Tn)
  record              := varint(n) block(value(T1) ... value(Tn))

count(n)              := varint(n)   ; n != 125
                       | 7d 7d       ; n = 125

block                 := (varint16(65535) <65535 bytes>)*
                         varint16(length) <length bytes>   ; length < 65535
```
