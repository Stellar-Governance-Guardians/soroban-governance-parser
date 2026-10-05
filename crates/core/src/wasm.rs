//! Minimal, dependency-free WebAssembly custom-section extractor.
//!
//! Only parses the section table (magic, version, section id/size pairs) and
//! reads custom sections (id 0). Fail-closed on malformed binaries.

use crate::error::DecodeError;

const WASM_MAGIC: [u8; 4] = [0x00, b'a', b's', b'm'];
const CUSTOM_SECTION_ID: u8 = 0;

/// Extract every custom section from a WASM binary as (name, payload).
pub fn custom_sections(wasm: &[u8]) -> Result<Vec<(String, Vec<u8>)>, DecodeError> {
    if wasm.len() < 8 {
        return Err(DecodeError::Wasm(
            "binary shorter than 8-byte header".into(),
        ));
    }
    if wasm[0..4] != WASM_MAGIC {
        return Err(DecodeError::Wasm("missing \\0asm magic".into()));
    }
    let version = u32::from_le_bytes([wasm[4], wasm[5], wasm[6], wasm[7]]);
    if version != 1 {
        return Err(DecodeError::Wasm(format!(
            "unsupported wasm version {version}"
        )));
    }

    let mut out = Vec::new();
    let mut pos = 8usize;
    while pos < wasm.len() {
        let id = wasm[pos];
        pos += 1;
        let (size, consumed) = read_uleb128(&wasm[pos..])?;
        pos += consumed;
        let size = usize::try_from(size)
            .map_err(|_| DecodeError::Wasm("section size exceeds usize".into()))?;
        let end = pos
            .checked_add(size)
            .ok_or_else(|| DecodeError::Wasm("section size overflow".into()))?;
        if end > wasm.len() {
            return Err(DecodeError::Wasm(format!(
                "section {id} claims {size} bytes but only {} remain",
                wasm.len() - pos
            )));
        }
        if id == CUSTOM_SECTION_ID {
            let (name_len, n_consumed) = read_uleb128(&wasm[pos..end])?;
            let name_start = pos + n_consumed;
            let name_len = usize::try_from(name_len)
                .map_err(|_| DecodeError::Wasm("custom section name length overflow".into()))?;
            let name_end = name_start
                .checked_add(name_len)
                .ok_or_else(|| DecodeError::Wasm("name length overflow".into()))?;
            if name_end > end {
                return Err(DecodeError::Wasm(
                    "custom section name exceeds section".into(),
                ));
            }
            let name = std::str::from_utf8(&wasm[name_start..name_end])
                .map_err(|e| DecodeError::Wasm(format!("custom section name not UTF-8: {e}")))?
                .to_owned();
            out.push((name, wasm[name_end..end].to_vec()));
        }
        pos = end;
    }
    if pos != wasm.len() {
        return Err(DecodeError::Wasm(
            "trailing bytes after last section".into(),
        ));
    }
    Ok(out)
}

/// Extract the payload of the first custom section with the given name.
pub fn custom_section(wasm: &[u8], name: &str) -> Result<Option<Vec<u8>>, DecodeError> {
    Ok(custom_sections(wasm)?
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, payload)| payload))
}

fn read_uleb128(bytes: &[u8]) -> Result<(u64, usize), DecodeError> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    for (i, byte) in bytes.iter().enumerate() {
        if shift >= 64 {
            return Err(DecodeError::Wasm("LEB128 value exceeds 64 bits".into()));
        }
        result |= u64::from(byte & 0x7F) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Ok((result, i + 1));
        }
    }
    Err(DecodeError::Wasm(
        "truncated LEB128 at end of binary".into(),
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn write_uleb128(out: &mut Vec<u8>, mut v: u64) {
        loop {
            let byte = (v & 0x7F) as u8;
            v >>= 7;
            if v == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    fn build_wasm(sections: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&WASM_MAGIC);
        out.extend_from_slice(&1u32.to_le_bytes());
        for (name, payload) in sections {
            let mut body = Vec::new();
            write_uleb128(&mut body, name.len() as u64);
            body.extend_from_slice(name.as_bytes());
            body.extend_from_slice(payload);
            out.push(CUSTOM_SECTION_ID);
            write_uleb128(&mut out, body.len() as u64);
            out.extend_from_slice(&body);
        }
        out
    }

    #[test]
    fn extracts_named_section() {
        let wasm = build_wasm(&[("other", b"123"), ("contractspecv0", b"hello")]);
        let got = custom_section(&wasm, "contractspecv0").expect("valid wasm");
        assert_eq!(got.as_deref(), Some(b"hello".as_slice()));
    }

    #[test]
    fn missing_section_is_none_not_error() {
        let wasm = build_wasm(&[("other", b"123")]);
        let got = custom_section(&wasm, "contractspecv0").expect("valid wasm");
        assert_eq!(got, None);
    }

    #[test]
    fn bad_magic_fails_closed() {
        let err = custom_section(b"not-a-wasm-at-all", "contractspecv0");
        assert!(matches!(err, Err(DecodeError::Wasm(_))));
    }

    #[test]
    fn truncated_section_fails_closed() {
        let mut wasm = build_wasm(&[("contractspecv0", b"hello")]);
        let n = wasm.len();
        wasm.truncate(n - 3);
        assert!(matches!(
            custom_section(&wasm, "contractspecv0"),
            Err(DecodeError::Wasm(_))
        ));
    }

    #[test]
    fn leb128_multibyte_size() {
        // Section with 200-byte payload forces a 2-byte LEB128 size field.
        let payload = vec![0xABu8; 200];
        let wasm = build_wasm(&[("contractspecv0", &payload)]);
        let got = custom_section(&wasm, "contractspecv0").expect("valid wasm");
        assert_eq!(got, Some(payload));
    }
}
