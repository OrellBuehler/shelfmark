use std::borrow::Cow;

#[derive(Debug, Clone)]
pub enum Node {
    Element(Element),
    Text(String),
    Raw(String),
}

#[derive(Debug, Clone, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub pre: String,
    pub root: Element,
}

pub fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

pub fn prefix(name: &str) -> Option<&str> {
    name.rsplit_once(':').map(|(p, _)| p)
}

impl Element {
    pub fn new(name: impl Into<String>) -> Element {
        Element {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn local(&self) -> &str {
        local(&self.name)
    }

    pub fn is(&self, local_name: &str) -> bool {
        self.local() == local_name
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .or_else(|| self.attrs.iter().find(|(k, _)| local(k) == name))
            .map(|(_, v)| v.as_str())
    }

    pub fn set_attr(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(a) = self.attrs.iter_mut().find(|(k, _)| k == name) {
            a.1 = value;
        } else {
            self.attrs.push((name.to_string(), value));
        }
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn child(&self, local_name: &str) -> Option<&Element> {
        self.elements().find(|e| e.is(local_name))
    }

    pub fn children_named<'a>(&'a self, local_name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.elements().filter(move |e| e.is(local_name))
    }

    pub fn child_mut(&mut self, local_name: &str) -> Option<&mut Element> {
        self.children.iter_mut().find_map(|n| match n {
            Node::Element(e) if e.is(local_name) => Some(e),
            _ => None,
        })
    }

    pub fn inner_xml(&self) -> String {
        let mut out = String::new();
        for n in &self.children {
            match n {
                Node::Element(c) => write_element(c, &mut out),
                Node::Text(t) => escape_text(t, &mut out),
                Node::Raw(r) => out.push_str(r),
            }
        }
        out
    }

    pub fn find(&self, local_name: &str) -> Option<&Element> {
        for e in self.elements() {
            if e.is(local_name) {
                return Some(e);
            }
            if let Some(f) = e.find(local_name) {
                return Some(f);
            }
        }
        None
    }

    pub fn find_all<'a>(&'a self, local_name: &str, out: &mut Vec<&'a Element>) {
        for e in self.elements() {
            if e.is(local_name) {
                out.push(e);
            }
            e.find_all(local_name, out);
        }
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        self.collect_text(&mut s);
        s
    }

    fn collect_text(&self, out: &mut String) {
        for n in &self.children {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => e.collect_text(out),
                Node::Raw(_) => {}
            }
        }
    }

