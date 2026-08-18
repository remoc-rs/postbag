# Postbag Full Format 1.0

## 1. Scope

This document specifies version 1.0 of the Postbag Full binary data format.

Postbag encodes one schema-defined value as a byte sequence. The schema determines the
type and structure of every value; type information is not included in the encoded data.
Record fields and variant tags carry identifiers.

A document normally begins with a header stating the version of the data format, as
specified in Section 8.1. The format does not define transport framing, compression,
encryption, or checksums. The [Postbag Slim format](POSTBAG-SLIM.md) is outside the scope
of this document.

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are to be interpreted as
described in RFC 2119.

## 2. Conventions

- Byte values are written in hexadecimal, for example `0x7d`.
- Byte sequences are written as space-separated hexadecimal pairs.
- `varint(n)` denotes the unsigned variable-length encoding defined in Section 3.1.
- `ε` denotes an empty byte sequence.
- Fixed-width multi-byte values use little-endian byte order.
- Encoded values have no alignment or padding.

A value is **block-final** when it extends to the end of its enclosing block. This property
affects the encoding of strings, byte strings, records, and variants as specified below.

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

A string outside block-final position is encoded as:

```
varint(byte_length) <UTF-8 bytes>
```

A block-final string omits `byte_length` and occupies the remainder of the block.
The byte sequence MUST be valid UTF-8.

A character uses the string encoding. An encoder MUST encode exactly one Unicode scalar
value. A decoder reads the first Unicode scalar value and MUST reject an empty or invalid
encoding.

### 3.5 Byte strings

A byte string uses the same encoding as a string, without the UTF-8 requirement:

```
varint(byte_length) <bytes>
```

A block-final byte string omits `byte_length` and occupies the remainder of the block.

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

## 5. Identifiers

Identifiers name record fields and variants. Their first byte contains a 7-bit identifier
code and a block flag in the high bit.

| Bits | Name | Meaning |
| --- | --- | --- |
| `0x00`-`0x3f` | short name | The value is the UTF-8 byte length; the name follows |
| `0x40` | long name | A 64-bit varint byte length and the UTF-8 name follow |
| `0x41`-`0x7c` | numbered name | `_0` through `_59`; no name bytes follow |
| `0x7d`-`0x7f` | reserved | Invalid in version 1.0 |
| `0x80` | block flag | A block follows the identifier |

The block flag is combined with one of the 7-bit codes above. Identifier names MUST be
valid UTF-8.

An encoder:

- MUST use the numbered form only for `_0` through `_59`, written in canonical decimal
  notation;
- MUST use the short form for other names shorter than 64 bytes;
- MUST use the long form for other names of 64 bytes or more; and
- MUST leave the block flag clear on record field identifiers.

The numbered and textual forms of `_0` through `_59` identify the same names when decoding.
A decoder MUST reject a reserved identifier code.

## 6. Block-final position

The following values are block-final:

1. the value of every record field;
2. every variant payload, either in an enclosing block or in a block introduced by the
   variant tag; and
3. the value inside a block introduced for a recoverable value, as specified in
   Section 7.9.

Block-final position propagates through an optional value that is present, through a
transparent wrapper, and through a recoverable value that introduces no block of its own.
It does not propagate into tuples, sequences, maps, or tuple-variant fields.

Block-final position changes these encodings:

| Value | Not block-final | Block-final |
| --- | --- | --- |
| String, character, byte string | Length followed by bytes | Bytes to the end of the block |
| Record | Field count followed by fields | Fields to the end of the block |
| Non-empty variant payload | Tag followed by a block | Tag followed directly by the payload |

All other value encodings are independent of block-final position.

## 7. Composite values

### 7.1 Unit

A unit value occupies no bytes:

```
unit := ε
```

### 7.2 Optional values

```
option(T) := 00
           | 01 value(T)
```

`0x00` denotes absence. `0x01` denotes presence and is followed by the contained value.
When the option is block-final, a present value is also block-final. A decoder MUST reject
any other discriminant.

### 7.3 Transparent wrappers

A transparent wrapper contributes no bytes. Its contained value is encoded in its place
and inherits block-final position. A recoverable value, specified in Section 7.9, uses
transparent wrapping but adds a block unless an enclosing block already bounds it.

### 7.4 Tuples and fixed-size arrays

Tuple elements are encoded consecutively without a count or framing:

```
tuple(T1, ..., Tn) := value(T1, false) ... value(Tn, false)
```

The schema supplies the number and type of elements. No element is block-final.
Fixed-size arrays use the same encoding.

### 7.5 Records

A record consists of fields. Each field is an identifier followed by a block containing
its value:

```
field := identifier(block_flag = 0) block(value(T, true))

record := varint(field_count) field^field_count   ; not block-final
        | field*                                  ; block-final
```

The field count is the number of fields actually encoded. Fields MAY occur in any order.
A decoder identifies fields by name, not by position.

For a block-final record, fields continue until the enclosing block is exhausted.

For each field, a decoder MUST enter the field block before decoding its value and MUST
close the block afterward. Unknown fields are skipped by closing their blocks without
decoding their contents.

### 7.6 Variants

A variant consists of an identifier tag and an optional payload:

```
variant := identifier(block_flag = 0) payload                  ; block-final
         | identifier(block_flag = 0)                          ; empty payload
         | identifier(block_flag = 1) block(payload)           ; otherwise
```

The payload is always block-final.

| Variant shape | Payload |
| --- | --- |
| Unit | Empty |
| Single value | The value |
| Tuple | Consecutive non-block-final fields, without a count |
| Record | Record fields without a field count |

