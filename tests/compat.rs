use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt::Debug;

use postbag::{
    cfg::{Cfg, Full, Slim, Version},
    deserialize, serialize,
};

/// Transform from one type to another via serialization followed by deserialization.
#[track_caller]
pub fn transform<T, R, const WITH_IDENTS: bool>(value: &T, cfg: Cfg<WITH_IDENTS>) -> R
where
    T: Serialize + DeserializeOwned + Debug + Eq,
    R: DeserializeOwned,
{
    let mut serialized = Vec::new();
    serialize(cfg, &mut serialized, &value).expect("serialization failed");
    println!("{serialized:02x?}");
    dbg!(serialized.len());

    let deserialized: T = deserialize(cfg, serialized.as_slice()).expect("deserialization failed");

    assert_eq!(*value, deserialized, "deserialized value does not match original value");

    deserialize(cfg, serialized.as_slice()).expect("deserialization to transformed type failed")
}

#[test]
fn changed_struct_fields() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct A {
        f1: u32,
        f2: u32,
        f3: u32,
    }

    #[derive(Serialize, Deserialize)]
    struct B {
        f2: u32,
        #[serde(default = "f4_default")]
        f4: u32,
    }

    const fn f4_default() -> u32 {
        4
    }

    let a = A { f1: 1, f2: 2, f3: 3 };

    let b: B = transform(&a, Full::new());

    assert_eq!(b.f2, a.f2);
    assert_eq!(b.f4, f4_default());
}

#[test]
fn added_struct_fields() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct A {
        f1: u32,
        f2: u32,
        f3: u32,
    }

    #[derive(Serialize, Deserialize)]
    struct B {
        f1: u32,
        f2: u32,
        f3: u32,
        #[serde(default = "f4_default")]
        f4: u32,
    }

    const fn f4_default() -> u32 {
        4
    }

    let a = A { f1: 1, f2: 2, f3: 3 };

    let b: B = transform(&a, Full::new());
    assert_eq!(b.f1, a.f1);
    assert_eq!(b.f2, a.f2);
    assert_eq!(b.f3, a.f3);
    assert_eq!(b.f4, f4_default());

    let b: B = transform(&a, Slim::new());
    assert_eq!(b.f1, a.f1);
    assert_eq!(b.f2, a.f2);
    assert_eq!(b.f3, a.f3);
    assert_eq!(b.f4, f4_default());
}

#[test]
fn changed_struct_variant_fields() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum A {
        V1,
        V2 { f1: u32, f2: u32, f3: u32 },
        V3,
    }

    #[derive(Serialize, Deserialize)]
    enum B {
        V1a,
        V3b,
        V2 {
            f2: u32,
            #[serde(default = "f4_default")]
            f4: u32,
        },
    }

    const fn f4_default() -> u32 {
        4
    }

    let a_f2 = 2;
    let a = A::V2 { f1: 1, f2: a_f2, f3: 3 };

    let b: B = transform(&a, Full::new());

    let B::V2 { f2, f4 } = b else { panic!("wrong variant") };
    assert_eq!(f2, a_f2);
    assert_eq!(f4, f4_default());
}

#[test]
fn added_struct_variant_fields() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum A {
        V1,
        V2 { f1: u32, f2: u32, f3: u32 },
        V3,
    }

    #[derive(Serialize, Deserialize)]
    enum B {
        V1a,
        V2 {
            f1: u32,
            f2: u32,
            f3: u32,
            #[serde(default = "f4_default")]
            f4: u32,
        },
    }

    const fn f4_default() -> u32 {
        4
    }

    let a_f1 = 1;
    let a_f2 = 2;
    let a_f3 = 3;
    let a = A::V2 { f1: a_f1, f2: a_f2, f3: a_f3 };

    let b: B = transform(&a, Full::new());
    let B::V2 { f1, f2, f3, f4 } = b else { panic!("wrong variant") };
    assert_eq!(f1, a_f1);
    assert_eq!(f2, a_f2);
    assert_eq!(f3, a_f3);
    assert_eq!(f4, f4_default());

    let b: B = transform(&a, Slim::new());
    let B::V2 { f1, f2, f3, f4 } = b else { panic!("wrong variant") };
    assert_eq!(f1, a_f1);
    assert_eq!(f2, a_f2);
    assert_eq!(f3, a_f3);
    assert_eq!(f4, f4_default());
}