    pub fn trimmed_text(&self) -> String {
        self.text().trim().to_string()
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.children = vec![Node::Text(text.into())];
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Element {
        self.set_text(text);
        self
    }

    pub fn with_attr(mut self, name: &str, value: impl Into<String>) -> Element {
        self.set_attr(name, value);
        self
    }
}

pub fn decode_bytes(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE
            .decode_without_bom_handling(&bytes[2..])
            .0
            .into_owned();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE
            .decode_without_bom_handling(&bytes[2..])
            .0
            .into_owned();
    }
    if bytes.len() >= 4 && bytes[0] == b'<' && bytes[1] == 0 && bytes[3] == 0 {
        return encoding_rs::UTF_16LE
            .decode_without_bom_handling(bytes)
            .0
            .into_owned();
    }
    if bytes.len() >= 4 && bytes[0] == 0 && bytes[1] == b'<' && bytes[2] == 0 {
        return encoding_rs::UTF_16BE
            .decode_without_bom_handling(bytes)
            .0
            .into_owned();
    }
    let head = &bytes[..bytes.len().min(300)];
    let head_str: String = head
        .iter()
        .map(|&b| if b < 128 { b as char } else { ' ' })
        .collect();
    let declared = head_str.find("encoding=").and_then(|i| {
        let rest = &head_str[i + 9..];
        let q = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let end = rest[1..].find(q)?;
        Some(rest[1..1 + end].to_string())
    });
    if let Some(label) = declared
        && let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes())
        && enc != encoding_rs::UTF_8
    {
        return enc.decode_without_bom_handling(bytes).0.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

const HTML_ENTITIES: &[(&str, char)] = &[
    ("nbsp", '\u{a0}'),
    ("iexcl", '¡'),
    ("cent", '¢'),
    ("pound", '£'),
    ("curren", '¤'),
    ("yen", '¥'),
    ("brvbar", '¦'),
    ("sect", '§'),
    ("uml", '¨'),
    ("copy", '©'),
    ("ordf", 'ª'),
    ("laquo", '«'),
    ("not", '¬'),
    ("shy", '\u{ad}'),
    ("reg", '®'),
    ("macr", '¯'),
    ("deg", '°'),
    ("plusmn", '±'),
    ("sup2", '²'),
    ("sup3", '³'),
    ("acute", '´'),
    ("micro", 'µ'),
    ("para", '¶'),
    ("middot", '·'),
    ("cedil", '¸'),
    ("sup1", '¹'),
    ("ordm", 'º'),
    ("raquo", '»'),
    ("frac14", '¼'),
    ("frac12", '½'),
    ("frac34", '¾'),
    ("iquest", '¿'),
    ("Agrave", 'À'),
    ("Aacute", 'Á'),
    ("Acirc", 'Â'),
    ("Atilde", 'Ã'),
    ("Auml", 'Ä'),
    ("Aring", 'Å'),
    ("AElig", 'Æ'),
    ("Ccedil", 'Ç'),
    ("Egrave", 'È'),
    ("Eacute", 'É'),
    ("Ecirc", 'Ê'),
    ("Euml", 'Ë'),
    ("Igrave", 'Ì'),
    ("Iacute", 'Í'),
    ("Icirc", 'Î'),
    ("Iuml", 'Ï'),
    ("ETH", 'Ð'),
    ("Ntilde", 'Ñ'),
    ("Ograve", 'Ò'),
    ("Oacute", 'Ó'),
    ("Ocirc", 'Ô'),
    ("Otilde", 'Õ'),
    ("Ouml", 'Ö'),
    ("times", '×'),
    ("Oslash", 'Ø'),
    ("Ugrave", 'Ù'),
    ("Uacute", 'Ú'),
    ("Ucirc", 'Û'),
    ("Uuml", 'Ü'),
    ("Yacute", 'Ý'),
    ("THORN", 'Þ'),
    ("szlig", 'ß'),
    ("agrave", 'à'),
    ("aacute", 'á'),
    ("acirc", 'â'),
    ("atilde", 'ã'),
    ("auml", 'ä'),
    ("aring", 'å'),
    ("aelig", 'æ'),
    ("ccedil", 'ç'),
    ("egrave", 'è'),
    ("eacute", 'é'),
    ("ecirc", 'ê'),
    ("euml", 'ë'),
    ("igrave", 'ì'),
    ("iacute", 'í'),
    ("icirc", 'î'),
    ("iuml", 'ï'),
    ("eth", 'ð'),
    ("ntilde", 'ñ'),
    ("ograve", 'ò'),
    ("oacute", 'ó'),
    ("ocirc", 'ô'),
    ("otilde", 'õ'),
    ("ouml", 'ö'),
    ("divide", '÷'),
    ("oslash", 'ø'),
    ("ugrave", 'ù'),
    ("uacute", 'ú'),
    ("ucirc", 'û'),
    ("uuml", 'ü'),
    ("yacute", 'ý'),
    ("thorn", 'þ'),
    ("yuml", 'ÿ'),
    ("OElig", 'Œ'),
    ("oelig", 'œ'),
    ("Scaron", 'Š'),
    ("scaron", 'š'),
    ("Yuml", 'Ÿ'),
    ("circ", 'ˆ'),
    ("tilde", '˜'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
    ("zwnj", '\u{200c}'),
    ("zwj", '\u{200d}'),
    ("lrm", '\u{200e}'),
    ("rlm", '\u{200f}'),
    ("ndash", '–'),
    ("mdash", '—'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("sbquo", '‚'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("bdquo", '„'),
    ("dagger", '†'),
    ("Dagger", '‡'),
    ("bull", '•'),
    ("hellip", '…'),
    ("permil", '‰'),
    ("prime", '′'),
    ("Prime", '″'),
    ("lsaquo", '‹'),
    ("rsaquo", '›'),
    ("euro", '€'),
    ("trade", '™'),
    ("larr", '←'),
    ("rarr", '→'),
    ("minus", '−'),
    ("infin", '∞'),
    ("ne", '≠'),
    ("le", '≤'),
    ("ge", '≥'),
];

fn decode_entities(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..]
            .char_indices()
            .take(33)
            .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '#'))
            .filter(|(_, c)| *c == ';')
            .map(|(i, _)| i);
        let Some(end) = end else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..1 + end];
        let decoded = if let Some(num) = name.strip_prefix('#') {
            let code = match num.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok(),
                None => num.parse::<u32>().ok(),
            };
            code.and_then(char::from_u32)
        } else {
            match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => HTML_ENTITIES
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, c)| *c),
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 2..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == ':'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '-' | '.')
}

pub fn parse(bytes: &[u8]) -> Option<Document> {
    parse_str(&decode_bytes(bytes))
}

