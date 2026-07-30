//! Streamende content-stream-lexer: levert operaties één voor één in een
//! HERBRUIKBARE `lopdf::content::Operation`, in plaats van de volledige
//! stream als `Vec<Operation>` te materialiseren zoals `Content::decode`.
//!
//! Waarom: op een 139 MB-content-stream met ~5M operaties bouwt de
//! lopdf/pom-route 15-20M heap-objecten (gemeten: 8,1 GB piek-RSS, 25 s).
//! Deze lexer alloceert per operatie vrijwel niets — getallen zijn inline
//! `Object::Integer/Real`, en de operand-Vec en operator-String worden per
//! stap hergebruikt.
//!
//! Extra t.o.v. lopdf: correcte afhandeling van inline images (`BI … ID
//! <raw bytes> EI`) — de rauwe beelddata wordt als geheel overgeslagen in
//! plaats van als losse pseudo-tokens gelexed, zodat de parser op
//! CAD-bestanden met duizenden inline strips niet ontspoort.

use lopdf::content::Operation;
use lopdf::{Dictionary, Object, StringFormat};

pub struct ContentStreamIter<'a> {
    bytes: &'a [u8],
    pos: usize,
    /// Byte-span (start, eind) van de rauwe data van de laatst gelexte
    /// inline image (tussen `ID` en `EI`), zodat de interpreter hem als
    /// DrawImage kan emitten. Overschreven bij elke volgende `ID`.
    last_inline: Option<(usize, usize)>,
}

#[inline(always)]
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'\x0C' | b'\0')
}

#[inline(always)]
fn is_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

