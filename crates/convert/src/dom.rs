use crate::entities;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Text(String),
    Elem(Elem),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Elem {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

pub fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

const VOID: &[&str] = &[
    "area", "base", "basefont", "br", "col", "embed", "frame", "hr", "img", "input", "link",
    "meta", "param", "source", "track", "wbr",
];

pub fn is_void(name: &str) -> bool {
    VOID.iter().any(|v| v.eq_ignore_ascii_case(name))
}

impl Elem {
    pub fn new(name: &str) -> Elem {
        Elem {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn lname(&self) -> String {
        local(&self.name).to_ascii_lowercase()
    }

    pub fn is(&self, name: &str) -> bool {
        local(&self.name).eq_ignore_ascii_case(name)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn attr_local(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| local(k).eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn set_attr(&mut self, name: &str, value: String) {
        match self.attrs.iter_mut().find(|(k, _)| k == name) {
            Some(a) => a.1 = value,
            None => self.attrs.push((name.to_string(), value)),
        }
    }

    pub fn remove_attr(&mut self, name: &str) {
        self.attrs.retain(|(k, _)| k != name);
    }

    pub fn elements(&self) -> impl Iterator<Item = &Elem> {
        self.children.iter().filter_map(|n| match n {
            Node::Elem(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    pub fn first(&self, name: &str) -> Option<&Elem> {
        self.elements().find(|e| e.is(name))
    }

    pub fn find<'a>(&'a self, name: &str) -> Option<&'a Elem> {
        for e in self.elements() {
            if e.is(name) {
                return Some(e);
            }
            if let Some(f) = e.find(name) {
                return Some(f);
            }
        }
        None
    }

    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Elem>) {
        for e in self.elements() {
            if e.is(name) {
                out.push(e);
            }
            e.find_all(name, out);
        }
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        self.collect_text(&mut out);
        out
    }

    fn collect_text(&self, out: &mut String) {
        for n in &self.children {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Elem(e) => e.collect_text(out),
            }
        }
    }

    pub fn walk_mut(&mut self, f: &mut dyn FnMut(&mut Elem)) {
        f(self);
        for n in &mut self.children {
            if let Node::Elem(e) = n {
                e.walk_mut(f);
            }
        }
    }

    pub fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a Elem)) {
        f(self);
        for e in self.elements() {
            e.walk(f);
        }
    }

    pub fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(Node::Text(t)) = self.children.last_mut() {
            t.push_str(text);
        } else {
            self.children.push(Node::Text(text.to_string()));
        }
    }
}

pub fn merge_text(e: &mut Elem) {
    let old = std::mem::take(&mut e.children);
    for n in old {
        match n {
            Node::Text(t) => {
                if let Some(Node::Text(last)) = e.children.last_mut() {
                    last.push_str(&t);
                } else if !t.is_empty() {
                    e.children.push(Node::Text(t));
                }
            }
            Node::Elem(mut c) => {
                merge_text(&mut c);
                e.children.push(Node::Elem(c));
            }
        }
    }
}

pub fn decode_bytes(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if bytes.starts_with(&[0xff, 0xfe]) {
        return encoding_rs::UTF_16LE.decode(bytes).0.into_owned();
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        return encoding_rs::UTF_16BE.decode(bytes).0.into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    let label = ["encoding=", "charset="].iter().find_map(|key| {
        let at = head.find(key)? + key.len();
        let rest = head[at..].trim_start_matches(['"', '\'']);
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':'))
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    });
    let enc = label
        .and_then(|l| encoding_rs::Encoding::for_label(l.as_bytes()))
        .unwrap_or(encoding_rs::WINDOWS_1252);
    enc.decode(bytes).0.into_owned()
}

pub fn decode_entities(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let semi = rest[1..].find(';').filter(|&n| n <= 32).map(|n| n + 1);
        let decoded = semi.and_then(|end| {
            let body = &rest[1..end];
            let ch = if let Some(num) = body.strip_prefix('#') {
                let code = match num.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => num.parse().ok(),
                };
                code.and_then(char::from_u32)
            } else {
                entities::lookup(body)
            };
            ch.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn clean_text(s: &str) -> String {
    let s = if s.contains('\r') {
        s.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        s.to_string()
    };
    if s.chars().any(is_bad_char) {
        s.chars().filter(|c| !is_bad_char(*c)).collect()
    } else {
        s
    }
}

fn is_bad_char(c: char) -> bool {
    (c < ' ' && c != '\n' && c != '\t') || c == '\u{fffe}' || c == '\u{ffff}'
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
    html: bool,
    stack: Vec<Elem>,
    roots: Vec<Node>,
    seen_head: bool,
    seen_body: bool,
}

type PResult<T> = Result<T, ()>;

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }

    fn add_node(&mut self, node: Node) {
        match self.stack.last_mut() {
            Some(top) => match node {
                Node::Text(t) => top.push_text(&t),
                n => top.children.push(n),
            },
            None => match node {
                Node::Text(t) => {
                    if let Some(Node::Text(last)) = self.roots.last_mut() {
                        last.push_str(&t);
                    } else {
                        self.roots.push(Node::Text(t));
                    }
                }
                n => self.roots.push(n),
            },
        }
    }

    fn add_text(&mut self, raw: &str, decode: bool) {
        if raw.is_empty() {
            return;
        }
        let t = if decode {
            clean_text(&decode_entities(raw))
        } else {
            raw.to_string()
        };
        self.add_node(Node::Text(t));
    }

    fn pop(&mut self) {
        if let Some(e) = self.stack.pop() {
            self.add_node(Node::Elem(e));
        }
    }

    fn run(&mut self) -> PResult<()> {
        while self.pos < self.s.len() {
            let rest = self.rest();
            let lt = match rest.find('<') {
                Some(i) => i,
                None => {
                    self.add_text(rest, true);
                    self.pos = self.s.len();
                    break;
                }
            };
            if lt > 0 {
                self.add_text(&rest[..lt], true);
                self.pos += lt;
                continue;
            }
            if rest.starts_with("<!--") {
                match rest.find("-->") {
                    Some(end) => self.pos += end + 3,
                    None => {
                        if !self.html {
                            return Err(());
                        }
                        self.pos = self.s.len();
                    }
                }
            } else if let Some(body) = rest.strip_prefix("<![CDATA[") {
                match body.find("]]>") {
                    Some(end) => {
                        self.add_text(&body[..end], false);
                        self.pos += 9 + end + 3;
                    }
                    None => {
                        if !self.html {
                            return Err(());
                        }
                        self.add_text(body, false);
                        self.pos = self.s.len();
                    }
                }
            } else if rest.starts_with("<!") {
                self.skip_declaration()?;
            } else if rest.starts_with("<?") {
                match rest.find("?>") {
                    Some(end) => self.pos += end + 2,
                    None => {
                        if !self.html {
                            return Err(());
                        }
                        self.pos = self.s.len();
                    }
                }
            } else if rest.starts_with("</") {
                self.end_tag()?;
            } else if rest[1..].starts_with(|c: char| c.is_alphabetic() || c == '_' || c == ':') {
                self.start_tag()?;
            } else {
                if !self.html {
                    return Err(());
                }
                self.add_text("<", false);
                self.pos += 1;
            }
        }
        if self.html {
            while !self.stack.is_empty() {
                self.pop();
            }
        } else if !self.stack.is_empty() {
            return Err(());
        }
        Ok(())
    }

    fn skip_declaration(&mut self) -> PResult<()> {
        let bytes = self.rest().as_bytes();
        let mut depth = 0;
        let mut quote = 0u8;
        for (i, &b) in bytes.iter().enumerate() {
            if quote != 0 {
                if b == quote {
                    quote = 0;
                }
                continue;
            }
            match b {
                b'"' | b'\'' => quote = b,
                b'[' => depth += 1,
                b']' => depth -= 1,
                b'>' if depth <= 0 => {
                    self.pos += i + 1;
                    return Ok(());
                }
                _ => {}
            }
        }
        if self.html {
            self.pos = self.s.len();
            Ok(())
        } else {
            Err(())
        }
    }

    fn end_tag(&mut self) -> PResult<()> {
        let rest = self.rest();
        let end = match rest.find('>') {
            Some(e) => e,
            None => {
                if self.html {
                    self.pos = self.s.len();
                    return Ok(());
                }
                return Err(());
            }
        };
        let name = rest[2..end].trim();
        self.pos += end + 1;
        if self.html {
            if is_void(name) {
                return Ok(());
            }
            if let Some(i) = self
                .stack
                .iter()
                .rposition(|e| e.name.eq_ignore_ascii_case(name))
            {
                while self.stack.len() > i {
                    self.pop();
                }
            }
            Ok(())
        } else if self.stack.last().is_some_and(|e| e.name == name) {
            self.pop();
            Ok(())
        } else {
            Err(())
        }
    }

    fn implied_ends(&mut self, name: &str) {
        let lname = local(name).to_ascii_lowercase();
        const HEAD_CONTENT: &[&str] = &[
            "head", "title", "meta", "link", "style", "script", "base", "noscript", "template",
        ];
        if self.stack.last().is_some_and(|e| e.is("head"))
            && !HEAD_CONTENT.contains(&lname.as_str())
        {
            self.pop();
        }
        let closes: &[&str] = match lname.as_str() {
            "li" => &["li"],
            "dt" | "dd" => &["dt", "dd"],
            "tr" => &["tr", "td", "th"],
            "td" | "th" => &["td", "th"],
            "tbody" | "thead" | "tfoot" => &["tbody", "thead", "tfoot", "tr", "td", "th"],
            "option" => &["option"],
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => &["p"],
            "p" | "div" | "ul" | "ol" | "dl" | "table" | "blockquote" | "pre" | "hr"
            | "section" | "figure" | "form" | "header" | "footer" | "nav" | "aside" | "article" => {
                &["p"]
            }
            _ => return,
        };
        let barrier: &[&str] = match lname.as_str() {
            "li" => &["ul", "ol"],
            "dt" | "dd" => &["dl"],
            "tr" | "td" | "th" | "tbody" | "thead" | "tfoot" => &["table"],
            _ => &[],
        };
        if closes == ["p"] || lname == "option" {
            if self
                .stack
                .last()
                .is_some_and(|e| closes.contains(&e.lname().as_str()))
            {
                self.pop();
            }
            return;
        }
        let mut target = None;
        for (i, e) in self.stack.iter().enumerate().rev() {
            let n = e.lname();
            if closes.contains(&n.as_str()) {
                target = Some(i);
            } else if barrier.contains(&n.as_str()) || (lname == "li" && n == "li") {
                break;
            }
        }
        if let Some(i) = target {
            while self.stack.len() > i {
                self.pop();
            }
        }
    }

    fn start_tag(&mut self) -> PResult<()> {
        let rest = self.rest();
        let bytes = rest.as_bytes();
        let mut i = 1;
        while i < bytes.len() && !matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>') {
            i += 1;
        }
        let name = rest[1..i].to_string();
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut self_closing = false;
        let mut closed = false;
        while i < bytes.len() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            match bytes[i] {
                b'>' => {
                    i += 1;
                    closed = true;
                    break;
                }
                b'/' => {
                    if bytes.get(i + 1) == Some(&b'>') {
                        self_closing = true;
                        i += 2;
                        closed = true;
                        break;
                    }
                    i += 1;
                    continue;
                }
                _ => {}
            }
            let name_start = i;
            while i < bytes.len()
                && !matches!(bytes[i], b'=' | b'>' | b' ' | b'\t' | b'\n' | b'\r')
                && !(bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'>'))
            {
                i += 1;
            }
            let aname = rest[name_start..i].to_string();
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            let mut value = String::new();
            if bytes.get(i) == Some(&b'=') {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                match bytes.get(i) {
                    Some(&q @ (b'"' | b'\'')) => {
                        let vstart = i + 1;
                        match rest[vstart..].find(q as char) {
                            Some(n) => {
                                value = rest[vstart..vstart + n].to_string();
                                i = vstart + n + 1;
                            }
                            None => {
                                if !self.html {
                                    return Err(());
                                }
                                i = bytes.len();
                            }
                        }
                    }
                    _ => {
                        if !self.html {
                            return Err(());
                        }
                        let vstart = i;
                        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>'
                        {
                            i += 1;
                        }
                        value = rest[vstart..i].to_string();
                    }
                }
            } else if !self.html {
                return Err(());
            }
            let value = clean_text(&decode_entities(&value.replace(['\n', '\t', '\r'], " ")));
            if !attrs.iter().any(|(k, _)| *k == aname) {
                attrs.push((aname, value));
            }
        }
        if !closed {
            if !self.html {
                return Err(());
            }
            self.pos = self.s.len();
            return Ok(());
        }
        self.pos += i;

        let lname = local(&name).to_ascii_lowercase();
        if self.html {
            match lname.as_str() {
                "html"
                    if !self.stack.is_empty()
                        || self.roots.iter().any(|n| matches!(n, Node::Elem(_))) =>
                {
                    return Ok(());
                }
                "head" if self.seen_head => return Ok(()),
                "body" if self.seen_body => return Ok(()),
                "head" => self.seen_head = true,
                "body" => self.seen_body = true,
                _ => {}
            }
            self.implied_ends(&name);
        }
        let elem = Elem {
            name,
            attrs,
            children: Vec::new(),
        };
        let void = self.html && is_void(&lname);
        if self_closing || void {
            self.add_node(Node::Elem(elem));
            return Ok(());
        }
        if lname == "script" || lname == "style" {
            let rest = self.rest();
            let lower = rest.to_ascii_lowercase();
            let close = format!("</{lname}");
            let (body, advance) = match lower.find(&close) {
                Some(at) => {
                    let after = lower[at..]
                        .find('>')
                        .map(|n| at + n + 1)
                        .unwrap_or(rest.len());
                    (&rest[..at], after)
                }
                None => {
                    if !self.html {
                        return Err(());
                    }
                    (rest, rest.len())
                }
            };
            let mut elem = elem;
            if lname == "style" {
                let t = if let Some(inner) = body.trim().strip_prefix("<![CDATA[") {
                    inner.strip_suffix("]]>").unwrap_or(inner).to_string()
                } else {
                    decode_entities(body)
                };
                elem.push_text(&clean_text(&t));
            } else {
                elem.push_text(&clean_text(body));
            }
            self.pos += advance;
            self.add_node(Node::Elem(elem));
            return Ok(());
        }
        self.stack.push(elem);
        Ok(())
    }
}

