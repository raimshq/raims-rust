use crate::{contract::valid_uuid, Error, ErrorKind};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default allowed difference from the signing timestamp, in either direction.
pub const DEFAULT_WEBHOOK_TOLERANCE: Duration = Duration::from_secs(300);

fn invalid() -> Error {
    Error::new(
        ErrorKind::SignatureVerification,
        "The webhook signature, signing headers, timestamp or secret is invalid.",
    )
}

/// Authenticates the exact HTTP body bytes before any JSON decoding.
/// Pass each original header separately so duplicate signing headers can be rejected.
pub fn verify_webhook_signature(
    payload: &[u8],
    headers: &[(&str, &str)],
    secret: &str,
    tolerance: Duration,
    now: SystemTime,
) -> Result<(), Error> {
    let id = signing_header(headers, "webhook-id")?;
    let timestamp = signing_header(headers, "webhook-timestamp")?;
    let signatures = signing_header(headers, "webhook-signature")?;
    if !valid_uuid(id)
        || timestamp.is_empty()
        || timestamp.len() > 12
        || !timestamp.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(invalid());
    }
    let timestamp_seconds = timestamp.parse::<u64>().map_err(|_| invalid())?;
    let current_seconds = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| invalid())?
        .as_secs();
    if current_seconds.abs_diff(timestamp_seconds) > tolerance.as_secs() {
        return Err(invalid());
    }
    let mut key = decode_secret(secret)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).map_err(|_| invalid())?;
    key.fill(0);
    mac.update(id.as_bytes());
    mac.update(b".");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(payload);
    let mut matched = false;
    for entry in signatures.split(',') {
        let entry = entry.trim_matches([' ', '\t']);
        if entry.len() != 67 || !entry.as_bytes()[..3].eq_ignore_ascii_case(b"v1=") {
            continue;
        }
        let mut received = [0_u8; 32];
        let mut valid = true;
        for (index, pair) in entry.as_bytes()[3..].chunks_exact(2).enumerate() {
            match (hex(pair[0]), hex(pair[1])) {
                (Some(high), Some(low)) => received[index] = (high << 4) | low,
                _ => {
                    valid = false;
                    break;
                }
            }
        }
        if valid {
            matched |= mac.clone().verify_slice(&received).is_ok();
        }
    }
    if !matched {
        return Err(invalid());
    }
    Ok(())
}

/// Authenticates the original bytes and then decodes a JSON event object.
/// The receiver must also check its environment and durably deduplicate its ID.
pub fn construct_webhook_event(
    payload: &[u8],
    headers: &[(&str, &str)],
    secret: &str,
    tolerance: Duration,
    now: SystemTime,
) -> Result<Value, Error> {
    verify_webhook_signature(payload, headers, secret, tolerance, now)?;
    let value: Value = serde_json::from_slice(payload).map_err(|_| invalid())?;
    if !value.is_object() {
        return Err(invalid());
    }
    Ok(value)
}

fn signing_header<'a>(headers: &'a [(&str, &'a str)], name: &str) -> Result<&'a str, Error> {
    let mut matching = headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let value = matching
        .next()
        .map(|(_, value)| *value)
        .ok_or_else(invalid)?;
    if matching.next().is_some()
        || value.is_empty()
        || value.bytes().any(|b| (b < 32 && b != b'\t') || b == 127)
    {
        return Err(invalid());
    }
    Ok(value)
}

fn decode_secret(secret: &str) -> Result<[u8; 32], Error> {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let encoded = secret.strip_prefix("whsec_").ok_or_else(invalid)?;
    if encoded.is_empty() || encoded.len() > 43 || (encoded.len() > 1 && encoded.starts_with('0')) {
        return Err(invalid());
    }
    let mut decoded = [0_u8; 32];
    for digit in encoded.bytes() {
        let mut carry = ALPHABET
            .iter()
            .position(|c| *c == digit)
            .ok_or_else(invalid)? as u32;
        for byte in &mut decoded {
            carry += u32::from(*byte) * 62;
            *byte = carry as u8;
            carry >>= 8;
        }
        if carry != 0 {
            decoded.fill(0);
            return Err(invalid());
        }
    }
    Ok(decoded)
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
