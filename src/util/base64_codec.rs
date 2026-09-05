//! ZDEP-007: byte-level base64 codec replacing the `base64` crate.
//! Standard alphabet, `=` padding on encode; whitespace skipped and both
//! padded/unpadded input accepted on decode; invalid bytes reject the
//! whole payload (matches tmux's b64_ntop/b64_pton).

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode_bytes(data: &[u8]) -> String {
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        result.push(BASE64_CHARS[b0 >> 2] as char);
        result.push(BASE64_CHARS[((b0 & 0x03) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(BASE64_CHARS[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }
    result
}

pub fn base64_decode_bytes(encoded: &str) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    // tmux parity: b64_pton skips ASCII whitespace anywhere in the payload;
    // any other non-alphabet byte rejects the whole payload via `?` below.
    let chars: Vec<u8> = encoded
        .bytes()
        .filter(|&b| b != b'=' && !b.is_ascii_whitespace())
        .collect();
    for chunk in chars.chunks(4) {
        if chunk.len() < 2 { break; }
        let b0 = BASE64_CHARS.iter().position(|&c| c == chunk[0])? as u8;
        let b1 = BASE64_CHARS.iter().position(|&c| c == chunk[1])? as u8;
        result.push((b0 << 2) | (b1 >> 4));
        if chunk.len() > 2 {
            let b2 = BASE64_CHARS.iter().position(|&c| c == chunk[2])? as u8;
            result.push((b1 << 4) | (b2 >> 2));
            if chunk.len() > 3 {
                let b3 = BASE64_CHARS.iter().position(|&c| c == chunk[3])? as u8;
                result.push((b2 << 6) | b3);
            }
        }
    }
    Some(result)
}
