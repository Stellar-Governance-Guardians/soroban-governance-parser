//! ScVal <-> JSON conversion. Pure, total where representable, fail-closed
//! otherwise. Wide integers (u64/i64 and larger) are emitted as decimal
//! strings to avoid IEEE-754 precision loss in downstream JSON consumers.

use serde_json::{Map, Value};
use stellar_strkey::{ed25519::PublicKey as StrkeyPublicKey, Contract as StrkeyContract};
use stellar_xdr::{
    ContractExecutable, Int128Parts, Int256Parts, PublicKey, ScAddress, ScContractInstance,
    ScError, ScMap, ScMapEntry, ScVal, UInt128Parts, UInt256Parts,
};

use crate::error::DecodeError;

const TYPE_KEY: &str = "_type";

/// Convert an `ScVal` tree into JSON. Fails with an explicit error on any
/// construct that cannot be represented faithfully.
pub fn scval_to_json(val: &ScVal) -> Result<Value, DecodeError> {
    Ok(match val {
        ScVal::Void => Value::Null,
        ScVal::Bool(b) => Value::Bool(*b),
        ScVal::Error(e) => error_to_json(e),
        ScVal::U32(n) => Value::from(*n),
        ScVal::I32(n) => Value::from(*n),
        // Precision-safety: 64-bit and wider integers become decimal strings.
        ScVal::U64(n) => tagged("u64", Value::from(n.to_string())),
        ScVal::I64(n) => tagged("i64", Value::from(n.to_string())),
        ScVal::Timepoint(t) => tagged("timepoint", Value::from(t.0.to_string())),
        ScVal::Duration(d) => tagged("duration", Value::from(d.0.to_string())),
        ScVal::U128(parts) => tagged("u128", Value::from(u128_parts_to_string(parts))),
        ScVal::I128(parts) => tagged("i128", Value::from(i128_parts_to_string(parts))),
        ScVal::U256(parts) => tagged("u256", Value::from(u256_parts_to_string(parts))),
        ScVal::I256(parts) => tagged("i256", Value::from(i256_parts_to_string(parts))),
        ScVal::Bytes(b) => tagged("bytes", Value::from(base64_encode(b.as_slice()))),
        ScVal::String(s) => Value::from(try_str(&s.0, "ScVal::String")?),
        ScVal::Symbol(s) => Value::from(try_str(s, "ScVal::Symbol")?),
        ScVal::Vec(v) => {
            let inner = v.as_ref().ok_or_else(|| {
                DecodeError::UnsupportedScVal("ScVal::Vec(None) is not representable".into())
            })?;
            let mut out = Vec::with_capacity(inner.len());
            for item in inner.as_slice() {
                out.push(scval_to_json(item)?);
            }
            Value::Array(out)
        }
        ScVal::Map(m) => map_to_json(m.as_ref())?,
        ScVal::Address(a) => address_to_json(a),
        ScVal::ContractInstance(ci) => contract_instance_to_json(ci)?,
        ScVal::LedgerKeyContractInstance => tagged("ledger_key_contract_instance", Value::Null),
        ScVal::LedgerKeyNonce(k) => tagged("ledger_key_nonce", Value::from(k.nonce.to_string())),
        ScVal::ExecutableTag(s) => tagged(
            "executable_tag",
            Value::from(try_str(&s.0, "ScVal::ExecutableTag")?),
        ),
    })
}

fn error_to_json(e: &ScError) -> Value {
    tagged(
        "error",
        match e {
            ScError::Contract(n) => Value::from(format!("contract:{n}")),
            ScError::WasmVm(c) => Value::from(format!("wasm_vm:{c}")),
            ScError::Context(c) => Value::from(format!("context:{c}")),
            ScError::Storage(c) => Value::from(format!("storage:{c}")),
            ScError::Object(c) => Value::from(format!("object:{c}")),
            ScError::Crypto(c) => Value::from(format!("crypto:{c}")),
            ScError::Events(c) => Value::from(format!("events:{c}")),
            ScError::Budget(c) => Value::from(format!("budget:{c}")),
            ScError::Value(c) => Value::from(format!("value:{c}")),
            ScError::Auth(c) => Value::from(format!("auth:{c}")),
        },
    )
}