#[test]
fn removed_struct_fields_nested_struct() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct A {
        f1: u32,
        f2: u32,
        f3: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct XA {
        a: A,
        x: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct B {
        f1: u32,
        f2: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct XB {
        a: B,
        x: u32,
    }

    let xa = XA { a: A { f1: 1, f2: 2, f3: 3 }, x: 99 };

    let xb: XB = transform(&xa, Full::new());
    assert_eq!(xb.a.f1, xa.a.f1);
    assert_eq!(xb.a.f2, xa.a.f2);
    assert_eq!(xb.x, xa.x);

    let xb: XB = transform(&xa, Slim::new());
    assert_eq!(xb.a.f1, xa.a.f1);
    assert_eq!(xb.a.f2, xa.a.f2);
    assert_eq!(xb.x, xa.x);
}

#[test]
fn removed_struct_fields_nested_tuple() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct A {
        f1: u32,
        f2: u32,
        f3: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct B {
        f1: u32,
        f2: u32,
    }

    let xa = (A { f1: 1, f2: 2, f3: 3 }, 99);

    let xb: (B, u32) = transform(&xa, Full::new());
    assert_eq!(xb.0.f1, xa.0.f1);
    assert_eq!(xb.0.f2, xa.0.f2);
    assert_eq!(xb.1, xa.1);

    let xb: (B, u32) = transform(&xa, Slim::new());
    assert_eq!(xb.0.f1, xa.0.f1);
    assert_eq!(xb.0.f2, xa.0.f2);
    assert_eq!(xb.1, xa.1);
}

#[test]
fn added_enum_variants_slim_encoding() {
    // Original enum with 3 variants
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Original {
        Variant1,
        Variant2(u32),
        Variant3 { value: String },
    }

    // Extended enum with additional variants at the end
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Extended {
        Variant1,
        Variant2(u32),
        Variant3 {
            value: String,
        },
        Variant4,
        Variant5(bool),
        #[serde(other)]
        Unknown,
    }

    // Even more extended enum for backward compatibility testing
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum MoreExtended {
        Variant1,
        Variant2(u32),
        Variant3 {
            value: String,
        },
        Variant4,
        Variant5(bool),
        Variant6 {
            x: i32,
            y: i32,
        },
        #[serde(other)]
        Unknown,
    }

    // Test forward compatibility: Original -> Extended
    let original_v1 = Original::Variant1;
    let extended_v1: Extended = transform(&original_v1, Slim::new());
    assert_eq!(extended_v1, Extended::Variant1);

    let original_v2 = Original::Variant2(42);
    let extended_v2: Extended = transform(&original_v2, Slim::new());
    assert_eq!(extended_v2, Extended::Variant2(42));

    let original_v3 = Original::Variant3 { value: "test".to_string() };
    let extended_v3: Extended = transform(&original_v3, Slim::new());
    assert_eq!(extended_v3, Extended::Variant3 { value: "test".to_string() });

    // Test backward compatibility: Extended -> Original (with #[serde(other)])
    let extended_v4 = Extended::Variant4;
    let mut serialized = Vec::new();
    serialize(Slim::new(), &mut serialized, &extended_v4).expect("serialization failed");

    // This should deserialize to Unknown variant when using Original enum with #[serde(other)]
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum OriginalWithOther {
        Variant1,
        Variant2(u32),
        Variant3 {
            value: String,
        },
        #[serde(other)]
        Unknown,
    }

    let deserialized: OriginalWithOther =
        deserialize(Slim::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, OriginalWithOther::Unknown);

    let extended_v5 = Extended::Variant5(true);
    let mut serialized = Vec::new();
    serialize(Slim::new(), &mut serialized, &extended_v5).expect("serialization failed");
    let deserialized: OriginalWithOther =
        deserialize(Slim::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, OriginalWithOther::Unknown);

    // Test compatibility with even more extended version
    let more_extended_v6 = MoreExtended::Variant6 { x: 10, y: 20 };
    let mut serialized = Vec::new();
    serialize(Slim::new(), &mut serialized, &more_extended_v6).expect("serialization failed");

    // Should deserialize to Unknown in Extended enum
    let deserialized: Extended = deserialize(Slim::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, Extended::Unknown);

    // Should also deserialize to Unknown in OriginalWithOther enum
    let deserialized: OriginalWithOther =
        deserialize(Slim::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, OriginalWithOther::Unknown);

    // Test that existing variants still work across all versions
    let more_extended_v1 = MoreExtended::Variant1;
    let extended_v1: Extended = transform(&more_extended_v1, Slim::new());
    assert_eq!(extended_v1, Extended::Variant1);

    let original_v1: OriginalWithOther = transform(&more_extended_v1, Slim::new());
    assert_eq!(original_v1, OriginalWithOther::Variant1);
}

#[test]
fn added_enum_variants_full_encoding() {
    // Original enum with 3 variants
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Original {
        Variant1,
        Variant2(u32),
        Variant3 { value: String },
    }

    // Extended enum with additional variants at the end
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Extended {
        Variant1,
        Variant2(u32),
        Variant3 {
            value: String,
        },
        Variant4,
        Variant5(bool),
        #[serde(other)]
        Unknown,
    }

    // Even more extended enum for backward compatibility testing
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum MoreExtended {
        Variant1,
        Variant2(u32),
        Variant3 {
            value: String,
        },
        Variant4,
        Variant5(bool),
        Variant6 {
            x: i32,
            y: i32,
        },
        #[serde(other)]
        Unknown,
    }

    // Test forward compatibility: Original -> Extended
    let original_v1 = Original::Variant1;
    let extended_v1: Extended = transform(&original_v1, Full::new());
    assert_eq!(extended_v1, Extended::Variant1);

    let original_v2 = Original::Variant2(42);
    let extended_v2: Extended = transform(&original_v2, Full::new());
    assert_eq!(extended_v2, Extended::Variant2(42));

    let original_v3 = Original::Variant3 { value: "test".to_string() };
    let extended_v3: Extended = transform(&original_v3, Full::new());
    assert_eq!(extended_v3, Extended::Variant3 { value: "test".to_string() });

    // Test backward compatibility: Extended -> Original (with #[serde(other)])
    let extended_v4 = Extended::Variant4;
    let mut serialized = Vec::new();
    serialize(Full::new(), &mut serialized, &extended_v4).expect("serialization failed");

    // This should deserialize to Unknown variant when using Original enum with #[serde(other)]
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum OriginalWithOther {
        Variant1,
        Variant2(u32),
        Variant3 {
            value: String,
        },
        #[serde(other)]
        Unknown,
    }

    let deserialized: OriginalWithOther =
        deserialize(Full::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, OriginalWithOther::Unknown);

    let extended_v5 = Extended::Variant5(true);
    let mut serialized = Vec::new();
    serialize(Full::new(), &mut serialized, &extended_v5).expect("serialization failed");
    let deserialized: OriginalWithOther =
        deserialize(Full::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, OriginalWithOther::Unknown);

    // Test compatibility with even more extended version
    let more_extended_v6 = MoreExtended::Variant6 { x: 10, y: 20 };
    let mut serialized = Vec::new();
    serialize(Full::new(), &mut serialized, &more_extended_v6).expect("serialization failed");

    // Should deserialize to Unknown in Extended enum
    let deserialized: Extended = deserialize(Full::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, Extended::Unknown);

    // Should also deserialize to Unknown in OriginalWithOther enum
    let deserialized: OriginalWithOther =
        deserialize(Full::new(), serialized.as_slice()).expect("deserialization failed");
    assert_eq!(deserialized, OriginalWithOther::Unknown);

    // Test that existing variants still work across all versions
    let more_extended_v1 = MoreExtended::Variant1;
    let extended_v1: Extended = transform(&more_extended_v1, Full::new());
    assert_eq!(extended_v1, Extended::Variant1);

    let original_v1: OriginalWithOther = transform(&more_extended_v1, Full::new());
    assert_eq!(original_v1, OriginalWithOther::Variant1);
}

#[test]
fn reordered_enum_variants_with_numerical_ids_full_encoding() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Original {
        #[serde(rename = "_0")]
        MyLongVariantName(u32),
        #[serde(rename = "_1")]
        AnotherLongVariantName,
        #[serde(rename = "_2")]
        VariantWithFields {
            #[serde(rename = "_0")]
            value: u8,
        },
    }

    // The variants are reordered and a new one is inserted in the middle,
    // but their numerical identifiers are preserved.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum Reordered {
        #[serde(rename = "_2")]
        VariantWithFields {
            #[serde(rename = "_0")]
            value: u8,
        },
        #[serde(rename = "_5")]
        AddedVariant(bool),
        #[serde(rename = "_1")]
        AnotherLongVariantName,
        #[serde(rename = "_0")]
        MyLongVariantName(u32),
    }

    let unit: Reordered = transform(&Original::AnotherLongVariantName, Full::new());
    assert_eq!(unit, Reordered::AnotherLongVariantName);

    let newtype: Reordered = transform(&Original::MyLongVariantName(42), Full::new());
    assert_eq!(newtype, Reordered::MyLongVariantName(42));

    let structed: Reordered = transform(&Original::VariantWithFields { value: 9 }, Full::new());
    assert_eq!(structed, Reordered::VariantWithFields { value: 9 });

    // A numerically identified variant occupies a single byte.
    let mut serialized = Vec::new();
    serialize(Full::new(), &mut serialized, &Original::AnotherLongVariantName).unwrap();
    assert_eq!(serialized.len(), 1);
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
struct AccountCredentials {
    id: String,
    #[serde(with = "pkcs8_serde")]
    key_pkcs8: Vec<u8>,
    directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    urls: Option<DirectoryUrls>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DirectoryUrls {
    new_nonce: String,
    new_account: String,
    new_order: String,
    new_authz: Option<String>,
    revoke_cert: Option<String>,
    key_change: Option<String>,
}

mod pkcs8_serde {
    use std::fmt;

    use base64::prelude::{BASE64_URL_SAFE_NO_PAD, Engine};
    use serde::{Deserializer, Serializer, de};

    pub fn serialize<S>(key_pkcs8: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = BASE64_URL_SAFE_NO_PAD.encode(key_pkcs8.as_ref());
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base64-encoded PKCS#8 private key")
            }

            fn visit_str<E>(self, v: &str) -> Result<Vec<u8>, E>
            where
                E: de::Error,
            {
                BASE64_URL_SAFE_NO_PAD.decode(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[test]
fn account_credentials_full_with_urls() {
    let test_credentials = AccountCredentials {
        id: "test-account-123".to_string(),
        key_pkcs8: vec![0x30, 0x82, 0x01, 0x22, 0x30, 0x0D], // Mock PKCS#8 DER data
        directory: Some("https://acme-v02.api.letsencrypt.org/directory".to_string()),
        urls: Some(DirectoryUrls {
            new_nonce: "https://acme-v02.api.letsencrypt.org/acme/new-nonce".to_string(),
            new_account: "https://acme-v02.api.letsencrypt.org/acme/new-acct".to_string(),
            new_order: "https://acme-v02.api.letsencrypt.org/acme/new-order".to_string(),
            new_authz: Some("https://acme-v02.api.letsencrypt.org/acme/new-authz".to_string()),
            revoke_cert: Some("https://acme-v02.api.letsencrypt.org/acme/revoke-cert".to_string()),
            key_change: Some("https://acme-v02.api.letsencrypt.org/acme/key-change".to_string()),
        }),
    };

    let _: AccountCredentials = transform(&test_credentials, Full::new());
}

#[test]
fn account_credentials_slim_with_urls() {
    let test_credentials = AccountCredentials {
        id: "test-account-456".to_string(),
        key_pkcs8: vec![0x30, 0x82, 0x01, 0x22, 0x30, 0x0D], // Mock PKCS#8 DER data
        directory: Some("https://acme-v02.api.letsencrypt.org/directory".to_string()),
        urls: Some(DirectoryUrls {
            new_nonce: "https://acme-v02.api.letsencrypt.org/acme/new-nonce".to_string(),
            new_account: "https://acme-v02.api.letsencrypt.org/acme/new-acct".to_string(),
            new_order: "https://acme-v02.api.letsencrypt.org/acme/new-order".to_string(),
            new_authz: Some("https://acme-v02.api.letsencrypt.org/acme/new-authz".to_string()),
            revoke_cert: Some("https://acme-v02.api.letsencrypt.org/acme/revoke-cert".to_string()),
            key_change: Some("https://acme-v02.api.letsencrypt.org/acme/key-change".to_string()),
        }),
    };

    let _: AccountCredentials = transform(&test_credentials, Slim::new());
}

#[test]
fn account_credentials_full_without_urls() {
    let test_credentials = AccountCredentials {
        id: "test-account-789".to_string(),
        key_pkcs8: vec![0x30, 0x82, 0x01, 0x22, 0x30, 0x0D], // Mock PKCS#8 DER data
        directory: Some("https://acme-v02.api.letsencrypt.org/directory".to_string()),
        urls: None, // No URLs
    };

    let _: AccountCredentials = transform(&test_credentials, Full::new());
}

#[test]
fn account_credentials_slim_without_urls() {
    let test_credentials = AccountCredentials {
        id: "test-account-101".to_string(),
        key_pkcs8: vec![0x30, 0x82, 0x01, 0x22, 0x30, 0x0D], // Mock PKCS#8 DER data
        directory: Some("https://acme-v02.api.letsencrypt.org/directory".to_string()),
        urls: None, // No URLs - this will cause skip_serializing_if to omit the field
    };

    let _: AccountCredentials = transform(&test_credentials, Slim::new());
}

// =============================================================================
// Middle field add/remove tests
// =============================================================================

#[test]
#[cfg_attr(postbag_fast_compile, ignore = "fast_compile does not support adding fields in the middle")]
fn added_struct_field_in_middle() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct A {
        f1: u32,
        f2: u32,
        f3: u32,
    }

    #[derive(Serialize, Deserialize)]
    struct B {
        f1: u32,
        #[serde(default = "mid_default")]
        f_mid: u32,
        f2: u32,
        f3: u32,
    }

    const fn mid_default() -> u32 {
        99
    }

    let a = A { f1: 1, f2: 2, f3: 3 };

    // Full mode: fields matched by name, so inserting in the middle works.
    let b: B = transform(&a, Full::new());
    assert_eq!(b.f1, a.f1);
    assert_eq!(b.f_mid, mid_default());
    assert_eq!(b.f2, a.f2);
    assert_eq!(b.f3, a.f3);
}

#[test]
fn removed_struct_field_from_middle() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct A {
        f1: u32,
        f2: u32,
        f3: u32,
    }

    // B drops f2 from the middle.
    #[derive(Serialize, Deserialize)]
    struct B {
        f1: u32,
        f3: u32,
    }

    let a = A { f1: 1, f2: 2, f3: 3 };

    // Full mode: fields matched by name, so removal from the middle works.
    let b: B = transform(&a, Full::new());
    assert_eq!(b.f1, a.f1);
    assert_eq!(b.f3, a.f3);
}

#[test]
#[cfg_attr(postbag_fast_compile, ignore = "fast_compile does not support adding fields in the middle")]
fn added_and_removed_struct_fields_in_middle() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct A {
        f1: u32,
        f2: u32,
        f3: u32,
        f4: u32,
    }

    // B keeps f1 and f4, drops f2/f3, adds f_new in the middle.
    #[derive(Serialize, Deserialize)]
    struct B {
        f1: u32,
        #[serde(default = "new_default")]
        f_new: u32,
        f4: u32,
    }

    const fn new_default() -> u32 {
        77
    }

    let a = A { f1: 1, f2: 2, f3: 3, f4: 4 };

    let b: B = transform(&a, Full::new());
    assert_eq!(b.f1, a.f1);
    assert_eq!(b.f_new, new_default());
    assert_eq!(b.f4, a.f4);
}

#[test]
#[cfg_attr(postbag_fast_compile, ignore = "fast_compile does not support adding fields in the middle")]
fn added_struct_variant_field_in_middle() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum A {
        V1,
        V2 { f1: u32, f2: u32, f3: u32 },
    }

    #[derive(Serialize, Deserialize)]
    enum B {
        V1,
        V2 {
            f1: u32,
            #[serde(default = "mid_default2")]
            f_mid: u32,
            f2: u32,
            f3: u32,
        },
    }

    const fn mid_default2() -> u32 {
        55
    }

    let a = A::V2 { f1: 1, f2: 2, f3: 3 };

    let b: B = transform(&a, Full::new());
    let B::V2 { f1, f_mid, f2, f3 } = b else { panic!("wrong variant") };
    assert_eq!(f1, 1);
    assert_eq!(f_mid, mid_default2());
    assert_eq!(f2, 2);
    assert_eq!(f3, 3);
}

#[test]
fn removed_struct_variant_field_from_middle() {
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    enum A {
        V1,
        V2 { f1: u32, f2: u32, f3: u32 },
    }

    #[derive(Serialize, Deserialize)]
    enum B {
        V1,
        V2 { f1: u32, f3: u32 },
    }

    let a = A::V2 { f1: 1, f2: 2, f3: 3 };

    let b: B = transform(&a, Full::new());
    let B::V2 { f1, f3 } = b else { panic!("wrong variant") };
    assert_eq!(f1, 1);
    assert_eq!(f3, 3);
}

#[test]
fn changed_fields_of_a_nested_struct() {
    // A struct that fills a field's block writes no field count, so the reader
    // finds the end of its fields by the end of the block. Adding, removing
    // and reordering fields has to keep working across that.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct OuterA {
        #[serde(rename = "_0")]
        inner: InnerA,
        #[serde(rename = "_1")]
        after: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct InnerA {
        #[serde(default)]
        f1: u32,
        f2: String,
        #[serde(default)]
        f3: bool,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct OuterB {
        #[serde(rename = "_0")]
        inner: InnerB,
        #[serde(rename = "_1")]
        after: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct InnerB {
        f2: String,
        #[serde(default)]
        f4: Option<u32>,
    }

    let value = OuterA { inner: InnerA { f1: 7, f2: "x".into(), f3: true }, after: 300 };

    for cfg in [Full::new(), Full::new().with_version(Version::Postbag0_4)] {
        // Fields the reader does not know are skipped, one it never got
        // takes its default, and the field after the struct is still found.
        let b: OuterB = transform(&value, cfg);
        assert_eq!(b.inner.f2, "x");
        assert_eq!(b.inner.f4, None);
        assert_eq!(b.after, 300);
    }
}

#[test]
#[cfg_attr(postbag_fast_compile, ignore = "fast_compile does not support adding fields in the middle")]
fn restored_fields_of_a_nested_struct() {
    // The other direction of `changed_fields_of_a_nested_struct`: the reader
    // knows more fields than it is sent, including one before a field it does
    // get, which is what the buffered path cannot do.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Outer<T> {
        #[serde(rename = "_0")]
        inner: T,
        #[serde(rename = "_1")]
        after: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Sent {
        f2: String,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Expected {
        #[serde(default)]
        f1: u32,
        f2: String,
        #[serde(default)]
        f3: bool,
    }

    let value = Outer { inner: Sent { f2: "x".into() }, after: 300 };

    for cfg in [Full::new(), Full::new().with_version(Version::Postbag0_4)] {
        let got: Outer<Expected> = transform(&value, cfg);
        assert_eq!(got.inner, Expected { f1: 0, f2: "x".into(), f3: false });
        assert_eq!(got.after, 300);
    }
}

#[test]
fn a_nested_struct_that_loses_all_its_fields() {
    // The block is then empty, which the reader must read as "no fields"
    // rather than running off the end.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Outer<T> {
        #[serde(rename = "_0")]
        inner: T,
        #[serde(rename = "_1")]
        after: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Empty {}

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Filled {
        #[serde(default)]
        f1: u32,
    }

    for cfg in [Full::new(), Full::new().with_version(Version::Postbag0_4)] {
        let value = Outer { inner: Empty {}, after: 300 };
        let grown: Outer<Filled> = transform(&value, cfg);
        assert_eq!(grown.inner.f1, 0);
        assert_eq!(grown.after, 300);

        let value = Outer { inner: Filled { f1: 7 }, after: 300 };
        let shrunk: Outer<Empty> = transform(&value, cfg);
        assert_eq!(shrunk.after, 300);
    }
}

#[test]
fn a_char_field_widened_to_a_string() {
    // A char and a string encode identically as a field value, so widening
    // one to the other is something people will do. Both directions have to
    // stay readable: with remoc the same pair of programs talks both ways, so
    // a widening that only works in one of them cannot be used at all.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct AsChar {
        #[serde(rename = "_0")]
        unit: char,
        #[serde(rename = "_1")]
        after: u32,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct AsString {
        #[serde(rename = "_0")]
        unit: String,
        #[serde(rename = "_1")]
        after: u32,
    }

    for cfg in [Full::new(), Full::new().with_version(Version::Postbag0_4)] {
        // The updated peer reads what the old one sends, exactly.
        let widened: AsString = transform(&AsChar { unit: '°', after: 300 }, cfg);
        assert_eq!(widened, AsString { unit: "°".into(), after: 300 });

        // The old peer reads what the updated one sends, keeping the first
        // character and — this is the point — the rest of the message.
        for sent in ["°C", "a", "hello, world"] {
            let narrowed: AsChar = transform(&AsString { unit: sent.into(), after: 300 }, cfg);
            assert_eq!(narrowed.unit, sent.chars().next().unwrap(), "reading {sent:?} as a char");
            assert_eq!(narrowed.after, 300, "the field after {sent:?} was still found");
        }

        // Nothing at all is still not a character.
        let empty = postbag::to_vec(cfg, &AsString { unit: String::new(), after: 300 }).unwrap();
        assert!(postbag::from_slice::<AsChar, _>(cfg, empty.as_slice()).is_err());
    }
}

#[test]
fn a_name_that_looks_numbered_but_is_not() {
    // `_07` parses as seven, but reading seven back gives `_7`. Encoding it
    // as a number would lose the field, so it is written out as a name.
    #[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
    struct Padded {
        #[serde(rename = "_07")]
        #[serde(default)]
        padded: u32,
        #[serde(rename = "_7")]
        #[serde(default)]
        plain: u32,
    }

    let value = Padded { padded: 1, plain: 2 };

    for cfg in [Full::new(), Full::new().with_version(Version::Postbag0_4)] {
        let bytes = postbag::to_vec(cfg, &value).unwrap();
        let back: Padded = postbag::from_slice(cfg, bytes.as_slice()).unwrap();

        assert_eq!(back, value, "a padded name must not collide with its plain form");
        assert!(bytes.windows(3).any(|w| w == b"_07"), "the name should be written out");
    }
}
