//! `contractspecv0` parsing: turn the XDR spec embedded in a contract's WASM
//! into structured, human-readable function/struct/enum descriptions.
//!
//! Encoding source of truth: SEP-0048 / soroban-sdk `contractspecv0`; the
//! section payload is an XDR length-prefixed `VecM<ScSpecEntry>`
//! (stellar-xdr 28 type set, verified against the published crate source).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use stellar_xdr::{
    Limited, Limits, ReadXdr, ScSpecEntry, ScSpecFunctionInputV0, ScSpecFunctionV0, ScSpecTypeDef,
    StringM,
};

use crate::error::DecodeError;
use crate::wasm;

pub const CONTRACTSPECV0_SECTION: &str = "contractspecv0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecFunction {
    pub name: String,
    pub doc: String,
    pub inputs: Vec<SpecInput>,
    pub outputs: Vec<SpecTypeRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecInput {
    pub name: String,
    pub doc: String,
    pub r#type: SpecTypeRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecTypeRef {
    /// Stable, human-readable rendering of the XDR `ScSpecTypeDef`.
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecStruct {
    pub name: String,
    pub lib: String,
    pub fields: Vec<SpecInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractSpec {
    pub functions: Vec<SpecFunction>,
    pub structs: Vec<SpecStruct>,
    pub enums: Vec<String>,
    pub unions: Vec<String>,
    pub errors: Vec<String>,
    pub events: Vec<String>,
    /// Always "contractspecv0" — the section the spec was read from.
    pub spec_version: String,
}

/// Extract and parse the `contractspecv0` custom section from contract WASM.
/// Fail-closed: missing section or malformed XDR is an explicit error.
pub fn parse_contract_spec(wasm_bytes: &[u8]) -> Result<ContractSpec, DecodeError> {
    let payload = wasm::custom_section(wasm_bytes, CONTRACTSPECV0_SECTION)?
        .ok_or(DecodeError::MissingContractSpec)?;
    parse_spec_payload(&payload)
}

/// Parse a raw `contractspecv0` payload (XDR bytes) without WASM wrapping.
/// The payload is a concatenation of XDR `ScSpecEntry` values (no length
/// prefix) — verified against a live testnet deployment; the length-prefixed
/// `VecM` framing fails on real WASM.
pub fn parse_spec_payload(payload: &[u8]) -> Result<ContractSpec, DecodeError> {
    let mut entries: Vec<ScSpecEntry> = Vec::new();
    let mut limited = Limited::new(payload, Limits::none());
    for item in ScSpecEntry::read_xdr_iter(&mut limited) {
        entries.push(item.map_err(|e| DecodeError::Xdr(format!("contractspecv0 payload: {e}")))?);
    }

    let mut spec = ContractSpec {
        functions: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        unions: Vec::new(),
        errors: Vec::new(),
        events: Vec::new(),
        spec_version: CONTRACTSPECV0_SECTION.to_owned(),
    };

    for entry in entries {
        match entry {
            ScSpecEntry::FunctionV0(f) => spec.functions.push(convert_function(&f)?),
            ScSpecEntry::UdtStructV0(s) => {
                let mut fields = Vec::with_capacity(s.fields.len());
                for field in s.fields.as_slice() {
                    fields.push(SpecInput {
                        name: spec_str(&field.name, "struct field name")?,
                        doc: spec_str(&field.doc, "struct field doc")?,
                        r#type: SpecTypeRef {
                            name: spec_type_name(&field.type_)?,
                        },
                    });
                }
                spec.structs.push(SpecStruct {
                    name: spec_str(&s.name, "struct name")?,
                    lib: spec_str(&s.lib, "struct lib")?,
                    fields,
                });
            }
            ScSpecEntry::UdtUnionV0(u) => spec.unions.push(spec_str(&u.name, "union name")?),
            ScSpecEntry::UdtEnumV0(e) => spec.enums.push(spec_str(&e.name, "enum name")?),
            ScSpecEntry::UdtErrorEnumV0(e) => spec.errors.push(spec_str(&e.name, "error name")?),
            ScSpecEntry::EventV0(ev) => spec.events.push(spec_str(&ev.name, "event name")?),
        }
    }
    Ok(spec)
}

fn convert_function(f: &ScSpecFunctionV0) -> Result<SpecFunction, DecodeError> {
    let name = spec_str(&f.name, "function name")?;
    let doc = spec_str(&f.doc, "function doc")?;
    let mut inputs = Vec::with_capacity(f.inputs.len());
    for ScSpecFunctionInputV0 { name, doc, type_ } in f.inputs.as_slice() {
        inputs.push(SpecInput {
            name: spec_str(name, "input name")?,
            doc: spec_str(doc, "input doc")?,
            r#type: SpecTypeRef {
                name: spec_type_name(type_)?,
            },
        });
    }
    let mut outputs = Vec::with_capacity(f.outputs.len());
    for t in f.outputs.as_slice() {
        outputs.push(SpecTypeRef {
            name: spec_type_name(t)?,
        });
    }
    Ok(SpecFunction {
        name,
        doc,
        inputs,
        outputs,
    })
}

