//! Minimal JSON, no dependencies — just what the Language Server needs.
//! Recursive parser + serializer. Preserves UTF-8 in strings (accumulates bytes).

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        if let Json::Obj(fields) = self {
            for (k, v) in fields {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    /// Serializes to compact JSON.
    pub fn to_string(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s
    }
    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    out.push_str(&(*n as i64).to_string());
                } else {
                    out.push_str(&n.to_string());
                }
            }
            Json::Str(s) => write_str(s, out),
            Json::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(fields) => {
                out.push('{');
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses a JSON string. Returns `None` if invalid.
pub fn parse(s: &str) -> Option<Json> {
    let mut p = P { b: s.as_bytes(), i: 0 };
    p.ws();
    let v = p.value()?;
    Some(v)
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Option<Json> {
        self.ws();
        if self.i >= self.b.len() {
            return None;
        }
        match self.b[self.i] {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => Some(Json::Str(self.string()?)),
            b't' => self.lit("true").map(|_| Json::Bool(true)),
            b'f' => self.lit("false").map(|_| Json::Bool(false)),
            b'n' => self.lit("null").map(|_| Json::Null),
            _ => self.number(),
        }
    }
    fn lit(&mut self, s: &str) -> Option<()> {
        if self.b[self.i..].starts_with(s.as_bytes()) {
            self.i += s.len();
            Some(())
        } else {
            None
        }
    }
    fn object(&mut self) -> Option<Json> {
        self.i += 1; // {
        let mut fields = Vec::new();
        self.ws();
        if self.i < self.b.len() && self.b[self.i] == b'}' {
            self.i += 1;
            return Some(Json::Obj(fields));
        }
        loop {
            self.ws();
            let key = self.string()?;
            self.ws();
            if self.i >= self.b.len() || self.b[self.i] != b':' {
                return None;
            }
            self.i += 1;
            let val = self.value()?;
            fields.push((key, val));
            self.ws();
            match self.b.get(self.i)? {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    break;
                }
                _ => return None,
            }
        }
        Some(Json::Obj(fields))
    }
    fn array(&mut self) -> Option<Json> {
        self.i += 1; // [
        let mut items = Vec::new();
        self.ws();
        if self.i < self.b.len() && self.b[self.i] == b']' {
            self.i += 1;
            return Some(Json::Arr(items));
        }
        loop {
            let v = self.value()?;
            items.push(v);
            self.ws();
            match self.b.get(self.i)? {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    break;
                }
                _ => return None,
            }
        }
        Some(Json::Arr(items))
    }
    fn string(&mut self) -> Option<String> {
        if self.b.get(self.i)? != &b'"' {
            return None;
        }
        self.i += 1;
        let mut bytes: Vec<u8> = Vec::new();
        while self.i < self.b.len() {
            let c = self.b[self.i];
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(bytes).ok(),
                b'\\' => {
                    let e = *self.b.get(self.i)?;
                    self.i += 1;
                    match e {
                        b'"' => bytes.push(b'"'),
                        b'\\' => bytes.push(b'\\'),
                        b'/' => bytes.push(b'/'),
                        b'n' => bytes.push(b'\n'),
                        b't' => bytes.push(b'\t'),
                        b'r' => bytes.push(b'\r'),
                        b'b' => bytes.push(0x08),
                        b'f' => bytes.push(0x0C),
                        b'u' => {
                            if self.i + 4 > self.b.len() {
                                return None;
                            }
                            let hex = std::str::from_utf8(&self.b[self.i..self.i + 4]).ok()?;
                            let cp = u32::from_str_radix(hex, 16).ok()?;
                            self.i += 4;
                            let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                            let mut buf = [0u8; 4];
                            bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return None,
                    }
                }
                _ => bytes.push(c), // raw byte (preserves multibyte UTF-8)
            }
        }
        None
    }
    fn number(&mut self) -> Option<Json> {
        let start = self.i;
        while self.i < self.b.len()
            && matches!(self.b[self.i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'-' | b'+')
        {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).ok()?;
        s.parse::<f64>().ok().map(Json::Num)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_object() {
        let j = parse(r#"{"a":1,"b":"x","c":[true,null,2.5]}"#).unwrap();
        assert_eq!(j.get("a"), Some(&Json::Num(1.0)));
        assert_eq!(j.get("b").and_then(|v| v.as_str()), Some("x"));
        assert_eq!(j.get("c").and_then(|v| v.as_array()).map(|a| a.len()), Some(3));
    }

    #[test]
    fn preserves_utf8_and_escapes() {
        let j = parse(r#"{"t":"olá\n\"x\""}"#).unwrap();
        assert_eq!(j.get("t").and_then(|v| v.as_str()), Some("olá\n\"x\""));
    }

    #[test]
    fn serializes_with_escaping() {
        let j = Json::Obj(vec![("m".into(), Json::Str("a\"b\nç".into()))]);
        assert_eq!(j.to_string(), r#"{"m":"a\"b\nç"}"#);
    }
}
