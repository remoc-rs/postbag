use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use std::{
    collections::BTreeMap,
    fmt::{Debug, Write},
    io::ErrorKind,
    marker::PhantomData,
};

use postbag::{
    Error,
    cfg::{Cfg, Full, Slim},
    deserialize, serialize,
};

/// Performs serialization followed by deserialization and checks that the
/// deserialized value is unchanged.
#[track_caller]
pub fn loopback_with_cfg<T, const WITH_IDENTS: bool>(value: &T, cfg: Cfg<WITH_IDENTS>)
where
    T: Serialize + DeserializeOwned + Debug + Eq,
{
    let mut serialized = Vec::new();
    serialize(cfg, &mut serialized, value).expect("serialization failed");
    println!("{serialized:02x?}");
    dbg!(serialized.len());

    let deserialized: T = deserialize(cfg, serialized.as_slice()).expect("deserialization failed");

    assert_eq!(*value, deserialized, "deserialized value does not match original value");
}

/// Performs serialization followed by deserialization and checks that the
/// deserialized value is unchanged.
#[track_caller]
pub fn loopback<T>(value: T)
where
    T: Serialize + DeserializeOwned + Debug + Eq,
{
    println!("Testing with field names");
    loopback_with_cfg(&value, Full::new());

    println!("Testing without field names");
    loopback_with_cfg(&value, Slim::new());
}

// =============================================================================
// Primitive Types Tests
// =============================================================================

#[test]
fn primitives_unsigned_integers() {
    loopback(0x05u8);
    loopback(0xA5C7u16);
    loopback(0xCDAB3412u32);
    loopback(0x1234_5678_90AB_CDEFu64);
    loopback(0x1234_5678_90AB_CDEF_1234_5678_90AB_CDEFu128);
}

#[test]
fn primitives_signed_integers() {
    loopback(32767i16); // positive edge
    loopback(-32768i16); // negative edge
    loopback(-19490127978232325886905073712831_i128); // large negative i128
}

#[test]
fn primitives_booleans() {
    loopback(false);
    loopback(true);
}

#[test]
fn primitives_characters() {
    loopback('a'); // simple char
    loopback('z'); // ASCII char
    loopback('¢'); // Latin char
    loopback('𐍈'); // Gothic char
    loopback('🥺'); // Emoji char
}

#[test]
fn primitives_unit_type() {
    loopback(());
}

// =============================================================================
// Struct and Newtype Tests
// =============================================================================

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct BasicU8S {
    st: u16,
    ei: u8,
    ote: u128,
    sf: u64,
    tt: u32,
}