fn map_to_json(m: Option<&ScMap>) -> Result<Value, DecodeError> {
    let map = m.ok_or_else(|| {
        DecodeError::UnsupportedScVal("ScVal::Map(None) is not representable".into())
    })?;
    let mut out = Map::new();
    for ScMapEntry { key, val } in map.as_slice() {
        // Object keys must be strings in JSON. Non-symbol/string keys would
        // lose information if coerced, so we fail closed on them.
        let k = match key {
            ScVal::Symbol(s) => try_str(s, "map key")?,
            ScVal::String(s) => try_str(&s.0, "map key")?,
            other => {
                return Err(DecodeError::UnsupportedScVal(format!(
                    "map key of variant {} cannot be a JSON object key without loss; \
                     refusing to coerce",
                    discriminant_name(other)
                )))
            }
        };
        out.insert(k, scval_to_json(val)?);
    }
    Ok(Value::Object(out))
}

fn address_to_json(a: &ScAddress) -> Value {
    match a {
        ScAddress::Account(id) => match &id.0 {
            PublicKey::PublicKeyTypeEd25519(u) => tagged(
                "address:account",
                Value::from(StrkeyPublicKey(u.0).to_string()),
            ),
        },
        ScAddress::Contract(cid) => tagged(
            "address:contract",
            Value::from(StrkeyContract(cid.0 .0).to_string()),
        ),
        ScAddress::MuxedAccount(m) => tagged(
            "address:muxed_account",
            Value::from(format!("{}:{}", StrkeyPublicKey(m.ed25519.0), m.id)),
        ),
        ScAddress::ClaimableBalance(cb) => match cb {
            stellar_xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(h) => tagged(
                "address:claimable_balance",
                Value::from(hex_encode(h.as_slice())),
            ),
        },
        ScAddress::LiquidityPool(p) => tagged(
            "address:liquidity_pool",
            Value::from(hex_encode(p.0.as_slice())),
        ),
    }
}

fn contract_instance_to_json(ci: &ScContractInstance) -> Result<Value, DecodeError> {
    let mut m = Map::new();
    m.insert(TYPE_KEY.into(), Value::from("contract_instance"));
    m.insert(
        "executable".into(),
        match &ci.executable {
            ContractExecutable::Wasm(hash) => tagged(
                "executable:wasm",
                Value::from(base64_encode(hash.as_slice())),
            ),
            ContractExecutable::StellarAsset => tagged("executable:stellar_asset", Value::Null),
            ContractExecutable::ExternalRef(r) => {
                tagged("executable:external_ref", Value::from(format!("{r:?}")))
            }
        },
    );
    m.insert(
        "storage".into(),
        match ci.storage.as_ref() {
            Some(_) => map_to_json(ci.storage.as_ref())?,
            None => Value::Null,
        },
    );
    Ok(Value::Object(m))
}

fn try_str(bytes: &[u8], what: &str) -> Result<String, DecodeError> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|e| DecodeError::UnsupportedScVal(format!("{what} is not valid UTF-8: {e}")))
}

fn discriminant_name(v: &ScVal) -> &'static str {
    match v {
        ScVal::Void => "Void",
        ScVal::Bool(_) => "Bool",
        ScVal::Error(_) => "Error",
        ScVal::U32(_) => "U32",
        ScVal::I32(_) => "I32",
        ScVal::U64(_) => "U64",
        ScVal::I64(_) => "I64",
        ScVal::Timepoint(_) => "Timepoint",
        ScVal::Duration(_) => "Duration",
        ScVal::U128(_) => "U128",
        ScVal::I128(_) => "I128",
        ScVal::U256(_) => "U256",
        ScVal::I256(_) => "I256",
        ScVal::Bytes(_) => "Bytes",
        ScVal::String(_) => "String",
        ScVal::Symbol(_) => "Symbol",
        ScVal::Vec(_) => "Vec",
        ScVal::Map(_) => "Map",
        ScVal::Address(_) => "Address",
        ScVal::ContractInstance(_) => "ContractInstance",
        ScVal::LedgerKeyContractInstance => "LedgerKeyContractInstance",
        ScVal::LedgerKeyNonce(_) => "LedgerKeyNonce",
        ScVal::ExecutableTag(_) => "ExecutableTag",
    }
}