impl<'a> ContentStreamIter<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        ContentStreamIter { bytes, pos: 0, last_inline: None }
    }

    /// Byte-span (start, eind — exclusief) in de invoer-slice van de rauwe
    /// inline-image-data van de laatst geleverde `ID`-operatie. De aanroeper
    /// slicet zelf: de span verwijst naar dezelfde bytes als `new()` kreeg.
    pub fn inline_image_span(&self) -> Option<(usize, usize)> {
        self.last_inline
    }

    #[inline(always)]
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws_and_comments(&mut self) {
        while let Some(b) = self.peek() {
            if is_ws(b) {
                self.pos += 1;
            } else if b == b'%' {
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Parse het volgende token als operand-Object. Retourneert None als het
    /// volgende token een operator is (of einde stream).
    fn parse_operand(&mut self) -> Option<Object> {
        self.skip_ws_and_comments();
        let b = self.peek()?;
        match b {
            b'0'..=b'9' | b'+' | b'-' | b'.' => Some(self.parse_number()),
            b'/' => Some(self.parse_name()),
            b'(' => Some(self.parse_literal_string()),
            b'<' => {
                if self.bytes.get(self.pos + 1) == Some(&b'<') {
                    Some(Object::Dictionary(self.parse_dict()))
                } else {
                    Some(self.parse_hex_string())
                }
            }
            b'[' => Some(self.parse_array()),
            _ => None, // operator, of iets onbekends dat de aanroeper afhandelt
        }
    }

    fn parse_number(&mut self) -> Object {
        let start = self.pos;
        let mut is_real = false;
        while let Some(b) = self.peek() {
            match b {
                b'0'..=b'9' | b'+' | b'-' => self.pos += 1,
                b'.' => {
                    is_real = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        // Veilig: alleen ASCII-cijfers/tekens in de slice.
        let s = unsafe { std::str::from_utf8_unchecked(&self.bytes[start..self.pos]) };
        if is_real {
            Object::Real(s.parse::<f32>().unwrap_or(0.0))
        } else {
            match s.parse::<i64>() {
                Ok(v) => Object::Integer(v),
                // bv. "--3" of los "+": permissief naar 0, zoals viewers doen
                Err(_) => Object::Real(s.parse::<f32>().unwrap_or(0.0)),
            }
        }
    }

    fn parse_name(&mut self) -> Object {
        self.pos += 1; // '/'
        let mut out: Vec<u8> = Vec::with_capacity(16);
        while let Some(b) = self.peek() {
            if is_ws(b) || is_delim(b) {
                break;
            }
            if b == b'#' {
                let h1 = self.bytes.get(self.pos + 1).copied();
                let h2 = self.bytes.get(self.pos + 2).copied();
                if let (Some(h1), Some(h2)) = (h1, h2) {
                    let hv = (hex_val(h1), hex_val(h2));
                    if let (Some(a), Some(c)) = hv {
                        out.push(a * 16 + c);
                        self.pos += 3;
                        continue;
                    }
                }
            }
            out.push(b);
            self.pos += 1;
        }
        Object::Name(out)
    }

    fn parse_literal_string(&mut self) -> Object {
        self.pos += 1; // '('
        let mut out: Vec<u8> = Vec::with_capacity(32);
        let mut depth = 1usize;
        while let Some(b) = self.peek() {
            self.pos += 1;
            match b {
                b'\\' => {
                    let Some(e) = self.peek() else { break };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'(' => out.push(b'('),
                        b')' => out.push(b')'),
                        b'\\' => out.push(b'\\'),
                        b'\r' => {
                            // regel-continuatie: \CRLF of \CR
                            if self.peek() == Some(b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = (e - b'0') as u32;
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + (d - b'0') as u32;
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xFF) as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'(' => {
                    depth += 1;
                    out.push(b'(');
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(b')');
                }
                other => out.push(other),
            }
        }
        Object::String(out, StringFormat::Literal)
    }

    fn parse_hex_string(&mut self) -> Object {
        self.pos += 1; // '<'
        let mut out: Vec<u8> = Vec::with_capacity(32);
        let mut hi: Option<u8> = None;
        while let Some(b) = self.peek() {
            self.pos += 1;
            if b == b'>' {
                break;
            }
            if let Some(v) = hex_val(b) {
                match hi {
                    None => hi = Some(v),
                    Some(h) => {
                        out.push(h * 16 + v);
                        hi = None;
                    }
                }
            }
        }
        if let Some(h) = hi {
            out.push(h * 16); // oneven aantal: laatste nibble → x0
        }
        Object::String(out, StringFormat::Hexadecimal)
    }

    fn parse_array(&mut self) -> Object {
        self.pos += 1; // '['
        let mut out: Vec<Object> = Vec::new();
        loop {
            self.skip_ws_and_comments();
            match self.peek() {
                None => break,
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => {
                    if let Some(obj) = self.parse_operand() {
                        out.push(obj);
                    } else if let Some(kw) = self.try_keyword() {
                        out.push(kw);
                    } else {
                        // onbekend token binnen array: token overslaan
                        self.skip_token();
                    }
                }
            }
        }
        Object::Array(out)
    }

    fn parse_dict(&mut self) -> Dictionary {
        self.pos += 2; // '<<'
        let mut dict = Dictionary::new();
        loop {
            self.skip_ws_and_comments();
            match self.peek() {
                None => break,
                Some(b'>') => {
                    if self.bytes.get(self.pos + 1) == Some(&b'>') {
                        self.pos += 2;
                        break;
                    }
                    self.pos += 1;
                }
                Some(b'/') => {
                    let key = match self.parse_name() {
                        Object::Name(k) => k,
                        _ => unreachable!(),
                    };
                    self.skip_ws_and_comments();
                    let val = if let Some(v) = self.parse_operand() {
                        v
                    } else if let Some(kw) = self.try_keyword() {
                        kw
                    } else {
                        self.skip_token();
                        Object::Null
                    };
                    dict.set(key, val);
                }
                Some(_) => self.skip_token(),
            }
        }
        dict
    }

    /// true/false/null als operand-keyword.
    fn try_keyword(&mut self) -> Option<Object> {
        let rest = &self.bytes[self.pos..];
        let kw_end = |n: usize| {
            rest.get(n).map_or(true, |&b| is_ws(b) || is_delim(b))
        };
        if rest.starts_with(b"true") && kw_end(4) {
            self.pos += 4;
            return Some(Object::Boolean(true));
        }
        if rest.starts_with(b"false") && kw_end(5) {
            self.pos += 5;
            return Some(Object::Boolean(false));
        }
        if rest.starts_with(b"null") && kw_end(4) {
            self.pos += 4;
            return Some(Object::Null);
        }
        None
    }

    fn skip_token(&mut self) {
        while let Some(b) = self.peek() {
            if is_ws(b) || is_delim(b) {
                if is_delim(b) && self.pos_token_start() {
                    self.pos += 1; // losse delimiter: één byte verder
                }
                break;
            }
            self.pos += 1;
        }
    }

    #[inline(always)]
    fn pos_token_start(&self) -> bool {
        true
    }

    /// Lees een operator-token (regular characters).
    fn read_operator_into(&mut self, out: &mut String) -> bool {
        self.skip_ws_and_comments();
        let start = self.pos;
        while let Some(b) = self.peek() {
            // Operators zijn 'regular characters'; ' en " zijn zelf operators.
            if is_ws(b) || is_delim(b) {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start {
            // Mogelijk een losse delimiter die geen operand kon zijn (bv. '}');
            // sla hem over zodat we niet blijven hangen.
            if self.peek().is_some() {
                self.pos += 1;
                out.clear();
                return true; // lege operator → aanroeper negeert en gaat door
            }
            return false;
        }
        out.clear();
        // Operators zijn ASCII; lossy is hier veilig en alloc-vrij bij ASCII.
        out.push_str(&String::from_utf8_lossy(&self.bytes[start..self.pos]));
        true
    }

    /// Sla een inline image over: aangeroepen NA het lexen van operator `ID`.
    /// Scant naar `EI` op een token-grens. De rauwe data-span wordt in
    /// `last_inline` bewaard zodat de interpreter hem kan emitten.
    fn skip_inline_image_data(&mut self) {
        // Eén whitespace-byte na ID hoort bij de syntax.
        if self.peek().map(is_ws) == Some(true) {
            self.pos += 1;
        }
        let start = self.pos;
        while self.pos + 1 < self.bytes.len() {
            if self.bytes[self.pos] == b'E' && self.bytes[self.pos + 1] == b'I' {
                let before_ok = self.pos == 0 || is_ws(self.bytes[self.pos - 1]);
                let after_ok = self
                    .bytes
                    .get(self.pos + 2)
                    .map_or(true, |&b| is_ws(b) || is_delim(b));
                if before_ok && after_ok {
                    // Data eindigt vóór de whitespace die bij de EI-syntax
                    // hoort (before_ok garandeert die ws, behalve op pos 0).
                    let end = if self.pos > start { self.pos - 1 } else { start };
                    self.last_inline = Some((start, end));
                    self.pos += 2; // voorbij EI
                    return;
                }
            }
            self.pos += 1;
        }
        self.last_inline = Some((start, self.bytes.len()));
        self.pos = self.bytes.len();
    }

    /// Vul `op` met de volgende operatie. Retourneert false bij einde stream.
    /// De operand-Vec en operator-String van `op` worden hergebruikt.
    pub fn next_into(&mut self, op: &mut Operation) -> bool {
        op.operands.clear();
        loop {
            self.skip_ws_and_comments();
            if self.peek().is_none() {
                return false;
            }
            if let Some(obj) = self.parse_operand() {
                op.operands.push(obj);
                continue;
            }
            if let Some(kw) = self.try_keyword() {
                op.operands.push(kw);
                continue;
            }
            if !self.read_operator_into(&mut op.operator) {
                return false;
            }
            if op.operator.is_empty() {
                // los delimiter-token overgeslagen; operanden laten staan en door
                continue;
            }
            if op.operator == "ID" {
                // Inline-image-data volgt rauw: overslaan tot EI. We leveren
                // de ID-operatie (operanden = de BI-dict-paren die als losse
                // operanden binnenkwamen); de interpreter negeert hem.
                self.skip_inline_image_data();
            }
            return true;
        }
    }
}

#[inline(always)]
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ops(src: &[u8]) -> Vec<(String, Vec<Object>)> {
        let mut it = ContentStreamIter::new(src);
        let mut op = Operation {
            operator: String::new(),
            operands: Vec::new(),
        };
        let mut out = Vec::new();
        while it.next_into(&mut op) {
            out.push((op.operator.clone(), op.operands.clone()));
        }
        out
    }

    #[test]
    fn parses_numbers_and_operators() {
        let v = ops(b"1 0 0 1 10.5 -20 cm\n0.5 w\nq Q");
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].0, "cm");
        assert_eq!(v[0].1.len(), 6);
        assert!(matches!(v[0].1[4], Object::Real(x) if (x - 10.5).abs() < 1e-6));
        assert!(matches!(v[0].1[5], Object::Integer(-20)));
        assert_eq!(v[1].0, "w");
        assert_eq!(v[2].0, "q");
        assert_eq!(v[3].0, "Q");
    }

    #[test]
    fn parses_names_strings_arrays() {
        let v = ops(b"/F1 12 Tf [(Hel\\)lo) -120 <48693F>] TJ (a(b)c) Tj");
        assert_eq!(v[0].0, "Tf");
        assert_eq!(v[0].1[0], Object::Name(b"F1".to_vec()));
        assert_eq!(v[1].0, "TJ");
        if let Object::Array(a) = &v[1].1[0] {
            assert_eq!(a.len(), 3);
            assert_eq!(a[0], Object::String(b"Hel)lo".to_vec(), StringFormat::Literal));
            assert_eq!(a[2], Object::String(b"Hi?".to_vec(), StringFormat::Hexadecimal));
        } else {
            panic!("geen array");
        }
        assert_eq!(v[2].1[0], Object::String(b"a(b)c".to_vec(), StringFormat::Literal));
    }

    #[test]
    fn skips_inline_image_raw_data() {
        // BI-dict-tokens komen als losse operanden binnen; na ID volgt rauwe
        // data (met valse haakjes!) tot EI op een token-grens; daarna gewoon door.
        let src = b"BI /W 16 /H 1 /BPC 8 ID \x00\x01(\xFF\x02 EIX EI\n1 0 0 1 5 5 cm";
        let v = ops(src);
        let names: Vec<&str> = v.iter().map(|(o, _)| o.as_str()).collect();
        assert!(names.contains(&"BI"));
        assert!(names.contains(&"ID"));
        assert_eq!(*names.last().unwrap(), "cm");
    }

    #[test]
    fn inline_image_span_covers_raw_data() {
        // De span begint direct na de ene ws-byte achter ID en eindigt vóór
        // de ws-byte die bij EI hoort — dus precies de rauwe data.
        let src = b"BI /W 3 /H 1 /BPC 8 /CS/G ID \xAA\xBB\xCC EI 1 0 0 1 5 5 cm";
        let mut it = ContentStreamIter::new(src);
        let mut op = Operation { operator: String::new(), operands: Vec::new() };
        let mut span = None;
        while it.next_into(&mut op) {
            if op.operator == "ID" {
                span = it.inline_image_span();
                // ID-operanden = de BI-dict-paren
                assert!(op.operands.iter().any(|o| *o == Object::Name(b"BPC".to_vec())));
            }
        }
        let (s, e) = span.expect("span gezet");
        assert_eq!(&src[s..e], b"\xAA\xBB\xCC");
    }

    #[test]
    fn parses_dicts_and_comments() {
        let v = ops(b"% commentaar\n/OC <</Type /OCG /On true>> BDC 1 0 0 RG");
        assert_eq!(v[0].0, "BDC");
        assert!(matches!(v[0].1[1], Object::Dictionary(_)));
        assert_eq!(v[1].0, "RG");
    }

    #[test]
    fn tolerates_garbage_tokens() {
        let v = ops(b"} { 1 0 0 1 0 0 cm");
        assert_eq!(v.last().unwrap().0, "cm");
        assert_eq!(v.last().unwrap().1.len(), 6);
    }
}