#[test]
fn structs_basic() {
    let data = BasicU8S {
        st: 0xABCD,
        ei: 0xFE,
        ote: 0x1234_4321_ABCD_DCBA_1234_4321_ABCD_DCBA,
        sf: 0x1234_4321_ABCD_DCBA,
        tt: 0xACAC_ACAC,
    };
    loopback(data);
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct NewTypeStruct(u32);

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct TupleStruct((u8, u16));

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct DualTupleStruct(u8, u16);

#[test]
fn structs_variants() {
    loopback(NewTypeStruct(5));
    loopback(TupleStruct((0xA0, 0x1234)));
    loopback(DualTupleStruct(0xA0, 0x1234));
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct RefStruct {
    bytes: std::vec::Vec<u8>,
    str_s: std::string::String,
}

#[test]
fn structs_with_references() {
    let message = "hElLo";
    let bytes = [0x01, 0x10, 0x02, 0x20];
    loopback(RefStruct { bytes: bytes.to_vec(), str_s: message.to_string() });
}

// =============================================================================
// Enum Tests
// =============================================================================

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum BasicEnum {
    Bib,
    Bim,
    Bap,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct EnumStruct {
    eight: u8,
    sixt: u16,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum DataEnum {
    Bib(u16),
    Bim(u64),
    Bap(u8),
    Kim(EnumStruct),
    Chi { a: u8, b: u32 },
    Sho(u16, u8),
}

#[test]
fn enums_basic() {
    loopback(BasicEnum::Bim);
}

#[test]
fn enums_with_data() {
    loopback(DataEnum::Bim(u64::MAX));
    loopback(DataEnum::Bib(u16::MAX));
    loopback(DataEnum::Bap(u8::MAX));
    loopback(DataEnum::Kim(EnumStruct { eight: 0xF0, sixt: 0xACAC }));
    loopback(DataEnum::Chi { a: 0x0F, b: 0xC7C7C7C7 });
    loopback(DataEnum::Sho(0x6969, 0x07));
}

// =============================================================================
// Nested Structure Tests
// =============================================================================

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct InnerStruct {
    id: u32,
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct OuterStruct {
    inner: InnerStruct,
    metadata: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct DeeplyNestedStruct {
    level1: OuterStruct,
    extra: u16,
}

#[test]
fn nested_structs() {
    let inner = InnerStruct { id: 42, name: "inner".to_string() };

    let outer = OuterStruct { inner, metadata: vec![1, 2, 3, 4] };

    loopback(outer);

    // Test deeply nested structs
    let deeply_nested = DeeplyNestedStruct {
        level1: OuterStruct {
            inner: InnerStruct { id: 999, name: "deep".to_string() },
            metadata: vec![0xFF, 0xAA, 0x55],
        },
        extra: 0x1234,
    };

    loopback(deeply_nested);
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
enum InnerEnum {
    Alpha(u8),
    Beta { x: u16, y: u16 },
    Gamma,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
enum OuterEnum {
    First(InnerEnum),
    Second { inner: InnerEnum, data: u32 },
    Third(InnerEnum, InnerEnum),
}

#[test]
fn nested_enums() {
    loopback(OuterEnum::First(InnerEnum::Alpha(42)));
    loopback(OuterEnum::First(InnerEnum::Beta { x: 100, y: 200 }));
    loopback(OuterEnum::First(InnerEnum::Gamma));

    loopback(OuterEnum::Second { inner: InnerEnum::Alpha(0xFF), data: 0xDEADBEEF });

    loopback(OuterEnum::Third(InnerEnum::Beta { x: 1, y: 2 }, InnerEnum::Alpha(99)));
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct StructWithEnum {
    status: InnerEnum,
    count: u64,
    nested_outer: OuterEnum,
}

#[test]
fn structs_with_nested_enums() {
    let data = StructWithEnum {
        status: InnerEnum::Beta { x: 10, y: 20 },
        count: 1000,
        nested_outer: OuterEnum::Second { inner: InnerEnum::Gamma, data: 0x12345678 },
    };

    loopback(data);

    // Test with different enum variants
    let data2 = StructWithEnum {
        status: InnerEnum::Alpha(127),
        count: u64::MAX,
        nested_outer: OuterEnum::Third(InnerEnum::Alpha(1), InnerEnum::Beta { x: u16::MAX, y: 0 }),
    };

    loopback(data2);
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
enum EnumWithStruct {
    Simple(u8),
    WithStruct(InnerStruct),
    WithOuter { outer: OuterStruct, flag: bool },
    Complex(InnerStruct, OuterStruct, u32),
}

#[test]
fn enums_with_nested_structs() {
    loopback(EnumWithStruct::Simple(42));

    loopback(EnumWithStruct::WithStruct(InnerStruct { id: 123, name: "test".to_string() }));

    loopback(EnumWithStruct::WithOuter {
        outer: OuterStruct {
            inner: InnerStruct { id: 456, name: "nested".to_string() },
            metadata: vec![0x01, 0x02, 0x03],
        },
        flag: true,
    });

    loopback(EnumWithStruct::Complex(
        InnerStruct { id: 789, name: "complex1".to_string() },
        OuterStruct {
            inner: InnerStruct { id: 101112, name: "complex2".to_string() },
            metadata: vec![0xDE, 0xAD, 0xBE, 0xEF],
        },
        0xCAFEBABE,
    ));
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct VeryComplexNested {
    enum_field: EnumWithStruct,
    struct_field: StructWithEnum,
    optional: Option<OuterEnum>,
    list: Vec<InnerEnum>,
}

#[test]
fn complex_nested_combinations() {
    let complex = VeryComplexNested {
        enum_field: EnumWithStruct::WithOuter {
            outer: OuterStruct {
                inner: InnerStruct { id: 1, name: "first".to_string() },
                metadata: vec![1, 2, 3],
            },
            flag: false,
        },
        struct_field: StructWithEnum {
            status: InnerEnum::Alpha(255),
            count: 9999,
            nested_outer: OuterEnum::First(InnerEnum::Gamma),
        },
        optional: Some(OuterEnum::Third(InnerEnum::Beta { x: 50, y: 100 }, InnerEnum::Alpha(200))),
        list: vec![InnerEnum::Gamma, InnerEnum::Alpha(1), InnerEnum::Beta { x: 2, y: 3 }],
    };

    loopback(complex);

    // Test with None optional
    let complex_none = VeryComplexNested {
        enum_field: EnumWithStruct::Simple(0),
        struct_field: StructWithEnum {
            status: InnerEnum::Gamma,
            count: 0,
            nested_outer: OuterEnum::First(InnerEnum::Alpha(1)),
        },
        optional: None,
        list: vec![],
    };

    loopback(complex_none);
}

#[test]
fn long_struct_fields() {
    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct LongFields {
        n123456789n123456789n123456789n123456789n123456789n123456789n123456789: String,
    }

    loopback(LongFields {
        n123456789n123456789n123456789n123456789n123456789n123456789n123456789: "abc".to_string(),
    });
}

#[test]
fn id_struct_fields() {
    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct IdFields {
        #[serde(rename = "_0")]
        my_long_field1: u8,
        #[serde(rename = "_2")]
        my_long_field2: u8,
        #[serde(rename = "_59")]
        my_long_field3: u8,
        #[serde(rename = "_60")]
        my_long_field4: u8,
    }

    loopback(IdFields { my_long_field1: 1, my_long_field2: 2, my_long_field3: 3, my_long_field4: 4 });
}

#[test]
fn id_enum_fields() {
    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    enum IdEnum {
        #[serde(rename = "_0")]
        MyLongVariant1,
        #[serde(rename = "_1")]
        MyLongVariant2(),
        #[serde(rename = "_2")]
        MyLongVariant3(u8),
        #[serde(rename = "_3")]
        MyLongVariant4((u8, u8)),
        #[serde(rename = "_4")]
        MyLongVariant5 {
            #[serde(rename = "_0")]
            long_field_name_a: u8,
            #[serde(rename = "_1")]
            long_field_name_b: u8,
        },
    }

    loopback(IdEnum::MyLongVariant1);
    loopback(IdEnum::MyLongVariant2());
    loopback(IdEnum::MyLongVariant3(1));
    loopback(IdEnum::MyLongVariant4((2, 3)));
    loopback(IdEnum::MyLongVariant5 { long_field_name_a: 4, long_field_name_b: 4 });
}

// =============================================================================
// Collection Tests
// =============================================================================

#[test]
fn collections_strings() {
    let input: &str = "hello, postbag!";
    loopback(input.to_string());

    let mut input: String = String::new();
    for _ in 0..256 {
        write!(&mut input, "abcd").unwrap();
    }
    loopback(input);

    // Additional string test
    let mut input: String = String::new();
    write!(&mut input, "helLO!").unwrap();
    loopback(input);
}

#[test]
fn collections_byte_slices_and_vecs() {
    let input: &[u8] = &[1u8, 2, 3, 4, 5, 6, 7, 8];
    loopback(input.to_vec());

    let mut input: Vec<u8> = Vec::new();
    for i in 0..1024 {
        input.push((i & 0xFF) as u8);
    }
    loopback(input);

    // Additional vec tests
    let mut input: Vec<u8> = Vec::new();
    input.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
    loopback(input);

    let input = vec![255, 255, 255, 0, 0, 0, 0, 0];
    loopback(input);
}

#[test]
fn collections_seq_special() {
    loopback(vec![1; 0]);
    loopback(vec![1; 124]);
    loopback(vec![1; 125]);
    loopback(vec![1; 126]);
}

#[test]
fn collections_maps() {
    let mut input: BTreeMap<u8, u8> = BTreeMap::new();
    input.insert(0x01, 0x05);
    input.insert(0x02, 0x06);
    input.insert(0x03, 0x07);
    input.insert(0x04, 0x08);
    loopback(input);
}

#[test]
fn collections_maps_special() {
    for len in [0, 124, 125, 126] {
        let mut input: BTreeMap<u8, u8> = BTreeMap::new();
        for i in 0..len {
            input.insert(i, i);
        }
        loopback(input);
    }
}

#[test]
fn collections_cstring() {
    // CString (uses serialize_bytes/deserialize_byte_buf)
    loopback(std::ffi::CString::new("heLlo").unwrap());
}

// =============================================================================
// Tuple Tests
// =============================================================================

#[test]
fn tuples_basic() {
    loopback((1u8, 10u32, "Hello!".to_string()));
    loopback((0x12u8, 0xC7A5u16));
}

// =============================================================================
// Bytes and Special Serialization Tests
// =============================================================================

#[derive(Debug, Eq, PartialEq)]
pub struct ByteSliceStruct {
    bytes: std::vec::Vec<u8>,
}

impl Serialize for ByteSliceStruct {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialization is generic for all slice types, so the default serialization of byte
        // slices does not use `Serializer::serialize_bytes`.
        serializer.serialize_bytes(&self.bytes)
    }
}

impl<'de> Deserialize<'de> for ByteSliceStruct {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialization of byte slices is specialized for byte slices, so the default
        // deserialization will call `Deserializer::deserialize_bytes`.
        Ok(Self { bytes: Deserialize::deserialize(deserializer)? })
    }
}

#[test]
fn bytes_arrays_and_custom_serialization() {
    loopback([0u8; 32]);
    loopback(vec![0u8; 32]);
    loopback(ByteSliceStruct { bytes: vec![0u8; 32] });
}

// =============================================================================
// Zero-Sized Type Tests
// =============================================================================

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ZSTStruct;

#[test]
fn zero_sized_types() {
    let input = vec![ZSTStruct, ZSTStruct, ZSTStruct];
    loopback(input);

    let input = vec![ZSTStruct, ZSTStruct, ZSTStruct, ZSTStruct];
    loopback(input);
}

// =============================================================================
// Unknown Length Sequence Tests
// =============================================================================

/// A custom sequence type that serializes with unknown length (None)
#[derive(Debug, Eq, PartialEq)]
pub struct UnknownLengthSeq<T> {
    items: Vec<T>,
}

impl<T> UnknownLengthSeq<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self { items }
    }
}

impl<T> Serialize for UnknownLengthSeq<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeSeq;
        // Serialize with unknown length (None) instead of Some(self.items.len())
        let mut seq = serializer.serialize_seq(None)?;
        for item in &self.items {
            seq.serialize_element(item)?;
        }
        seq.end()
    }
}

impl<'de, T> Deserialize<'de> for UnknownLengthSeq<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{SeqAccess, Visitor};

        struct UnknownLengthSeqVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for UnknownLengthSeqVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = UnknownLengthSeq<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of unknown length")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(UnknownLengthSeq::new(items))
            }
        }

        deserializer.deserialize_seq(UnknownLengthSeqVisitor(PhantomData))
    }
}

#[test]
fn sequences_unknown_length() {
    // Test empty sequence
    let empty_seq = UnknownLengthSeq::new(vec![] as Vec<u32>);
    println!("Testing empty sequence");
    loopback(empty_seq);

    // Test sequence with single element
    let single_seq = UnknownLengthSeq::new(vec![42u32]);
    loopback(single_seq);

    // Test sequence with multiple elements
    let multi_seq = UnknownLengthSeq::new(vec![1u32, 2, 3, 4, 5]);
    loopback(multi_seq);

    // Test sequence with different data types
    let string_seq = UnknownLengthSeq::new(vec!["hello".to_string(), "world".to_string(), "postbag".to_string()]);
    loopback(string_seq);

    // Test sequence with complex nested structures
    let complex_seq = UnknownLengthSeq::new(vec![
        BasicU8S {
            st: 0x1234,
            ei: 0xAB,
            ote: 0x5678_9ABC_DEF0_1234_5678_9ABC_DEF0_1234,
            sf: 0x9876_5432_10AB_CDEF,
            tt: 0xDEAD_BEEF,
        },
        BasicU8S {
            st: 0x5678,
            ei: 0xCD,
            ote: 0x1111_2222_3333_4444_5555_6666_7777_8888,
            sf: 0xFFFF_FFFF_FFFF_FFFF,
            tt: 0xCAFE_BABE,
        },
    ]);
    loopback(complex_seq);

    // Test sequence with large number of elements to test boundary conditions
    let large_seq = UnknownLengthSeq::new((0..1000u16).collect());
    loopback(large_seq);

    // Test sequence with boundary length values (around SPECIAL_LEN = 125)
    for len in [123, 124, 125, 126, 127] {
        let boundary_seq = UnknownLengthSeq::new((0..len as u8).collect());
        loopback(boundary_seq);
    }
}

// =============================================================================
// Unknown Length Map Tests
// =============================================================================

/// A custom map type that serializes with unknown length (None)
#[derive(Debug, Eq, PartialEq)]
pub struct UnknownLengthMap<K, V> {
    items: BTreeMap<K, V>,
}

impl<K, V> UnknownLengthMap<K, V> {
    pub fn new(items: BTreeMap<K, V>) -> Self {
        Self { items }
    }
}

impl<K, V> Serialize for UnknownLengthMap<K, V>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        // Serialize with unknown length (None) instead of Some(self.items.len())
        let mut map = serializer.serialize_map(None)?;
        for (key, value) in &self.items {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de, K, V> Deserialize<'de> for UnknownLengthMap<K, V>
where
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};

        struct UnknownLengthMapVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K, V> Visitor<'de> for UnknownLengthMapVisitor<K, V>
        where
            K: Deserialize<'de> + Ord,
            V: Deserialize<'de>,
        {
            type Value = UnknownLengthMap<K, V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map of unknown length")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut items = BTreeMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    items.insert(key, value);
                }
                Ok(UnknownLengthMap::new(items))
            }
        }

        deserializer.deserialize_map(UnknownLengthMapVisitor(PhantomData))
    }
}