pub fn parse_str(src: &str) -> Option<Document> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut pos = 0;
    let mut stack: Vec<Element> = vec![Element::new("#root")];
    let mut pre = String::new();
    let b = src.as_bytes();
    while pos < b.len() {
        if b[pos] != b'<' {
            let end = src[pos..].find('<').map_or(b.len(), |i| pos + i);
            let text = decode_entities(&src[pos..end]).into_owned();
            stack.last_mut()?.children.push(Node::Text(text));
            pos = end;
            continue;
        }
        let rest = &src[pos..];
        if rest.starts_with("<!--") {
            let end = rest.find("-->").map_or(rest.len(), |i| i + 3);
            push_raw(&mut stack, &mut pre, &rest[..end]);
            pos += end;
        } else if rest.starts_with("<![CDATA[") {
            let body_end = rest.find("]]>");
            let body = &rest[9..body_end.unwrap_or(rest.len())];
            stack
                .last_mut()?
                .children
                .push(Node::Text(body.to_string()));
            pos += body_end.map_or(rest.len(), |i| i + 3);
        } else if rest.starts_with("<?") {
            let end = rest.find("?>").map_or(rest.len(), |i| i + 2);
            if !rest.starts_with("<?xml ") && !rest.starts_with("<?xml?") {
                push_raw(&mut stack, &mut pre, &rest[..end]);
            }
            pos += end;
        } else if rest.starts_with("<!") {
            let mut depth = 0i32;
            let mut end = rest.len();
            for (i, c) in rest.char_indices() {
                match c {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    '>' if depth <= 0 => {
                        end = i + 1;
                        break;
                    }
                    _ => {}
                }
            }
            push_raw(&mut stack, &mut pre, &rest[..end]);
            pos += end;
        } else if rest.starts_with("</") {
            let end = rest.find('>').map_or(rest.len(), |i| i + 1);
            let name = rest[2..end.saturating_sub(1).max(2)].trim();
            if let Some(idx) = stack
                .iter()
                .rposition(|e| e.name == name)
                .filter(|&i| i > 0)
            {
                while stack.len() > idx {
                    let done = stack.pop()?;
                    stack.last_mut()?.children.push(Node::Element(done));
                }
            }
            pos += end;
        } else if rest[1..].chars().next().is_some_and(is_name_start) {
            let (el, len, selfclose) = parse_start_tag(rest);
            pos += len;
            if selfclose {
                stack.last_mut()?.children.push(Node::Element(el));
            } else {
                stack.push(el);
            }
        } else {
            stack.last_mut()?.children.push(Node::Text("<".into()));
            pos += 1;
        }
    }
    while stack.len() > 1 {
        let done = stack.pop()?;
        stack.last_mut()?.children.push(Node::Element(done));
    }
    let top = stack.pop()?;
    let mut root = None;
    for n in top.children {
        if let Node::Element(e) = n
            && root.is_none()
        {
            root = Some(e);
        }
    }
    Some(Document { pre, root: root? })
}

fn push_raw(stack: &mut [Element], pre: &mut String, raw: &str) {
    if stack.len() == 1 {
        pre.push_str(raw);
        pre.push('\n');
    } else if let Some(top) = stack.last_mut() {
        top.children.push(Node::Raw(raw.to_string()));
    }
}

fn parse_start_tag(s: &str) -> (Element, usize, bool) {
    let b = s.as_bytes();
    let name_end = s[1..]
        .find(|c: char| !is_name_char(c))
        .map_or(s.len(), |e| e + 1);
    let mut el = Element::new(&s[1..name_end]);
    let mut i = name_end;
    loop {
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            return (el, b.len(), false);
        }
        match b[i] {
            b'>' => return (el, i + 1, false),
            b'/' => {
                if b.get(i + 1) == Some(&b'>') {
                    return (el, i + 2, true);
                }
                i += 1;
            }
            _ => {
                let start = i;
                while i < b.len()
                    && !matches!(b[i], b'=' | b'>' | b'/')
                    && !(b[i] as char).is_whitespace()
                {
                    i += 1;
                }
                let key = s[start..i].to_string();
                let mut j = i;
                while j < b.len() && (b[j] as char).is_whitespace() {
                    j += 1;
                }
                let mut value = String::new();
                if b.get(j) == Some(&b'=') {
                    j += 1;
                    while j < b.len() && (b[j] as char).is_whitespace() {
                        j += 1;
                    }
                    if let Some(&q) = b.get(j).filter(|c| **c == b'"' || **c == b'\'') {
                        let vstart = j + 1;
                        let vend = s[vstart..].find(q as char).map_or(b.len(), |e| vstart + e);
                        value = decode_entities(&s[vstart..vend]).into_owned();
                        j = (vend + 1).min(b.len());
                    } else {
                        let vstart = j;
                        while j < b.len() && !(b[j] as char).is_whitespace() && b[j] != b'>' {
                            j += 1;
                        }
                        value = decode_entities(&s[vstart..j]).into_owned();
                    }
                    i = j;
                }
                if !key.is_empty() && !el.attrs.iter().any(|(k, _)| *k == key) {
                    el.attrs.push((key, value));
                }
            }
        }
    }
}

fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c if (c as u32) < 32 && !matches!(c, '\n' | '\r' | '\t') => {}
            c => out.push(c),
        }
    }
}

fn escape_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if (c as u32) < 32 => {}
            c => out.push(c),
        }
    }
}

fn write_element(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.name);
    for (k, v) in &e.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for n in &e.children {
        match n {
            Node::Element(c) => write_element(c, out),
            Node::Text(t) => escape_text(t, out),
            Node::Raw(r) => out.push_str(r),
        }
    }
    out.push_str("</");
    out.push_str(&e.name);
    out.push('>');
}

impl Document {
    pub fn serialize(&self) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
        out.push_str(&self.pre);
        write_element(&self.root, &mut out);
        out.push('\n');
        out
    }
}

pub fn escape(s: &str) -> String {
    let mut out = String::new();
    escape_text(s, &mut out);
    out
}
