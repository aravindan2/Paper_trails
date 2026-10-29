//! Byte-level forensic analysis behind `strings`, `hexdump`, `sha256sum`,
//! and `decode`.
//!
//! Every function here is a pure function over byte slices or string slices:
//! no filesystem access, no I/O, no terminal or browser APIs. All output is
//! deterministic, so native tests and the future WASM build see identical
//! results.
//!
//! Design notes:
//! - `strings` treats bytes `0x20..=0x7E` (space through `~`) as printable
//!   ASCII — matching the classic Unix definition — and any other byte as a
//!   separator.
//! - `hexdump` renders a fixed `hexdump -C`-style layout (16 bytes per row:
//!   8-hex-digit offset, hex bytes in two groups, ASCII gutter) but omits
//!   `hexdump`'s trailing end-offset line.
//! - `sha256_hex` computes real SHA-256 through the `sha2` crate; digests are
//!   lowercase hex. No simulated or placeholder hashing exists anywhere.
//! - `decode` is strict on purpose: canonical hex (even digit count, no `0x`
//!   prefix or whitespace) and standard base64 (`A-Za-z0-9+/` with required
//!   `=` padding and zero trailing bits).

use sha2::{Digest, Sha256};
use std::fmt;

/// Default minimum sequence length for `strings` (characters).
pub const DEFAULT_STRINGS_MIN_LENGTH: usize = 4;

/// Bytes per row in `hexdump` output.
pub const HEXDUMP_BYTES_PER_ROW: usize = 16;

/// Printable ASCII: space (`0x20`) through tilde (`0x7e`). Everything else,
/// including control bytes and bytes `>= 0x7F`, breaks a sequence.
const fn is_printable_ascii(byte: u8) -> bool {
    byte >= 0x20 && byte <= 0x7e
}

fn flush_run(run: &mut Vec<u8>, min_length: usize, out: &mut Vec<String>) {
    // Runs only ever contain printable ASCII, so the lossy conversion is exact.
    if !run.is_empty() && run.len() >= min_length {
        out.push(String::from_utf8_lossy(run).into_owned());
    }
    run.clear();
}

/// Extract contiguous printable-ASCII sequences of at least `min_length`
/// bytes, in the order they occur. Empty runs are never emitted, even when
/// `min_length` is 0.
pub fn extract_strings(data: &[u8], min_length: usize) -> Vec<String> {
    let mut sequences = Vec::new();
    let mut run: Vec<u8> = Vec::new();
    for &byte in data {
        if is_printable_ascii(byte) {
            run.push(byte);
        } else {
            flush_run(&mut run, min_length, &mut sequences);
        }
    }
    flush_run(&mut run, min_length, &mut sequences);
    sequences
}