#[test]
fn maps_unknown_length() {
    // Test empty map
    let empty_map = UnknownLengthMap::new(BTreeMap::<u32, u32>::new());
    println!("Testing empty map");
    loopback(empty_map);

    // Test map with single entry
    let mut single_map_data = BTreeMap::new();
    single_map_data.insert(42u32, 84u32);
    let single_map = UnknownLengthMap::new(single_map_data);
    loopback(single_map);

    // Test map with multiple entries
    let mut multi_map_data = BTreeMap::new();
    for i in 1..=5 {
        multi_map_data.insert(i, i * 2);
    }
    let multi_map = UnknownLengthMap::new(multi_map_data);
    loopback(multi_map);

    // Test map with string keys and values
    let mut string_map_data = BTreeMap::new();
    string_map_data.insert("hello".to_string(), "world".to_string());
    string_map_data.insert("postbag".to_string(), "serde".to_string());
    string_map_data.insert("rust".to_string(), "language".to_string());
    let string_map = UnknownLengthMap::new(string_map_data);
    loopback(string_map);

    // Test map with complex nested structures as values
    let mut complex_map_data = BTreeMap::new();
    complex_map_data.insert(
        1u8,
        BasicU8S {
            st: 0x1234,
            ei: 0xAB,
            ote: 0x5678_9ABC_DEF0_1234_5678_9ABC_DEF0_1234,
            sf: 0x9876_5432_10AB_CDEF,
            tt: 0xDEAD_BEEF,
        },
    );
    complex_map_data.insert(
        2u8,
        BasicU8S {
            st: 0x5678,
            ei: 0xCD,
            ote: 0x1111_2222_3333_4444_5555_6666_7777_8888,
            sf: 0xFFFF_FFFF_FFFF_FFFF,
            tt: 0xCAFE_BABE,
        },
    );
    let complex_map = UnknownLengthMap::new(complex_map_data);
    loopback(complex_map);

    // Test map with large number of entries to test boundary conditions
    let mut large_map_data = BTreeMap::new();
    for i in 0..1000u16 {
        large_map_data.insert(i, i * 3);
    }
    let large_map = UnknownLengthMap::new(large_map_data);
    loopback(large_map);

    // Test map with boundary length values (around SPECIAL_LEN = 125)
    for len in [123, 124, 125, 126, 127] {
        let mut boundary_map_data = BTreeMap::new();
        for i in 0..len {
            boundary_map_data.insert(i as u8, i as u8);
        }
        let boundary_map = UnknownLengthMap::new(boundary_map_data);
        loopback(boundary_map);
    }

    // Test map with mixed key-value types
    let mut mixed_map_data = BTreeMap::new();
    mixed_map_data.insert(10u16, "ten".to_string());
    mixed_map_data.insert(20u16, "twenty".to_string());
    mixed_map_data.insert(30u16, "thirty".to_string());
    let mixed_map = UnknownLengthMap::new(mixed_map_data);
    loopback(mixed_map);
}

