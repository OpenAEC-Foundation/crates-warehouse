use std::collections::HashMap;

/// Map a PostScript glyph name to its Unicode character.
pub fn glyph_name_to_unicode(name: &str) -> Option<char> {
    match name {
        // Uppercase letters
        "A" => Some('A'), "B" => Some('B'), "C" => Some('C'), "D" => Some('D'),
        "E" => Some('E'), "F" => Some('F'), "G" => Some('G'), "H" => Some('H'),
        "I" => Some('I'), "J" => Some('J'), "K" => Some('K'), "L" => Some('L'),
        "M" => Some('M'), "N" => Some('N'), "O" => Some('O'), "P" => Some('P'),
        "Q" => Some('Q'), "R" => Some('R'), "S" => Some('S'), "T" => Some('T'),
        "U" => Some('U'), "V" => Some('V'), "W" => Some('W'), "X" => Some('X'),
        "Y" => Some('Y'), "Z" => Some('Z'),
        // Lowercase letters
        "a" => Some('a'), "b" => Some('b'), "c" => Some('c'), "d" => Some('d'),
        "e" => Some('e'), "f" => Some('f'), "g" => Some('g'), "h" => Some('h'),
        "i" => Some('i'), "j" => Some('j'), "k" => Some('k'), "l" => Some('l'),
        "m" => Some('m'), "n" => Some('n'), "o" => Some('o'), "p" => Some('p'),
        "q" => Some('q'), "r" => Some('r'), "s" => Some('s'), "t" => Some('t'),
        "u" => Some('u'), "v" => Some('v'), "w" => Some('w'), "x" => Some('x'),
        "y" => Some('y'), "z" => Some('z'),
        // Digits
        "zero" => Some('0'), "one" => Some('1'), "two" => Some('2'),
        "three" => Some('3'), "four" => Some('4'), "five" => Some('5'),
        "six" => Some('6'), "seven" => Some('7'), "eight" => Some('8'),
        "nine" => Some('9'),
        // Punctuation and common symbols
        "space" => Some(' '),
        "exclam" => Some('!'),
        "quotedbl" => Some('"'),
        "numbersign" => Some('#'),
        "dollar" => Some('$'),
        "percent" => Some('%'),
        "ampersand" => Some('&'),
        "quotesingle" => Some('\''),
        "parenleft" => Some('('),
        "parenright" => Some(')'),
        "asterisk" => Some('*'),
        "plus" => Some('+'),
        "comma" => Some(','),
        "hyphen" => Some('-'),
        "period" => Some('.'),
        "slash" => Some('/'),
        "colon" => Some(':'),
        "semicolon" => Some(';'),
        "less" => Some('<'),
        "equal" => Some('='),
        "greater" => Some('>'),
        "question" => Some('?'),
        "at" => Some('@'),
        "bracketleft" => Some('['),
        "backslash" => Some('\\'),
        "bracketright" => Some(']'),
        "asciicircum" => Some('^'),
        "underscore" => Some('_'),
        "grave" => Some('`'),
        "braceleft" => Some('{'),
        "bar" => Some('|'),
        "braceright" => Some('}'),
        "asciitilde" => Some('~'),
        // Special characters
        "degree" => Some('\u{00B0}'),
        "plusminus" => Some('\u{00B1}'),
        "twosuperior" => Some('\u{00B2}'),
        "threesuperior" => Some('\u{00B3}'),
        "multiply" => Some('\u{00D7}'),
        "divide" => Some('\u{00F7}'),
        "Euro" => Some('\u{20AC}'),
        "bullet" => Some('\u{2022}'),
        "endash" => Some('\u{2013}'),
        "emdash" => Some('\u{2014}'),
        "ellipsis" => Some('\u{2026}'),
        "trademark" => Some('\u{2122}'),
        "copyright" => Some('\u{00A9}'),
        "registered" => Some('\u{00AE}'),
        // Quotes
        "quotedblleft" => Some('\u{201C}'),
        "quotedblright" => Some('\u{201D}'),
        "quoteleft" => Some('\u{2018}'),
        "quoteright" => Some('\u{2019}'),
        "guillemotleft" => Some('\u{00AB}'),
        "guillemotright" => Some('\u{00BB}'),
        "guilsinglleft" => Some('\u{2039}'),
        "guilsinglright" => Some('\u{203A}'),
        "quotesinglbase" => Some('\u{201A}'),
        "quotedblbase" => Some('\u{201E}'),
        // Ligatures
        "fi" => Some('\u{FB01}'),
        "fl" => Some('\u{FB02}'),
        // Accented characters commonly used
        "Agrave" => Some('\u{00C0}'), "Aacute" => Some('\u{00C1}'),
        "Acircumflex" => Some('\u{00C2}'), "Atilde" => Some('\u{00C3}'),
        "Adieresis" => Some('\u{00C4}'), "Aring" => Some('\u{00C5}'),
        "AE" => Some('\u{00C6}'), "Ccedilla" => Some('\u{00C7}'),
        "Egrave" => Some('\u{00C8}'), "Eacute" => Some('\u{00C9}'),
        "Ecircumflex" => Some('\u{00CA}'), "Edieresis" => Some('\u{00CB}'),
        "Igrave" => Some('\u{00CC}'), "Iacute" => Some('\u{00CD}'),
        "Icircumflex" => Some('\u{00CE}'), "Idieresis" => Some('\u{00CF}'),
        "Eth" => Some('\u{00D0}'), "Ntilde" => Some('\u{00D1}'),
        "Ograve" => Some('\u{00D2}'), "Oacute" => Some('\u{00D3}'),
        "Ocircumflex" => Some('\u{00D4}'), "Otilde" => Some('\u{00D5}'),
        "Odieresis" => Some('\u{00D6}'), "Oslash" => Some('\u{00D8}'),
        "Ugrave" => Some('\u{00D9}'), "Uacute" => Some('\u{00DA}'),
        "Ucircumflex" => Some('\u{00DB}'), "Udieresis" => Some('\u{00DC}'),
        "Yacute" => Some('\u{00DD}'), "Thorn" => Some('\u{00DE}'),
        "germandbls" => Some('\u{00DF}'),
        "agrave" => Some('\u{00E0}'), "aacute" => Some('\u{00E1}'),
        "acircumflex" => Some('\u{00E2}'), "atilde" => Some('\u{00E3}'),
        "adieresis" => Some('\u{00E4}'), "aring" => Some('\u{00E5}'),
        "ae" => Some('\u{00E6}'), "ccedilla" => Some('\u{00E7}'),
        "egrave" => Some('\u{00E8}'), "eacute" => Some('\u{00E9}'),
        "ecircumflex" => Some('\u{00EA}'), "edieresis" => Some('\u{00EB}'),
        "igrave" => Some('\u{00EC}'), "iacute" => Some('\u{00ED}'),
        "icircumflex" => Some('\u{00EE}'), "idieresis" => Some('\u{00EF}'),
        "eth" => Some('\u{00F0}'), "ntilde" => Some('\u{00F1}'),
        "ograve" => Some('\u{00F2}'), "oacute" => Some('\u{00F3}'),
        "ocircumflex" => Some('\u{00F4}'), "otilde" => Some('\u{00F5}'),
        "odieresis" => Some('\u{00F6}'), "oslash" => Some('\u{00F8}'),
        "ugrave" => Some('\u{00F9}'), "uacute" => Some('\u{00FA}'),
        "ucircumflex" => Some('\u{00FB}'), "udieresis" => Some('\u{00FC}'),
        "yacute" => Some('\u{00FD}'), "thorn" => Some('\u{00FE}'),
        "ydieresis" => Some('\u{00FF}'),
        // Currency and misc
        "cent" => Some('\u{00A2}'),
        "sterling" => Some('\u{00A3}'),
        "currency" => Some('\u{00A4}'),
        "yen" => Some('\u{00A5}'),
        "brokenbar" => Some('\u{00A6}'),
        "section" => Some('\u{00A7}'),
        "dieresis" => Some('\u{00A8}'),
        "ordfeminine" => Some('\u{00AA}'),
        "logicalnot" => Some('\u{00AC}'),
        "softhyphen" => Some('\u{00AD}'),
        "macron" => Some('\u{00AF}'),
        "mu" => Some('\u{00B5}'),
        "paragraph" => Some('\u{00B6}'),
        "periodcentered" => Some('\u{00B7}'),
        "cedilla" => Some('\u{00B8}'),
        "onesuperior" => Some('\u{00B9}'),
        "ordmasculine" => Some('\u{00BA}'),
        "onequarter" => Some('\u{00BC}'),
        "onehalf" => Some('\u{00BD}'),
        "threequarters" => Some('\u{00BE}'),
        "questiondown" => Some('\u{00BF}'),
        "exclamdown" => Some('\u{00A1}'),
        // Dashes and whitespace
        "minus" => Some('\u{2212}'),
        "nbspace" => Some('\u{00A0}'),
        // Math
        "lozenge" => Some('\u{25CA}'),
        "infinity" => Some('\u{221E}'),
        "notequal" => Some('\u{2260}'),
        "lessequal" => Some('\u{2264}'),
        "greaterequal" => Some('\u{2265}'),
        "approxequal" => Some('\u{2248}'),
        // Misc
        "dagger" => Some('\u{2020}'),
        "daggerdbl" => Some('\u{2021}'),
        "perthousand" => Some('\u{2030}'),
        _ => None,
    }
}