fn parse_nodes(s: &str, html: bool) -> PResult<Vec<Node>> {
    let mut p = Parser {
        s,
        pos: 0,
        html,
        stack: Vec::new(),
        roots: Vec::new(),
        seen_head: false,
        seen_body: false,
    };
    p.run()?;
    Ok(p.roots)
}

pub fn parse(bytes: &[u8]) -> Elem {
    let s = decode_bytes(bytes);
    let strict = parse_nodes(&s, false).ok().filter(|nodes| {
        nodes.iter().filter(|n| matches!(n, Node::Elem(_))).count() == 1
            && nodes.iter().all(|n| {
                matches!(n, Node::Elem(_)) || matches!(n, Node::Text(t) if t.trim().is_empty())
            })
    });
    let nodes = strict.unwrap_or_else(|| parse_nodes(&s, true).unwrap_or_default());
    let mut elems: Vec<Node> = nodes
        .into_iter()
        .filter(|n| match n {
            Node::Elem(_) => true,
            Node::Text(t) => !t.trim().is_empty(),
        })
        .collect();
    if elems.len() == 1
        && let Some(Node::Elem(e)) = elems.pop()
    {
        return e;
    }
    let mut html = Elem::new("html");
    html.children = elems;
    html
}

pub fn escape_text(s: &str, out: &mut Vec<u8>) {
    let mut start = 0;
    for (i, b) in s.bytes().enumerate() {
        let rep: &[u8] = match b {
            b'&' => b"&amp;",
            b'<' => b"&lt;",
            b'>' => b"&gt;",
            _ => continue,
        };
        out.extend_from_slice(&s.as_bytes()[start..i]);
        out.extend_from_slice(rep);
        start = i + 1;
    }
    out.extend_from_slice(&s.as_bytes()[start..]);
}