// =============================================================================
// Error Handling and Edge Case Tests
// =============================================================================

#[test]
fn error_handling_vec_bounds() {
    // This won't actually prove anything since tests will likely always be
    // run on devices with larger amounts of memory, but it can't hurt.
    assert!(matches!(
        deserialize::<_, Vec<u8>, false>(
            Slim::new().with_header(false),
            [(1 << 7) | 8, 255, 255, 255, 0, 0, 0, 0, 0].as_slice()
        ),
        Err(Error::Io(io)) if io.kind() == ErrorKind::UnexpectedEof
    ));
}

#[test]
fn varint_boundary_tests() {
    loopback(u32::MAX);

    // The bytes are a value on its own, so no header precedes them.
    let deser =
        deserialize::<_, u32, false>(Slim::new().with_header(false), [0xFF, 0xFF, 0xFF, 0xFF, 0x1F].as_slice());
    assert!(matches!(deser, Err(Error::BadVarint)));
}

// =============================================================================
// Fixed int encoding
// =============================================================================

#[test]
fn fixed_int() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    pub struct DefinitelyLE {
        #[serde(with = "postbag::fixint")]
        x: u16,
    }

    loopback(DefinitelyLE { x: 0xABCD });
}

// =============================================================================
// Serde alias tests
// =============================================================================

/// Exposure.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Exposure {
    /// Gain level.
    pub gain: u8,
    /// Exposure time in microseconds.
    #[serde(alias = "time_100usec")]
    pub time_usec: u32,
}

#[test]
fn serde_alias_loopback() {
    loopback(Exposure { gain: 42, time_usec: 100_000 });
    loopback(Exposure { gain: 0, time_usec: 0 });
    loopback(Exposure { gain: 255, time_usec: u32::MAX });
}

#[test]
fn serde_alias_compat() {
    // Old struct uses the original field name that is now an alias.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct ExposureOld {
        gain: u8,
        time_100usec: u32,
    }

    let old = ExposureOld { gain: 42, time_100usec: 100_000 };

    // Serialize with old field name, deserialize with new struct that has alias.
    // This only works in Full mode (field names are serialized).
    let mut serialized = Vec::new();
    serialize(Full::new(), &mut serialized, &old).expect("serialization failed");
    println!("old serialized: {serialized:02x?}");

    let new: Exposure =
        deserialize(Full::new(), serialized.as_slice()).expect("deserialization with alias failed");
    assert_eq!(new.gain, old.gain);
    assert_eq!(new.time_usec, old.time_100usec);
}