An encoder MUST omit a payload block for a unit variant or for a tuple or record variant
with no fields. Otherwise, an encoder MUST set the block flag and add a payload block when
the variant itself is not block-final.

When a non-block-final variant has a clear block flag, a decoder MUST treat its payload as
an empty block. An unknown variant MAY be rejected or handled as a designated fallback;
its payload is skipped when its block is closed.

### 7.7 Sequences

A counted sequence is encoded as:

```
sequence(T) := count(n) value(T, false)^n
```

No element is block-final. The count encoding reserves the one-byte value `0x7d`:

```
count(n) := varint(n)   ; n != 125
          | 7d 7d       ; n = 125
```

An uncounted sequence is encoded as:

```
sequence(T) := 7d 00 block((01 value(T, false))* 00)
```

Within the block, `0x01` announces an element and `0x00` terminates the sequence. A decoder
MUST reject any other announcement byte.

An encoder SHOULD use the counted form when the element count is available before
encoding.

### 7.8 Maps

A counted map is encoded as:

```
map(K, V) := count(n) (value(K, false) value(V, false))^n
```

The count is the number of key-value pairs and uses the encoding from Section 7.7. Keys
and values are not block-final.

An uncounted map is encoded as:

```
map(K, V) := 7d 00 block((01 value(K, false) value(V, false))* 00)
```

Within the block, `0x01` announces a key-value pair and `0x00` terminates the map. A decoder
MUST reject any other announcement byte.

### 7.9 Recoverable values

A recoverable value is bounded so that, if decoding fails, a decoder can skip it and
continue with the values that follow:

```
recoverable(T) := value(T, true)          ; block-final
                | block(value(T, true))   ; otherwise
```

In block-final position, the enclosing block already bounds the value, so no block is
introduced and the encoding is the one the value has without the wrapper. Everywhere else
a block is introduced and the value is block-final within it.

The schema identifies recoverable values. The encoding includes no marker, so an encoder
and a decoder MUST agree on which values are recoverable.

A decoder that fails to decode a recoverable value MUST close every block opened while
decoding it, through the block that bounds the value. The decoder MAY then substitute
another value. If the failure leaves the position of the following data unknown, as when
the underlying byte source fails, the decoder MUST report the failure and MUST NOT
substitute a value.

## 8. Document encoding

### 8.1 Header

A document header is two bytes and states the version of the data format and whether
identifiers are serialized:

```
header := ba flags
```

The bits of `flags` are:

| Bits | Name | Value |
| --- | --- | --- |
| 7-5 | fixed | `0b101` |
| 4 | identifiers | `1` in this format |
| 3-0 | version | `1` for version 1.0 |

In version 1.0 of this format the header is therefore `ba b1`.

Version `0` identifies Postbag 0.4 and earlier, which has no header, and version `15` is
reserved to introduce an extended version encoding. An encoder MUST NOT write either as
the version.

A decoder expecting a header MUST reject a document whose first byte is not `0xba`, whose
fixed bits are not `0b101`, whose version it does not implement, or whose identifiers bit
does not match the format it decodes.

### 8.2 Root value

A Postbag document encodes exactly one value and normally includes a header:

```
document := header value(root_type, false)
          | value(root_type, false)          ; header omission agreed out of band
```

The root value is not block-final. No byte length or terminator is included.

An encoder SHOULD write the header. It MAY omit the header when the format and version are
agreed out of band. A decoder MUST know beforehand whether a header is present.

## 9. Grammar summary

```
document              := header value(root_type, false)
                       | value(root_type, false)                 ; agreed header omission

header                := ba flags                              ; flags = 0b101 <idents:1>
                                                               ;               <version:4>
                                                               ; ba b1 in version 1.0

value(T, final):
  boolean             := 00 | 01
  uint8               := <byte>
  int8                := <two's-complement byte>
  uint16..uint128     := varint(v)
  int16..int128       := varint(zigzag(v))
  float32             := <4 IEEE 754 bytes, little-endian>
  float64             := <8 IEEE 754 bytes, little-endian>
  string              := final ? <UTF-8 bytes to end of block>
                               : varint(length) <UTF-8 bytes>
  character           := string
  bytes               := final ? <bytes to end of block>
                               : varint(length) <bytes>
  unit                := ε
  option(T)           := 00 | 01 value(T, final)
  transparent(T)      := value(T, final)
  recoverable(T)      := final ? value(T, true)
                               : block(value(T, true))
  tuple(T1..Tn)       := value(T1, false) ... value(Tn, false)
  array(T, n)         := value(T, false)^n
  sequence(T)         := count(n) value(T, false)^n
                       | 7d 00 block((01 value(T, false))* 00)
  map(K, V)           := count(n) (value(K, false) value(V, false))^n
                       | 7d 00 block((01 value(K, false) value(V, false))* 00)
  record              := final ? field*
                               : varint(n) field^n
  variant             := identifier(0) payload                 ; final
                       | identifier(0)                         ; empty payload
                       | identifier(0x80) block(payload)       ; otherwise

field                 := identifier(0) block(value(T, true))

payload:
  unit                := ε
  single(T)           := value(T, true)
  tuple(T1..Tn)       := value(T1, false) ... value(Tn, false)
  record              := field*

identifier(flag)      := (flag | length) <UTF-8 bytes>         ; length <= 63
                       | (flag | 0x40) varint(length) <UTF-8 bytes>
                                                                  ; length >= 64
                       | (flag | code)                          ; 0x41 <= code <= 0x7c

count(n)              := varint(n)                             ; n != 125
                       | 7d 7d                                 ; n = 125

block                 := (varint16(65535) <65535 bytes>)*
                         varint16(length) <length bytes>        ; length < 65535
```