/// Decode a Windows-1252 byte in the 0x80-0x9F range to Unicode.
pub fn win_ansi_decode(code: u8) -> char {
    match code {
        0x80 => '\u{20AC}', // Euro
        0x82 => '\u{201A}', // quotesinglbase
        0x83 => '\u{0192}', // florin
        0x84 => '\u{201E}', // quotedblbase
        0x85 => '\u{2026}', // ellipsis
        0x86 => '\u{2020}', // dagger
        0x87 => '\u{2021}', // daggerdbl
        0x88 => '\u{02C6}', // circumflex
        0x89 => '\u{2030}', // perthousand
        0x8A => '\u{0160}', // Scaron
        0x8B => '\u{2039}', // guilsinglleft
        0x8C => '\u{0152}', // OE
        0x8E => '\u{017D}', // Zcaron
        0x91 => '\u{2018}', // quoteleft
        0x92 => '\u{2019}', // quoteright
        0x93 => '\u{201C}', // quotedblleft
        0x94 => '\u{201D}', // quotedblright
        0x95 => '\u{2022}', // bullet
        0x96 => '\u{2013}', // endash
        0x97 => '\u{2014}', // emdash
        0x98 => '\u{02DC}', // tilde
        0x99 => '\u{2122}', // trademark
        0x9A => '\u{0161}', // scaron
        0x9B => '\u{203A}', // guilsinglright
        0x9C => '\u{0153}', // oe
        0x9E => '\u{017E}', // zcaron
        0x9F => '\u{0178}', // Ydieresis
        _ => code as char,
    }
}