fn spec_str<const MAX: u32>(s: &StringM<MAX>, what: &str) -> Result<String, DecodeError> {
    std::str::from_utf8(s)
        .map(str::to_owned)
        .map_err(|e| DecodeError::Xdr(format!("{what} is not valid UTF-8: {e}")))
}

/// Render an `ScSpecTypeDef` as a stable readable name. Total: every variant
/// is handled explicitly; there is no catch-all that could hide new XDR.
pub fn spec_type_name(t: &ScSpecTypeDef) -> Result<String, DecodeError> {
    Ok(match t {
        ScSpecTypeDef::Val => "any".into(),
        ScSpecTypeDef::Bool => "bool".into(),
        ScSpecTypeDef::Void => "void".into(),
        ScSpecTypeDef::Error => "error".into(),
        ScSpecTypeDef::U32 => "u32".into(),
        ScSpecTypeDef::I32 => "i32".into(),
        ScSpecTypeDef::U64 => "u64".into(),
        ScSpecTypeDef::I64 => "i64".into(),
        ScSpecTypeDef::U128 => "u128".into(),
        ScSpecTypeDef::I128 => "i128".into(),
        ScSpecTypeDef::U256 => "u256".into(),
        ScSpecTypeDef::I256 => "i256".into(),
        ScSpecTypeDef::Bytes => "bytes".into(),
        ScSpecTypeDef::String => "string".into(),
        ScSpecTypeDef::Symbol => "symbol".into(),
        ScSpecTypeDef::Address => "address".into(),
        ScSpecTypeDef::MuxedAddress => "muxed_address".into(),
        ScSpecTypeDef::Timepoint => "timepoint".into(),
        ScSpecTypeDef::Duration => "duration".into(),
        ScSpecTypeDef::BytesN(b) => format!("bytesn<{}>", b.n),
        ScSpecTypeDef::Map(m) => format!(
            "map<{}, {}>",
            spec_type_name(&m.key_type)?,
            spec_type_name(&m.value_type)?
        ),
        ScSpecTypeDef::Tuple(t) => {
            let mut parts = Vec::with_capacity(t.value_types.len());
            for v in t.value_types.as_slice() {
                parts.push(spec_type_name(v)?);
            }
            format!("({})", parts.join(", "))
        }
        ScSpecTypeDef::Vec(v) => format!("vec<{}>", spec_type_name(&v.element_type)?),
        ScSpecTypeDef::Option(o) => format!("option<{}>", spec_type_name(&o.value_type)?),
        ScSpecTypeDef::Result(r) => format!(
            "result<{}, {}>",
            spec_type_name(&r.ok_type)?,
            spec_type_name(&r.error_type)?
        ),
        ScSpecTypeDef::Udt(u) => spec_str(&u.name, "UDT name")?,
    })
}