/// Format one half-row of up to 8 bytes as space-separated lowercase hex,
/// padded with placeholder spaces so both halves are always 23 characters.
fn hex_group(row: &[u8], start: usize) -> String {
    (0..HEXDUMP_BYTES_PER_ROW / 2)
        .map(|slot| match row.get(start + slot) {
            Some(byte) => format!("{byte:02x}"),
            None => "  ".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render bytes as a `hexdump -C`-style listing: 8-hex-digit offset, two
/// groups of 8 hex bytes, and an ASCII gutter where non-printables show as
/// `.`. The final partial row is space-padded so the gutter stays aligned.
/// An empty input produces empty output.
pub fn hexdump(data: &[u8]) -> String {
    data.chunks(HEXDUMP_BYTES_PER_ROW)
        .enumerate()
        .map(|(index, row)| {
            let ascii: String = row
                .iter()
                .map(|&byte| {
                    if is_printable_ascii(byte) {
                        byte as char
                    } else {
                        '.'
                    }
                })
                .collect();
            format!(
                "{:08x}  {}  {}  |{ascii}|",
                index * HEXDUMP_BYTES_PER_ROW,
                hex_group(row, 0),
                hex_group(row, HEXDUMP_BYTES_PER_ROW / 2),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compute the SHA-256 digest of `data` as 64 lowercase hex characters.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Why `decode` rejected a value. `Display` renders the message *after* the
/// command prefix, e.g. `decode: invalid hex: odd number of digits (5)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The VALUE token was an empty string.
    EmptyValue,
    OddHexLength {
        digits: usize,
    },
    InvalidHexDigit {
        value: char,
        index: usize,
    },
    Base64Length {
        length: usize,
    },
    InvalidBase64Char {
        value: char,
        index: usize,
    },
    /// `=` appeared before the end of the value.
    PaddingNotAtEnd,
    /// More than two `=` padding characters.
    TooMuchPadding,
    /// The final quantum had non-zero unused bits (non-canonical encoding).
    NonCanonicalTrailingBits,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::EmptyValue => write!(f, "empty value"),
            DecodeError::OddHexLength { digits } => {
                write!(f, "invalid hex: odd number of digits ({digits})")
            }
            DecodeError::InvalidHexDigit { value, index } => {
                write!(f, "invalid hex: invalid digit '{value}' at index {index}")
            }
            DecodeError::Base64Length { length } => {
                write!(f, "invalid base64: length {length} is not a multiple of 4")
            }
            DecodeError::InvalidBase64Char { value, index } => {
                write!(
                    f,
                    "invalid base64: invalid character '{value}' at index {index}"
                )
            }
            DecodeError::PaddingNotAtEnd => {
                write!(f, "invalid base64: '=' padding must appear only at the end")
            }
            DecodeError::TooMuchPadding => {
                write!(
                    f,
                    "invalid base64: at most two '=' padding characters are allowed"
                )
            }
            DecodeError::NonCanonicalTrailingBits => {
                write!(
                    f,
                    "invalid base64: non-canonical trailing bits must be zero"
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}

fn hex_value(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some(c as u8 - b'0'),
        'a'..='f' => Some(c as u8 - b'a' + 10),
        'A'..='F' => Some(c as u8 - b'A' + 10),
        _ => None,
    }
}

/// Decode an even number of hexadecimal digits (case-insensitive) into bytes.
pub fn decode_hex(input: &str) -> Result<Vec<u8>, DecodeError> {
    let chars: Vec<char> = input.chars().collect();
    if chars.is_empty() {
        return Err(DecodeError::EmptyValue);
    }
    if !chars.len().is_multiple_of(2) {
        return Err(DecodeError::OddHexLength {
            digits: chars.len(),
        });
    }
    let mut out = Vec::with_capacity(chars.len() / 2);
    for (index, pair) in chars.chunks(2).enumerate() {
        let high = hex_value(pair[0]).ok_or(DecodeError::InvalidHexDigit {
            value: pair[0],
            index: index * 2,
        })?;
        let low = hex_value(pair[1]).ok_or(DecodeError::InvalidHexDigit {
            value: pair[1],
            index: index * 2 + 1,
        })?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn base64_value(c: char) -> Option<u8> {
    match c {
        'A'..='Z' => Some(c as u8 - b'A'),
        'a'..='z' => Some(c as u8 - b'a' + 26),
        '0'..='9' => Some(c as u8 - b'0' + 52),
        '+' => Some(62),
        '/' => Some(63),
        _ => None,
    }
}

/// Decode standard base64 (`A-Za-z0-9+/`, `=` padding required, length a
/// multiple of 4, canonical zero trailing bits). Whitespace, newlines, and
/// URL-safe alphabets are rejected rather than silently tolerated.
pub fn decode_base64(input: &str) -> Result<Vec<u8>, DecodeError> {
    let chars: Vec<char> = input.chars().collect();
    if chars.is_empty() {
        return Err(DecodeError::EmptyValue);
    }
    if !chars.len().is_multiple_of(4) {
        return Err(DecodeError::Base64Length {
            length: chars.len(),
        });
    }
    let data: &[char] = match chars.iter().position(|&c| c == '=') {
        Some(index) => {
            if chars[index..].iter().any(|&c| c != '=') {
                return Err(DecodeError::PaddingNotAtEnd);
            }
            if chars.len() - index > 2 {
                return Err(DecodeError::TooMuchPadding);
            }
            // With a multiple-of-4 length and at most two padding characters,
            // the data length is necessarily 0, 2, or 3 mod 4; the impossible
            // 1-mod-4 remainder cannot occur here.
            &chars[..index]
        }
        None => &chars[..],
    };

    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    let mut accumulator: u32 = 0;
    let mut bits: u32 = 0;
    for (index, &c) in data.iter().enumerate() {
        let Some(value) = base64_value(c) else {
            return Err(DecodeError::InvalidBase64Char { value: c, index });
        };
        accumulator = (accumulator << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    // Leftover bits exist only in the short final quantum; canonical base64
    // requires them to be zero.
    if accumulator != 0 {
        return Err(DecodeError::NonCanonicalTrailingBits);
    }
    Ok(out)
}

/// Render decoded bytes for the terminal: valid UTF-8 is printed as text;
/// anything else gets the explicit, documented `(bytes) 7f 45 ...` form
/// (lowercase hex, space-separated).
pub fn format_decoded(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let hex = bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("(bytes) {hex}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_extracts_runs_in_order_with_default_minimum() {
        assert_eq!(extract_strings(b"AB\x00CD\x7fEFGH\xff", 4), ["EFGH"]);
        assert_eq!(
            extract_strings(b"AB\x00CD\x7fEFGH\xff", 1),
            ["AB", "CD", "EFGH"]
        );
        assert_eq!(
            extract_strings(b"one\x00two\x00three", 3),
            ["one", "two", "three"]
        );
    }

    #[test]
    fn strings_boundaries_are_space_through_tilde() {
        // Space counts as printable, DEL (0x7f) does not — it splits runs.
        assert_eq!(extract_strings(b"hi there\x00", 4), ["hi there"]);
        assert_eq!(extract_strings(b"~~~~\x00\x7f~~", 2), ["~~~~", "~~"]);
        assert_eq!(extract_strings(b"~~~~\x00\x7f~~", 4), ["~~~~"]);
        // Sequences at the very start and end of the buffer survive.
        assert_eq!(extract_strings(b"\x00run\x00", 3), ["run"]);
        // 0x20..=0x7E: both endpoints printable.
        assert_eq!(extract_strings(b" \x7e\x00", 2), [" \x7e"]);
    }

    #[test]
    fn strings_handles_empty_and_short_inputs() {
        assert!(extract_strings(b"", DEFAULT_STRINGS_MIN_LENGTH).is_empty());
        assert!(extract_strings(b"abc", DEFAULT_STRINGS_MIN_LENGTH).is_empty());
        // Zero minimum still never emits empty runs.
        assert_eq!(extract_strings(b"\x00A\x00", 0), ["A"]);
        // All-binary input yields nothing.
        assert!(extract_strings(&[0x00, 0x01, 0xff], 4).is_empty());
    }

    #[test]
    fn hexdump_of_empty_input_is_empty() {
        assert_eq!(hexdump(b""), "");
    }

    #[test]
    fn hexdump_matches_hexdump_c_format_for_full_row() {
        // Ground truth: `hexdump -C` on bytes 00..10.
        let row: Vec<u8> = (0u8..16).collect();
        assert_eq!(
            hexdump(&row),
            "00000000  00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f  |................|"
        );
    }

    #[test]
    fn hexdump_pads_partial_final_rows() {
        // Ground truth: `hexdump -C` on the 10-byte ELF-style header, and on
        // bytes 00..11 (full row + one leftover byte).
        let engine10 = [0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0xff, 0xfe];
        assert_eq!(
            hexdump(&engine10),
            "00000000  7f 45 4c 46 02 01 01 00  ff fe                    |.ELF......|"
        );

        let seventeen: Vec<u8> = (0u8..=16).collect();
        assert_eq!(
            hexdump(&seventeen),
            "00000000  00 01 02 03 04 05 06 07  08 09 0a 0b 0c 0d 0e 0f  |................|\n\
             00000010  10                                                |.|"
        );
    }

    #[test]
    fn hexdump_ascii_gutter_dots_non_printables() {
        let tool = [0x7f, 0x45, 0x4c, 0x46, 0x78, 0x79, 0xff];
        assert_eq!(
            hexdump(&tool),
            "00000000  7f 45 4c 46 78 79 ff                              |.ELFxy.|"
        );
    }

    #[test]
    fn sha256_matches_fips_and_cross_checked_vectors() {
        // FIPS 180-4 / NIST known-answer vectors.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Cross-checked against `sha256sum` and Python hashlib for these
        // byte sequences (including a 4-block input).
        assert_eq!(
            sha256_hex(&[0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0xff, 0xfe]),
            "f6a87668bcc94b9708ec03e72c7c12907dfc45edd944ddcaa8ac310a36485d18"
        );
        assert_eq!(
            sha256_hex(&(0u8..=16).collect::<Vec<u8>>()),
            "3e5718fea51a8f3f5baca61c77afab473c1810f8b9db330273b4011ce92c787e"
        );
        assert_eq!(
            sha256_hex(&(0u8..=255).collect::<Vec<u8>>()),
            "40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880"
        );
    }

    #[test]
    fn decode_hex_valid_values() {
        assert_eq!(decode_hex("68656c6c6f").unwrap(), b"hello");
        // Case-insensitive digits.
        assert_eq!(decode_hex("48656C6C6F").unwrap(), b"Hello");
        assert_eq!(decode_hex("00").unwrap(), [0]);
        assert_eq!(decode_hex("ffff").unwrap(), [0xff, 0xff]);
    }

    #[test]
    fn decode_hex_rejects_malformed_values() {
        assert_eq!(decode_hex(""), Err(DecodeError::EmptyValue));
        assert_eq!(
            decode_hex("68656"),
            Err(DecodeError::OddHexLength { digits: 5 })
        );
        assert_eq!(
            decode_hex("6g"),
            Err(DecodeError::InvalidHexDigit {
                value: 'g',
                index: 1
            })
        );
        // Non-ASCII input is reported by character, not by byte offset.
        assert_eq!(
            decode_hex("éé"),
            Err(DecodeError::InvalidHexDigit {
                value: 'é',
                index: 0
            })
        );
    }

    #[test]
    fn decode_base64_valid_values() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(
            decode_base64("SGVsbG8sIFdvcmxkIQ==").unwrap(),
            b"Hello, World!"
        );
        // No padding needed when the length is already a multiple of 3.
        assert_eq!(decode_base64("Y2FzZSBmaWxlIDQy").unwrap(), b"case file 42");
        // Standard (non URL-safe) alphabet characters.
        assert_eq!(decode_base64("+/8=").unwrap(), [0xfb, 0xff]);
        // Two-character final quantum.
        assert_eq!(decode_base64("TQ==").unwrap(), b"M");
        // Padding-only or short values are rejected by the length rule first.
        assert_eq!(
            decode_base64("=="),
            Err(DecodeError::Base64Length { length: 2 })
        );
    }

    #[test]
    fn decode_base64_rejects_malformed_values() {
        assert_eq!(decode_base64(""), Err(DecodeError::EmptyValue));
        assert_eq!(
            decode_base64("aGVsbG8"),
            Err(DecodeError::Base64Length { length: 7 })
        );
        assert_eq!(
            decode_base64("ab!d"),
            Err(DecodeError::InvalidBase64Char {
                value: '!',
                index: 2
            })
        );
        assert_eq!(decode_base64("a==="), Err(DecodeError::TooMuchPadding));
        assert_eq!(decode_base64("ab=c"), Err(DecodeError::PaddingNotAtEnd));
        // 'Q'=16, 'R'=17 leaves a non-zero trailing nibble: non-canonical.
        assert_eq!(
            decode_base64("QR=="),
            Err(DecodeError::NonCanonicalTrailingBits)
        );
        // Whitespace is not silently stripped.
        assert_eq!(
            decode_base64("aGVs bG8="),
            Err(DecodeError::Base64Length { length: 9 })
        );
    }

    #[test]
    fn format_decoded_prefers_text_and_falls_back_to_bytes() {
        assert_eq!(format_decoded(b"hello"), "hello");
        assert_eq!(format_decoded(b""), "");
        // Valid multi-byte UTF-8 is still text.
        assert_eq!(format_decoded("héllo".as_bytes()), "héllo");
        // Invalid UTF-8 uses the documented representation.
        assert_eq!(format_decoded(&[0xff, 0xfe]), "(bytes) ff fe");
        assert_eq!(format_decoded(b"ab\xff"), "(bytes) 61 62 ff");
    }
}
