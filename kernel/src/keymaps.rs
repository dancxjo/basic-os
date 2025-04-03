use alloc::collections::BTreeMap;

/// Represents a single compose mapping: dead + base → result
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComposeKey {
    pub dead: char,
    pub base: char,
}

pub struct ComposeTable {
    table: BTreeMap<ComposeKey, char>,
}

impl ComposeTable {
    pub fn new() -> Self {
        let mut table = BTreeMap::new();

        macro_rules! compose {
            ($dead:literal, $base:literal => $out:literal) => {
                table.insert(
                    ComposeKey {
                        dead: $dead,
                        base: $base,
                    },
                    $out,
                );
            };
        }

        // Acute accents
        compose!('´', 'a' => 'á');
        compose!('´', 'e' => 'é');
        compose!('´', 'i' => 'í');
        compose!('´', 'o' => 'ó');
        compose!('´', 'u' => 'ú');
        compose!('´', 'y' => 'ý');
        compose!('´', 'A' => 'Á');
        compose!('´', 'E' => 'É');
        compose!('´', 'I' => 'Í');
        compose!('´', 'O' => 'Ó');
        compose!('´', 'U' => 'Ú');
        compose!('´', 'Y' => 'Ý');

        // Grave
        compose!('`', 'a' => 'à');
        compose!('`', 'e' => 'è');
        compose!('`', 'i' => 'ì');
        compose!('`', 'o' => 'ò');
        compose!('`', 'u' => 'ù');
        compose!('`', 'A' => 'À');

        // Circumflex
        compose!('^', 'a' => 'â');
        compose!('^', 'e' => 'ê');
        compose!('^', 'i' => 'î');
        compose!('^', 'o' => 'ô');
        compose!('^', 'u' => 'û');
        compose!('^', 'A' => 'Â');

        // Tilde
        compose!('~', 'a' => 'ã');
        compose!('~', 'n' => 'ñ');
        compose!('~', 'o' => 'õ');
        compose!('~', 'A' => 'Ã');
        compose!('~', 'N' => 'Ñ');
        compose!('~', 'O' => 'Õ');

        // Diaeresis
        compose!('"', 'a' => 'ä');
        compose!('"', 'e' => 'ë');
        compose!('"', 'i' => 'ï');
        compose!('"', 'o' => 'ö');
        compose!('"', 'u' => 'ü');
        compose!('"', 'y' => 'ÿ');
        compose!('"', 'A' => 'Ä');

        // Cedilla
        compose!(',', 'c' => 'ç');
        compose!(',', 'C' => 'Ç');

        Self { table }
    }

    pub fn try_compose(&self, dead: char, base: char) -> Option<char> {
        self.table.get(&ComposeKey { dead, base }).copied()
    }
}

/// US QWERTY with AltGr (US International-AltGr style), basic support for four layers
#[derive(Copy, Clone)]
pub struct KeyMapEntry {
    pub normal: Option<char>,      // Base
    pub shifted: Option<char>,     // Shift
    pub altgr: Option<char>,       // AltGr / Right Alt
    pub shift_altgr: Option<char>, // Shift + AltGr
}

pub const US_ALTGR_INTL: [KeyMapEntry; 512] = {
    let mut map = [KeyMapEntry {
        normal: None,
        shifted: None,
        altgr: None,
        shift_altgr: None,
    }; 512];

    macro_rules! key {
        ($sc:expr, $norm:expr, $shift:expr, $altgr:expr, $s_altgr:expr) => {
            map[$sc] = KeyMapEntry {
                normal: Some($norm),
                shifted: Some($shift),
                altgr: Some($altgr),
                shift_altgr: Some($s_altgr),
            };
        };
    }

    key!(0x02, '1', '!', '¡', '¹');
    key!(0x03, '2', '@', '€', '²');
    key!(0x04, '3', '#', '£', '³');
    key!(0x05, '4', '$', '¢', '¤');
    key!(0x06, '5', '%', '‰', '½');
    key!(0x07, '6', '^', '¬', '¾');
    key!(0x08, '7', '&', '{', '⅐');
    key!(0x09, '8', '*', '[', '⅛');
    key!(0x0A, '9', '(', ']', '⅜');
    key!(0x0B, '0', ')', '}', '⅝');
    key!(0x0C, '-', '_', '–', '—');
    key!(0x0D, '=', '+', '≠', '±');

    key!(0x10, 'q', 'Q', 'ä', 'Ä');
    key!(0x11, 'w', 'W', 'å', 'Å');
    key!(0x12, 'e', 'E', 'é', 'É');
    key!(0x13, 'r', 'R', '®', 'Ȑ');
    key!(0x14, 't', 'T', 'þ', 'Þ');
    key!(0x15, 'y', 'Y', 'ý', 'Ý');
    key!(0x16, 'u', 'U', 'ú', 'Ú');
    key!(0x17, 'i', 'I', 'í', 'Í');
    key!(0x18, 'o', 'O', 'ó', 'Ó');
    key!(0x19, 'p', 'P', 'ö', 'Ö');

    key!(0x1E, 'a', 'A', 'á', 'Á');
    key!(0x1F, 's', 'S', 'ß', 'Ś');
    key!(0x20, 'd', 'D', 'ð', 'Ð');
    key!(0x21, 'f', 'F', 'ƒ', '₣');
    key!(0x22, 'g', 'G', 'ğ', 'Ğ');
    key!(0x23, 'h', 'H', 'ħ', 'Ħ');
    key!(0x24, 'j', 'J', 'ĵ', 'Ĵ');
    key!(0x25, 'k', 'K', 'ĸ', 'Ǩ');
    key!(0x26, 'l', 'L', 'ł', 'Ł');

    key!(0x2C, 'z', 'Z', 'ž', 'Ž');
    key!(0x2D, 'x', 'X', '×', 'Ξ');
    key!(0x2E, 'c', 'C', 'ç', 'Ç');
    key!(0x2F, 'v', 'V', 'ʌ', '∇');
    key!(0x30, 'b', 'B', 'β', 'ß');
    key!(0x31, 'n', 'N', 'ñ', 'Ñ');
    key!(0x32, 'm', 'M', 'µ', '—');

    key!(0x39, ' ', ' ', ' ', '␣');

    map
};