fn tagged(t: &str, v: Value) -> Value {
    let mut m = Map::new();
    m.insert(TYPE_KEY.into(), Value::from(t));
    m.insert("value".into(), v);
    Value::Object(m)
}

fn u128_parts_to_string(p: &UInt128Parts) -> String {
    (((u128::from(p.hi)) << 64) | u128::from(p.lo)).to_string()
}

fn i128_parts_to_string(p: &Int128Parts) -> String {
    // Exact bit composition: hi is the signed high word, lo the unsigned low.
    let raw = (i128::from(p.hi) << 64) | i128::from(p.lo);
    raw.to_string()
}

fn u256_parts_to_string(p: &UInt256Parts) -> String {
    let mut digits = big_to_decimal(&[p.hi_hi, p.hi_lo, p.lo_hi, p.lo_lo]);
    digits.reverse();
    digits.into_iter().collect()
}

fn i256_parts_to_string(p: &Int256Parts) -> String {
    let negative = p.hi_hi < 0;
    // Deliberate two's-complement bit reinterpretation of the signed high word.
    #[allow(clippy::cast_sign_loss)]
    let mut words = [p.hi_hi as u64, p.hi_lo, p.lo_hi, p.lo_lo];
    if negative {
        twos_complement_negate(&mut words);
    }
    let mut digits = big_to_decimal(&words);
    digits.reverse();
    let s: String = digits.into_iter().collect();
    if negative {
        format!("-{s}")
    } else {
        s
    }
}

/// Divide a big-endian array of u64 words by 10 repeatedly, returning the
/// decimal digits (least significant first).
// Truncation casts are exact here: each quotient is bounded by the u64 word
// it was computed from, and the remainder is bounded by 9.
#[allow(clippy::cast_possible_truncation)]
fn big_to_decimal(words: &[u64; 4]) -> Vec<char> {
    let mut w = *words;
    let mut digits = Vec::new();
    loop {
        let mut rem: u128 = 0;
        let mut all_zero = true;
        for word in &mut w {
            let cur = (rem << 32) | u128::from(*word >> 32);
            let q_hi = (cur / 10) as u64;
            rem = cur % 10;
            let cur2 = (rem << 32) | u128::from(*word & 0xFFFF_FFFF);
            let q_lo = (cur2 / 10) as u64;
            rem = cur2 % 10;
            *word = (q_hi << 32) | q_lo;
            if *word != 0 {
                all_zero = false;
            }
        }
        let digit = char::from(b'0' + (rem as u8));
        digits.push(digit);
        if all_zero {
            break;
        }
    }
    digits
}

fn twos_complement_negate(words: &mut [u64; 4]) {
    let mut carry: u64 = 1;
    for w in words.iter_mut().rev() {
        let (inv, o1) = (!*w).overflowing_add(carry);
        *w = inv;
        carry = u64::from(o1);
    }
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(char::from(BASE64_ALPHABET[(n >> 18) as usize & 63]));
        out.push(char::from(BASE64_ALPHABET[(n >> 12) as usize & 63]));
        if chunk.len() > 1 {
            out.push(char::from(BASE64_ALPHABET[(n >> 6) as usize & 63]));
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(char::from(BASE64_ALPHABET[n as usize & 63]));
        } else {
            out.push('=');
        }
    }
    out
}

