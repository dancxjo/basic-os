use alloc::string::String;
use uuid::Uuid;

const CONSONANTS: &[u8] = b"bdfghjklmnprstvz";
const VOWELS: &[u8] = b"aeiou";

pub fn from_uuid(uuid: Uuid) -> String {
    let bytes = uuid.as_bytes();
    let mut result = String::new();

    for chunk in bytes.chunks(2) {
        let word = proquint_word(chunk);
        if !result.is_empty() {
            result.push('-');
        }
        result.push_str(&word);
    }

    result
}

fn proquint_word(bytes: &[u8]) -> String {
    let mut word = String::new();
    let value = u16::from_be_bytes([bytes[0], bytes[1]]);

    word.push(CONSONANTS[((value >> 12) & 0x0F) as usize] as char);
    word.push(VOWELS[((value >> 10) & 0x03) as usize] as char);
    word.push(CONSONANTS[((value >> 6) & 0x0F) as usize] as char);
    word.push(VOWELS[((value >> 4) & 0x03) as usize] as char);
    word.push(CONSONANTS[(value & 0x0F) as usize] as char);

    word
}