/// Parse a PDF Differences array into a map of char_code -> glyph_name.
/// The array alternates between integer codes and glyph name entries.
/// e.g. [24 /breve /caron 30 /ring /dotaccent]
pub fn parse_differences(arr: &[lopdf::Object]) -> HashMap<u8, String> {
    let mut map = HashMap::new();
    let mut code: u8 = 0;

    for obj in arr {
        match obj {
            lopdf::Object::Integer(i) => {
                code = *i as u8;
            }
            lopdf::Object::Name(name) => {
                let name_str = String::from_utf8_lossy(name).to_string();
                map.insert(code, name_str);
                code = code.wrapping_add(1);
            }
            _ => {}
        }
    }

    map
}

/// Resolve a character code to Unicode using encoding name and differences.
pub fn resolve_char_code(
    encoding_name: Option<&str>,
    differences: &HashMap<u8, String>,
    char_code: u8,
) -> char {
    // First check Differences override
    if let Some(glyph_name) = differences.get(&char_code) {
        if glyph_name == ".notdef" {
            return '\u{FFFD}';
        }
        if let Some(ch) = glyph_name_to_unicode(glyph_name) {
            return ch;
        }
        // Try to interpret as uniXXXX
        if glyph_name.starts_with("uni") && glyph_name.len() == 7 {
            if let Ok(val) = u32::from_str_radix(&glyph_name[3..], 16) {
                if let Some(ch) = char::from_u32(val) {
                    return ch;
                }
            }
        }
    }

    // Apply base encoding
    match encoding_name {
        Some("WinAnsiEncoding") => {
            if (0x80..=0x9F).contains(&char_code) {
                win_ansi_decode(char_code)
            } else {
                char_code as char
            }
        }
        Some("MacRomanEncoding") => {
            // For MacRoman, 0x00-0x7F same as ASCII; 0x80+ has differences
            // Simplified: use the byte as Latin-1 for now
            char_code as char
        }
        _ => {
            // Default: treat as WinAnsiEncoding (most common in modern PDFs)
            if (0x80..=0x9F).contains(&char_code) {
                win_ansi_decode(char_code)
            } else {
                char_code as char
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_name_to_unicode() {
        assert_eq!(glyph_name_to_unicode("A"), Some('A'));
        assert_eq!(glyph_name_to_unicode("space"), Some(' '));
        assert_eq!(glyph_name_to_unicode("endash"), Some('\u{2013}'));
        assert_eq!(glyph_name_to_unicode("fi"), Some('\u{FB01}'));
        assert_eq!(glyph_name_to_unicode("Euro"), Some('\u{20AC}'));
        assert_eq!(glyph_name_to_unicode("nonexistent"), None);
    }

    #[test]
    fn test_win_ansi_decode() {
        assert_eq!(win_ansi_decode(0x80), '\u{20AC}');
        assert_eq!(win_ansi_decode(0x93), '\u{201C}');
        assert_eq!(win_ansi_decode(0x41), 'A');
    }

    #[test]
    fn test_parse_differences() {
        let arr = vec![
            lopdf::Object::Integer(65),
            lopdf::Object::Name(b"Aacute".to_vec()),
            lopdf::Object::Name(b"Acircumflex".to_vec()),
            lopdf::Object::Integer(200),
            lopdf::Object::Name(b"endash".to_vec()),
        ];
        let map = parse_differences(&arr);
        assert_eq!(map.get(&65), Some(&"Aacute".to_string()));
        assert_eq!(map.get(&66), Some(&"Acircumflex".to_string()));
        assert_eq!(map.get(&200), Some(&"endash".to_string()));
    }

    #[test]
    fn test_resolve_char_code() {
        let mut diffs = HashMap::new();
        diffs.insert(65, "bullet".to_string());

        // Differences override
        assert_eq!(resolve_char_code(Some("WinAnsiEncoding"), &diffs, 65), '\u{2022}');
        // WinAnsi for 0x80
        assert_eq!(resolve_char_code(Some("WinAnsiEncoding"), &HashMap::new(), 0x80), '\u{20AC}');
        // Plain ASCII
        assert_eq!(resolve_char_code(Some("WinAnsiEncoding"), &HashMap::new(), 0x41), 'A');
    }
}