fn hex_encode(input: &[u8]) -> String {
    let mut s = String::with_capacity(input.len() * 2);
    for b in input {
        s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        s.push(char::from_digit(u32::from(b & 0xF), 16).unwrap_or('0'));
    }
    s
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use stellar_xdr::{ScVec, VecM};

    fn sym(s: &str) -> ScVal {
        ScVal::Symbol(stellar_xdr::ScSymbol(s.parse().expect("short symbol")))
    }

    #[test]
    fn symbol_roundtrip() {
        let json = scval_to_json(&sym("fee")).expect("symbol converts");
        assert_eq!(json, Value::from("fee"));
    }

    #[test]
    fn u128_precision_preserved() {
        let v = ScVal::U128(UInt128Parts {
            hi: u64::MAX,
            lo: u64::MAX,
        });
        let json = scval_to_json(&v).expect("converts");
        assert_eq!(
            json.get("value").and_then(Value::as_str),
            Some("340282366920938463463374607431768211455")
        );
    }

    #[test]
    fn i128_negative() {
        let v = ScVal::I128(Int128Parts { hi: -1, lo: 0 });
        let json = scval_to_json(&v).expect("converts");
        assert_eq!(
            json.get("value").and_then(Value::as_str),
            Some("-18446744073709551616")
        );
        let v = ScVal::I128(Int128Parts {
            hi: -1,
            lo: u64::MAX,
        });
        let json = scval_to_json(&v).expect("converts");
        assert_eq!(json.get("value").and_then(Value::as_str), Some("-1"));
    }

    #[test]
    fn i256_negative_small() {
        let v = ScVal::I256(Int256Parts {
            hi_hi: -1,
            hi_lo: u64::MAX,
            lo_hi: u64::MAX,
            lo_lo: u64::MAX - 4,
        });
        let json = scval_to_json(&v).expect("converts");
        assert_eq!(json.get("value").and_then(Value::as_str), Some("-5"));
    }

    #[test]
    fn u256_zero() {
        let v = ScVal::U256(UInt256Parts {
            hi_hi: 0,
            hi_lo: 0,
            lo_hi: 0,
            lo_lo: 0,
        });
        let json = scval_to_json(&v).expect("converts");
        assert_eq!(json.get("value").and_then(Value::as_str), Some("0"));
    }

    #[test]
    fn nested_map_and_vec() {
        let v = ScVal::Map(Some(
            vec![ScMapEntry {
                key: sym("voters"),
                val: ScVal::Vec(Some(ScVec(
                    vec![ScVal::U32(1), ScVal::U32(2)]
                        .try_into()
                        .expect("small"),
                ))),
            }]
            .try_into()
            .expect("small"),
        ));
        let json = scval_to_json(&v).expect("converts");
        assert_eq!(json["voters"][0], Value::from(1));
        assert_eq!(json["voters"][1], Value::from(2));
        let _ = VecM::<u8, 8>::default();
    }

    #[test]
    fn non_string_map_key_fails_closed() {
        let v = ScVal::Map(Some(
            vec![ScMapEntry {
                key: ScVal::U32(7),
                val: ScVal::Void,
            }]
            .try_into()
            .expect("small"),
        ));
        assert!(matches!(
            scval_to_json(&v),
            Err(DecodeError::UnsupportedScVal(_))
        ));
    }

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    /// Real captured testnet XDR: topic[0] of a Stellar Asset Contract `fee`
    /// event (testnet, ledger 5035200, tx 2dceb657..., SAC contract
    /// CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC).
    #[test]
    fn real_testnet_topic_decodes() {
        use stellar_xdr::{Limits, ReadXdr};
        let bytes = base64_decode("AAAADwAAAANmZWUA");
        let v = ScVal::from_xdr(&bytes, Limits::none()).expect("valid XDR");
        assert_eq!(scval_to_json(&v).expect("converts"), Value::from("fee"));
    }

    #[allow(clippy::cast_possible_truncation)]
    fn base64_decode(s: &str) -> Vec<u8> {
        // Test helper: decode the fixed alphabet with padding.
        let mut out = Vec::new();
        let mut buf: u32 = 0;
        let mut bits: u32 = 0;
        for c in s.bytes().filter(|b| *b != b'=') {
            let v = BASE64_ALPHABET
                .iter()
                .position(|a| *a == c)
                .expect("valid base64 char");
            buf = (buf << 6) | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
        out
    }
}
