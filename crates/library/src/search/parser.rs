use crate::error::{Error, Result};

pub const LOCATIONS: &[&str] = &[
    "all",
    "search",
    "authors",
    "author",
    "languages",
    "language",
    "series",
    "formats",
    "format",
    "publisher",
    "rating",
    "tags",
    "tag",
    "identifiers",
    "identifier",
    "isbn",
    "author_sort",
    "comments",
    "comment",
    "cover",
    "id",
    "pages",
    "last_modified",
    "pubdate",
    "series_index",
    "series_sort",
    "title_sort",
    "size",
    "date",
    "title",
    "uuid",
    "narrators",
    "narrator",
    "audiobook",
    "audio",
    "ebook",
    "duration",
    "downloaded",
];

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Not(Box<Node>),
    Token { loc: String, query: String },
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Open,
    Close,
    Word(String),
    Quoted(String),
}

impl Tok {
    fn text(&self) -> &str {
        match self {
            Tok::Open => "(",
            Tok::Close => ")",
            Tok::Word(s) | Tok::Quoted(s) => s,
        }
    }
}

const MARK: char = '\u{E000}';

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> String {
    let bytes: Vec<u8> = (0..s.len() / 2)
        .filter_map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn encode_docstrings(expr: &str) -> String {
    let mut out = String::new();
    let mut rest = expr;
    while let Some(start) = rest.find("\"\"\"") {
        let after = &rest[start + 3..];
        let Some(first) = after.chars().next() else {
            break;
        };
        let search_from = first.len_utf8();
        match after[search_from..].find("\"\"\"") {
            Some(end) => {
                let inner = &after[..search_from + end];
                out.push_str(&rest[..start]);
                out.push(MARK);
                out.push_str(&hex(inner));
                out.push(MARK);
                rest = &after[search_from + end + 3..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn decode_docstrings(s: &str) -> String {
    let mut out = String::new();
    let mut parts = s.split(MARK);
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    let mut in_hex = true;
    for p in parts {
        if in_hex {
            out.push_str(&unhex(p));
        } else {
            out.push_str(p);
        }
        in_hex = !in_hex;
    }
    out
}

fn restore_escapes(s: &str) -> String {
    s.replace('\u{1}', "\\")
        .replace('\u{2}', "\"")
        .replace('\u{3}', "(")
        .replace('\u{4}', ")")
}

fn tokenize(expr: &str) -> Vec<Tok> {
    let expr = encode_docstrings(expr);
    let expr = expr
        .replace("\\\\", "\u{1}")
        .replace("\\\"", "\u{2}")
        .replace("\\(", "\u{3}")
        .replace("\\)", "\u{4}");
    let chars: Vec<char> = expr.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    let fix = |s: &str| restore_escapes(&decode_docstrings(s));
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '(' {
            toks.push(Tok::Open);
            i += 1;
        } else if c == ')' {
            toks.push(Tok::Close);
            i += 1;
        } else if c == '"' {
            let Some(end) = chars[i + 1..].iter().position(|&x| x == '"') else {
                break;
            };
            let inner: String = chars[i + 1..i + 1 + end].iter().collect();
            toks.push(Tok::Quoted(fix(&inner)));
            i += end + 2;
        } else {
            let start = i;
            while i < chars.len()
                && !(chars[i] == '"'
                    || chars[i] == '('
                    || chars[i] == ')'
                    || chars[i].is_whitespace())
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            toks.push(Tok::Word(fix(&word)));
        }
    }
    toks
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn lcase(&self) -> Option<String> {
        self.peek().map(|t| t.text().to_lowercase())
    }

    fn or_expression(&mut self) -> Result<Node> {
        let lhs = self.and_expression()?;
        if self.lcase().as_deref() == Some("or") {
            self.pos += 1;
            let rhs = self.or_expression()?;
            return Ok(Node::Or(Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    fn and_expression(&mut self) -> Result<Node> {
        let lhs = self.not_expression()?;
        if self.lcase().as_deref() == Some("and") {
            self.pos += 1;
            let rhs = self.and_expression()?;
            return Ok(Node::And(Box::new(lhs), Box::new(rhs)));
        }
        let starts_term = matches!(self.peek(), Some(Tok::Word(_) | Tok::Quoted(_) | Tok::Open));
        if starts_term && self.lcase().as_deref() != Some("or") {
            let rhs = self.and_expression()?;
            return Ok(Node::And(Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    fn not_expression(&mut self) -> Result<Node> {
        if self.lcase().as_deref() == Some("not") {
            self.pos += 1;
            return Ok(Node::Not(Box::new(self.not_expression()?)));
        }
        self.location_expression()
    }

    fn location_expression(&mut self) -> Result<Node> {
        if self.peek() == Some(&Tok::Open) {
            self.pos += 1;
            let res = self.or_expression()?;
            if self.peek() != Some(&Tok::Close) {
                return Err(Error::Search("missing )".into()));
            }
            self.pos += 1;
            return Ok(res);
        }
        if !matches!(self.peek(), Some(Tok::Word(_) | Tok::Quoted(_))) {
            return Err(Error::Search("expected a lookup name or a word".into()));
        }
        Ok(self.base_token())
    }

    fn base_token(&mut self) -> Node {
        let tok = self.toks[self.pos].clone();
        self.pos += 1;
        let word = match tok {
            Tok::Quoted(q) => {
                return Node::Token {
                    loc: "all".into(),
                    query: q,
                };
            }
            Tok::Word(w) => w,
            _ => unreachable!(),
        };
        let words: Vec<&str> = word.split(':').collect();
        if words.len() > 1 && LOCATIONS.contains(&words[0].to_lowercase().as_str()) {
            let loc = words[0].to_lowercase();
            let rest = &words[1..];
            if rest.len() == 1
                && let Some(Tok::Quoted(q)) = self.peek()
            {
                let q = q.clone();
                self.pos += 1;
                return Node::Token { loc, query: q };
            }
            return Node::Token {
                loc,
                query: rest.join(":"),
            };
        }
        Node::Token {
            loc: "all".into(),
            query: word,
        }
    }
}

pub fn parse(expr: &str) -> Result<Node> {
    let mut p = Parser {
        toks: tokenize(expr),
        pos: 0,
    };
    let node = p.or_expression()?;
    if p.pos < p.toks.len() {
        return Err(Error::Search("extra characters at end of search".into()));
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(loc: &str, q: &str) -> Node {
        Node::Token {
            loc: loc.into(),
            query: q.into(),
        }
    }

    fn and(a: Node, b: Node) -> Node {
        Node::And(Box::new(a), Box::new(b))
    }

    fn or(a: Node, b: Node) -> Node {
        Node::Or(Box::new(a), Box::new(b))
    }

    #[test]
    fn bare_and_fields() {
        assert_eq!(parse("foo").unwrap(), tok("all", "foo"));
        assert_eq!(parse("author:Asimov").unwrap(), tok("author", "Asimov"));
        assert_eq!(parse("AUTHOR:Asimov").unwrap(), tok("author", "Asimov"));
        assert_eq!(parse("nofield:thing").unwrap(), tok("all", "nofield:thing"));
        assert_eq!(
            parse("identifiers:isbn:123").unwrap(),
            tok("identifiers", "isbn:123")
        );
    }

    #[test]
    fn quoting() {
        assert_eq!(parse("\"two words\"").unwrap(), tok("all", "two words"));
        assert_eq!(
            parse("title:\"two words\"").unwrap(),
            tok("title", "two words")
        );
        assert_eq!(
            parse("tags:\"=Science Fiction\"").unwrap(),
            tok("tags", "=Science Fiction")
        );
        assert_eq!(
            parse(r#"tags:"=a \"b\"""#).unwrap(),
            tok("tags", "=a \"b\"")
        );
        assert_eq!(
            parse("comments:\"\"\"a \"quoted\" (x)\"\"\"").unwrap(),
            tok("comments", "a \"quoted\" (x)")
        );
        assert_eq!(parse(r"title:\(x\)").unwrap(), tok("title", "(x)"));
    }

    #[test]
    fn operators() {
        assert_eq!(parse("a b").unwrap(), and(tok("all", "a"), tok("all", "b")));
        assert_eq!(
            parse("a and b").unwrap(),
            and(tok("all", "a"), tok("all", "b"))
        );
        assert_eq!(
            parse("a or b c").unwrap(),
            or(tok("all", "a"), and(tok("all", "b"), tok("all", "c")))
        );
        assert_eq!(
            parse("not a").unwrap(),
            Node::Not(Box::new(tok("all", "a")))
        );
        assert_eq!(
            parse("(a or b) and not tag:c").unwrap(),
            and(
                or(tok("all", "a"), tok("all", "b")),
                Node::Not(Box::new(tok("tag", "c")))
            )
        );
        assert_eq!(
            parse("a OR b").unwrap(),
            or(tok("all", "a"), tok("all", "b"))
        );
    }

    #[test]
    fn errors() {
        assert!(parse("(a or b").is_err());
        assert!(parse("a or").is_err());
        assert!(parse("").is_err());
        assert!(parse("a )").is_err());
        assert!(parse("not").is_err());
    }
}
