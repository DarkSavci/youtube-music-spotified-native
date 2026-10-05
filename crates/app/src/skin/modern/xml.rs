//! A reader for the XML of modern skins, which is XML only loosely.
//!
//! Skins were written by hand and read by a parser that forgave nearly
//! everything: tags in any case, ampersands left bare, attributes without
//! quotes, elements never closed. A strict parser refuses half of them, so
//! this one takes what it can: the tags, their attributes, and how they
//! nest. Text between tags says nothing a skin's layout needs, and is
//! passed over.

/// One element: its name and its attributes' names in lower case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Node {
    /// An attribute's value, by its name in lower case.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// An attribute read as a whole number; a fraction is cut short, as
    /// skins that wrote `12.5` had it cut.
    pub fn number(&self, name: &str) -> Option<i32> {
        let value = self.get(name)?.trim();
        value
            .parse::<i32>()
            .ok()
            .or_else(|| value.parse::<f32>().ok().map(|number| number as i32))
    }

    /// Whether an attribute is there and says no: `0` or `false`.
    pub fn is_off(&self, name: &str) -> bool {
        self.get(name)
            .is_some_and(|value| matches!(value.trim(), "0" | "false" | "no"))
    }
}

/// Puts the usual five entities back as the characters they stand for.
fn unescape(value: &str) -> String {
    if !value.contains('&') {
        return value.to_owned();
    }
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Reads one tag's inside, from after its `<` to before its `>`: the name
/// and the attributes.
fn tag(inside: &str) -> Node {
    let inside = inside.trim().trim_end_matches('/').trim_end();
    let end = inside
        .find(|c: char| c.is_whitespace())
        .unwrap_or(inside.len());
    let mut node = Node {
        name: inside[..end].to_ascii_lowercase(),
        ..Node::default()
    };
    let mut rest = inside[end..].trim_start();
    while !rest.is_empty() {
        let Some(equals) = rest.find('=') else {
            break;
        };
        let key = rest[..equals].trim().to_ascii_lowercase();
        let after = rest[equals + 1..].trim_start();
        let (value, left) = match after.chars().next() {
            Some(quote @ ('"' | '\'')) => match after[1..].find(quote) {
                Some(close) => (&after[1..=close], &after[close + 2..]),
                // Never closed: the rest of the tag is the value.
                None => (&after[1..], ""),
            },
            _ => {
                let close = after
                    .find(|c: char| c.is_whitespace())
                    .unwrap_or(after.len());
                (&after[..close], &after[close..])
            }
        };
        // A key with a space in it is two words run together by a
        // missing quote; the last of them is the attribute.
        if let Some(key) = key.split_whitespace().last() {
            node.attributes.push((key.to_owned(), unescape(value)));
        }
        rest = left.trim_start();
    }
    node
}

/// Reads a file's elements, nested as its tags say. An element that is
/// never closed is closed where its parent is.
pub fn parse(text: &str) -> Vec<Node> {
    // The elements still open, outermost first, under a root of no name.
    let mut open = vec![Node::default()];
    let close = |open: &mut Vec<Node>| {
        if open.len() > 1
            && let Some(done) = open.pop()
            && let Some(parent) = open.last_mut()
        {
            parent.children.push(done);
        }
    };
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if let Some(comment) = rest.strip_prefix("!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some(end) = rest.find('>') else {
            break;
        };
        let inside = &rest[..end];
        rest = &rest[end + 1..];
        if inside.starts_with(['?', '!']) {
            continue;
        }
        if let Some(name) = inside.strip_prefix('/') {
            let name = name.trim().to_ascii_lowercase();
            // Closes the nearest element of that name, and with it any
            // left open inside it. A stray one closes nothing.
            if let Some(depth) = open.iter().rposition(|node| node.name == name)
                && depth > 0
            {
                while open.len() > depth {
                    close(&mut open);
                }
            }
            continue;
        }
        let node = tag(inside);
        if node.name.is_empty() {
            continue;
        }
        if inside.trim_end().ends_with('/') {
            if let Some(parent) = open.last_mut() {
                parent.children.push(node);
            }
        } else {
            open.push(node);
        }
    }
    while open.len() > 1 {
        close(&mut open);
    }
    open.pop().map(|root| root.children).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_nest_and_attributes_are_read_whatever_their_case() {
        let nodes = parse(
            r#"<?xml version="1.0"?>
            <!-- a comment with <tags/> in it -->
            <Container ID="Main" name="Main Window">
              <Layout id="normal" Background="bg">
                <Button action="PLAY" x="35" y='87' image="play"/>
                <layer x=-42 relatx=1 image="title"/>
              </Layout>
            </Container>"#,
        );
        assert_eq!(nodes.len(), 1);
        let container = &nodes[0];
        assert_eq!(container.name, "container");
        assert_eq!(container.get("id"), Some("Main"));
        let layout = &container.children[0];
        assert_eq!(layout.get("background"), Some("bg"));
        let button = &layout.children[0];
        assert_eq!(
            (button.number("x"), button.number("y")),
            (Some(35), Some(87))
        );
        let layer = &layout.children[1];
        assert_eq!(layer.number("x"), Some(-42));
        assert_eq!(layer.get("relatx"), Some("1"));
    }

    #[test]
    fn what_a_strict_parser_refuses_is_still_read() {
        // Bare ampersands, an element never closed, a closing tag for
        // nothing, entities, and a number with a fraction.
        let nodes = parse(
            r#"<elements>
                 <bitmap id="a&b" file="pic/a.png" x="1.5">
                 <color id="text" value="0,255,0"/>
               </elements></stray>
               <text default="Rock &amp; Roll" visible="0"/>"#,
        );
        assert_eq!(nodes.len(), 2);
        let bitmap = &nodes[0].children[0];
        assert_eq!(bitmap.get("id"), Some("a&b"));
        assert_eq!(bitmap.number("x"), Some(1));
        // The colour was written inside the bitmap that was never closed.
        assert_eq!(bitmap.children[0].name, "color");
        assert_eq!(nodes[1].get("default"), Some("Rock & Roll"));
        assert!(nodes[1].is_off("visible"));
        assert!(!nodes[1].is_off("default"));
        assert!(parse("no tags at all").is_empty());
        assert!(parse("<unfinished attr=").is_empty());
    }
}