impl ContractSpec {
    /// Look up a function by name. Returns None only when genuinely absent —
    /// callers must treat None as "cannot verify", never as "no arguments".
    pub fn function(&self, name: &str) -> Option<&SpecFunction> {
        self.functions.iter().find(|f| f.name == name)
    }

    pub fn to_json(&self) -> Result<Value, DecodeError> {
        serde_json::to_value(self).map_err(|e| DecodeError::Json(e.to_string()))
    }
}

/// Decode a function's arguments using the spec: zip positional `ScVal`s with
/// declared input names. Fail-closed on arity mismatch or missing function.
pub fn decode_args_with_spec(
    spec: &ContractSpec,
    fn_name: &str,
    args: &[stellar_xdr::ScVal],
) -> Result<Map<String, Value>, DecodeError> {
    let f = spec.function(fn_name).ok_or_else(|| {
        DecodeError::UnsupportedScVal(format!(
            "function `{fn_name}` not found in contractspecv0; refusing to guess argument names"
        ))
    })?;
    if f.inputs.len() != args.len() {
        return Err(DecodeError::UnsupportedScVal(format!(
            "function `{fn_name}` declares {} inputs but {} args were provided",
            f.inputs.len(),
            args.len()
        )));
    }
    let mut out = Map::new();
    for (input, arg) in f.inputs.iter().zip(args) {
        out.insert(input.name.clone(), crate::scval::scval_to_json(arg)?);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use stellar_xdr::{Limits, VecM, WriteXdr};

    #[test]
    fn spec_type_names_are_stable() {
        assert_eq!(spec_type_name(&ScSpecTypeDef::U128).expect("ok"), "u128");
        assert_eq!(
            spec_type_name(&ScSpecTypeDef::Address).expect("ok"),
            "address"
        );
        assert_eq!(
            spec_type_name(&ScSpecTypeDef::BytesN(stellar_xdr::ScSpecTypeBytesN {
                n: 32
            }))
            .expect("ok"),
            "bytesn<32>"
        );
    }

    #[test]
    fn empty_payload_parses_to_empty_spec() {
        // An empty concatenation of entries is a valid empty spec.
        let spec = parse_spec_payload(&[]).expect("parses");
        assert!(spec.functions.is_empty());
        assert_eq!(spec.spec_version, "contractspecv0");
    }

    #[test]
    fn real_function_entry_roundtrips() {
        use stellar_xdr::{
            ScSpecFunctionInputV0, ScSpecFunctionV0, ScSymbol,
        };
        let f = ScSpecFunctionV0 {
            doc: "".parse().expect("empty doc"),
            name: ScSymbol("propose".parse().expect("symbol")),
            inputs: VecM::try_from(vec![ScSpecFunctionInputV0 {
                doc: "".parse().expect("empty doc"),
                name: "creator".parse().expect("name"),
                type_: ScSpecTypeDef::Address,
            }])
            .expect("small"),
            outputs: VecM::try_from(vec![ScSpecTypeDef::U32]).expect("small"),
        };
        let entry = ScSpecEntry::FunctionV0(f);
        let xdr = entry.to_xdr(Limits::none()).expect("serializes");
        let spec = parse_spec_payload(&xdr).expect("parses");
        assert_eq!(spec.functions.len(), 1);
        assert_eq!(spec.functions[0].name, "propose");
        assert_eq!(spec.functions[0].inputs[0].name, "creator");
        assert_eq!(spec.functions[0].inputs[0].r#type.name, "address");
        assert_eq!(spec.functions[0].outputs[0].name, "u32");
    }

    #[test]
    fn garbage_payload_fails_closed() {
        assert!(matches!(
            parse_spec_payload(&[0xFF, 0xFF, 0xFF]),
            Err(DecodeError::Xdr(_))
        ));
    }
}