pub fn escape_attr(s: &str, out: &mut Vec<u8>) {
    let mut start = 0;
    for (i, b) in s.bytes().enumerate() {
        let rep: &[u8] = match b {
            b'&' => b"&amp;",
            b'<' => b"&lt;",
            b'>' => b"&gt;",
            b'"' => b"&quot;",
            b'\n' => b"&#10;",
            b'\r' => b"&#13;",
            b'\t' => b"&#9;",
            _ => continue,
        };
        out.extend_from_slice(&s.as_bytes()[start..i]);
        out.extend_from_slice(rep);
        start = i + 1;
    }
    out.extend_from_slice(&s.as_bytes()[start..]);
}

pub fn write_start_tag(e: &Elem, out: &mut Vec<u8>, self_close: bool) {
    out.push(b'<');
    out.extend_from_slice(e.name.as_bytes());
    for (k, v) in &e.attrs {
        out.push(b' ');
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(b"=\"");
        escape_attr(v, out);
        out.push(b'"');
    }
    out.extend_from_slice(if self_close { b"/>" } else { b">" });
}

pub fn write_end_tag(e: &Elem, out: &mut Vec<u8>) {
    out.extend_from_slice(b"</");
    out.extend_from_slice(e.name.as_bytes());
    out.push(b'>');
}

