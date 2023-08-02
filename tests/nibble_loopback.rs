use core::fmt::Debug;
use core::fmt::Write;
use core::ops::Deref;

#[cfg(feature = "heapless")]
use heapless::{FnvIndexMap, String, Vec};

#[cfg(feature = "heapless")]
use postcard::to_nibble_vec;

use postcard::from_nibbles;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct BasicU8S {
    st: u16,
    ei: u8,
    sf: u64,
    tt: u32,
}

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

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct NewTypeStruct(u32);

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct TupleStruct((u8, u16));

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct RefStruct<'a> {
    bytes: &'a [u8],
    str_s: &'a str,
}

#[cfg(feature = "heapless")]
#[test]
fn loopback() {
    // Basic types
    test_one((), &[]);
    test_one(false, &[0x00]);
    test_one(true, &[0x10]);
    test_one(5u8, &[0x50]);
    test_one(0xA5C7u16, &[0x9A, 0xAF, 0x87]);
    test_one(0xCDAB3412u32, &[0x92, 0xE8, 0xAC, 0xED, 0x0C]);
    test_one(
        0x1234_5678_90AB_CDEFu64,
        &[0xEF, 0x9B, 0xAF, 0x85, 0x89, 0xCF, 0x95, 0x9A, 0x12],
    );

    // https://github.com/jamesmunns/postcard/pull/83
    test_one(32767i16, &[0xFE, 0xFF, 0x03]);
    test_one(-32768i16, &[0xFF, 0xFF, 0x03]);

    // chars
    test_one('z', &[0x10, 0x7a]);
    test_one('¢', &[0x20, 0xc2, 0xa2]);
    test_one('𐍈', &[0x40, 0xF0, 0x90, 0x8D, 0x88]);
    test_one('🥺', &[0x40, 0xF0, 0x9F, 0xA5, 0xBA]);

    // Structs
    test_one(
        BasicU8S {
            st: 0xABCD,
            ei: 0xFE,
            sf: 0x1234_4321_ABCD_DCBA,
            tt: 0xACAC_ACAC,
        },
        &[
            0x9A, 0xDF, 0x95, 0xBF, 0x6B, 0xAB, 0x9B, 0x7D, 0xE9, 0xAE, 0x49, 0x09, 0xA1, 0x2A,
            0xCD, 0x9B, 0x2E, 0x50, 0xA0, // one free nib left at the end
        ],
    );

    // Enums!
    test_one(BasicEnum::Bim, &[0x10]); // one nib
    test_one(
        DataEnum::Bim(u64::max_value()),
        &[
            0x1F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xF0, 0x10, // one free nib left at the end
        ],
    );
    test_one(DataEnum::Bib(u16::max_value()), &[0x09, 0xFF, 0xFF, 0x70]);
    test_one(DataEnum::Bap(u8::max_value()), &[0x2B, 0xF7]);
    test_one(
        DataEnum::Kim(EnumStruct {
            eight: 0xF0,
            sixt: 0xACAC,
        }),
        &[0x3B, 0xE0, 0x9A, 0xEA, 0xD4],
    );
    test_one(
        DataEnum::Chi {
            a: 0x0F,
            b: 0xC7C7C7C7,
        },
        &[0x49, 0x7C, 0x78, 0xF9, 0xFB, 0xE0, 0xC0],
    );
    test_one(DataEnum::Sho(0x6969, 0x07), &[0x5E, 0xCD, 0xD1, 0x70]);

    // Tuples!
    test_one((0x12u8, 0xC7A5u16), &[0xA2, 0x9C, 0xBE, 0xC5]);

    // Structs!
    test_one(NewTypeStruct(5), &[0x05]);
    test_one(TupleStruct((0xA0, 0x1234)), &[0xAC, 0x09, 0x98, 0xE4]);

    let mut input: Vec<u8, 5> = Vec::new();
    input.extend_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05]).unwrap();
    test_one(input, &[0x51, 0x23, 0x45]);

    let mut input: String<8> = String::new();
    write!(&mut input, "helLO!").unwrap();
    test_one(input, &[0x60, b'h', b'e', b'l', b'L', b'O', b'!']);

    let mut input: FnvIndexMap<u8, u8, 4> = FnvIndexMap::new();
    input.insert(0x01, 0x05).unwrap();
    input.insert(0x02, 0x06).unwrap();
    input.insert(0x03, 0x07).unwrap();
    input.insert(0x04, 0x08).unwrap();
    test_one(
        input,
        &[0x41, 0x52, 0x63, 0x74, 0x90],
    );

    // `CString` (uses `serialize_bytes`/`deserialize_byte_buf`)
    #[cfg(feature = "use-std")]
    test_one(
        std::ffi::CString::new("heLlo").unwrap(),
        &[0x50, b'h', b'e', b'L', b'l', b'o'],
    );
}

#[cfg(feature = "heapless")]
#[track_caller]
fn test_one<'a, 'de, T>(data: T, ser_rep: &'a [u8])
where
    T: Serialize + DeserializeOwned + Eq + PartialEq + Debug,
{
    let serialized: Vec<u8, 2048> = to_nibble_vec(&data).unwrap();
    assert_eq!(serialized.len(), ser_rep.len());
    let mut x: ::std::vec::Vec<u8> = vec![];
    x.extend(serialized.deref().iter().cloned());
    assert_eq!(x, ser_rep, "{:x?}", x);
    {
        // let deserialized: T = from_bytes(serialized.deref()).unwrap();
        let deserialized: T = from_nibbles(&x).unwrap();
        assert_eq!(data, deserialized);
    }
}