pub fn write_node(n: &Node, out: &mut Vec<u8>, expand: &dyn Fn(&Elem) -> bool) {
    match n {
        Node::Text(t) => escape_text(t, out),
        Node::Elem(e) => write_elem(e, out, expand),
    }
}

pub fn write_elem(e: &Elem, out: &mut Vec<u8>, expand: &dyn Fn(&Elem) -> bool) {
    if e.children.is_empty() && !expand(e) {
        write_start_tag(e, out, true);
        return;
    }
    write_start_tag(e, out, false);
    for c in &e.children {
        write_node(c, out, expand);
    }
    write_end_tag(e, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ser(e: &Elem) -> String {
        let mut out = Vec::new();
        write_elem(e, &mut out, &|_| false);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn parses_well_formed_xhtml() {
        let e = parse(
            b"<?xml version=\"1.0\"?><!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"x.dtd\">\
              <html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p class='a'>Hi&nbsp;&amp;\
              &#x41;<br/><a id=\"x\"/></p></body></html>",
        );
        assert_eq!(
            ser(&e),
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p class=\"a\">Hi\u{a0}&amp;A<br/><a id=\"x\"/></p></body></html>"
        );
    }

    #[test]
    fn falls_back_to_html_mode() {
        let e =
            parse(b"<html><body><p>one<p>two<br><img src=a.png><ul><li>a<li>b</ul></body></html>");
        assert_eq!(
            ser(&e),
            "<html><body><p>one</p><p>two<br/><img src=\"a.png\"/></p><ul><li>a</li><li>b</li></ul></body></html>"
        );
    }

    #[test]
    fn unknown_entities_stay_literal() {
        assert_eq!(
            decode_entities("a &foo; b & c &#0x; &lt;"),
            "a &foo; b & c &#0x; <"
        );
    }

    #[test]
    fn style_and_cdata() {
        let e = parse(b"<html><head><style>a > b { x: y }</style></head><body><![CDATA[1 < 2]]></body></html>");
        assert_eq!(
            ser(&e),
            "<html><head><style>a &gt; b { x: y }</style></head><body>1 &lt; 2</body></html>"
        );
    }

    #[test]
    fn latin1_documents() {
        let e = parse(b"<?xml version=\"1.0\" encoding=\"iso-8859-1\"?><p>caf\xe9</p>");
        assert_eq!(e.text(), "caf\u{e9}");
    }
}
