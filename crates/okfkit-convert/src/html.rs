//! HTML pages (wiki and CMS exports): keep the main content, drop page chrome, and turn every
//! table into a markdown table (htmd leaves header-less tables as one cell per line).

use std::rc::Rc;

use htmd::{Element, HtmlToMarkdown};
use markup5ever_rcdom::{Handle, Node, NodeData};

use crate::Error;

/// Elements that are page chrome, not content.
const CHROME: &[&str] = &[
    "script", "style", "noscript", "nav", "header", "footer", "aside", "form", "button", "svg",
    "iframe", "head",
];

fn tag(node: &Node) -> Option<&str> {
    match &node.data {
        NodeData::Element { name, .. } => Some(&name.local),
        _ => None,
    }
}

fn attr(node: &Node, key: &str) -> Option<String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs
            .borrow()
            .iter()
            .find(|a| &*a.name.local == key)
            .map(|a| a.value.to_string()),
        _ => None,
    }
}

fn text_of(node: &Handle, out: &mut String) {
    match &node.data {
        NodeData::Text { contents } => out.push_str(&contents.borrow()),
        _ => {
            if tag(node).is_some_and(|t| CHROME.contains(&t)) {
                return;
            }
            for c in node.children.borrow().iter() {
                text_of(c, out);
            }
        }
    }
}

fn find(node: &Handle, pred: &dyn Fn(&Node) -> bool) -> Option<Handle> {
    if pred(node) {
        return Some(node.clone());
    }
    node.children.borrow().iter().find_map(|c| find(c, pred))
}

/// The main content: `<main>`, `<article>`, `role=main`, a well-known content id, else `<body>`.
fn content_root(doc: &Handle) -> Handle {
    let by_tag = |t: &'static str| move |n: &Node| tag(n) == Some(t);
    find(doc, &by_tag("main"))
        .or_else(|| find(doc, &|n: &Node| attr(n, "role").as_deref() == Some("main")))
        .or_else(|| {
            find(doc, &|n: &Node| {
                attr(n, "id").is_some_and(|id| {
                    matches!(
                        id.as_str(),
                        "main-content" | "content" | "main" | "mw-content-text"
                    )
                })
            })
        })
        .or_else(|| find(doc, &by_tag("article")))
        .or_else(|| find(doc, &by_tag("body")))
        .unwrap_or_else(|| doc.clone())
}

fn cell_text(cell: &Handle) -> String {
    let mut s = String::new();
    text_of(cell, &mut s);
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

fn rows(node: &Handle, out: &mut Vec<Vec<String>>) {
    for c in node.children.borrow().iter() {
        match tag(c) {
            Some("tr") => {
                let cells: Vec<String> = c
                    .children
                    .borrow()
                    .iter()
                    .filter(|x| matches!(tag(x), Some("td" | "th")))
                    .map(cell_text)
                    .collect();
                if !cells.is_empty() {
                    out.push(cells);
                }
            }
            Some("table") => {} // nested tables are flattened into their cell's text
            _ => rows(c, out),
        }
    }
}

fn table(node: &Rc<Node>) -> String {
    let mut r = Vec::new();
    rows(node, &mut r);
    let width = r.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 {
        return String::new();
    }
    let line = |cells: &[String]| {
        let mut l = String::from("|");
        for i in 0..width {
            l.push(' ');
            l.push_str(cells.get(i).map_or("", String::as_str));
            l.push_str(" |");
        }
        l.push('\n');
        l
    };
    let mut out = String::from("\n\n");
    out.push_str(&line(&r[0]));
    out.push('|');
    for _ in 0..width {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in &r[1..] {
        out.push_str(&line(row));
    }
    out.push('\n');
    out
}

/// Converts an HTML page; returns (markdown, title).
pub fn to_markdown(html: &str) -> Result<(String, Option<String>), Error> {
    let converter = HtmlToMarkdown::builder()
        .skip_tags(CHROME.to_vec())
        .add_handler(
            vec!["table"],
            |_: &dyn htmd::element_handler::Handlers, e: Element| Some(table(e.node).into()),
        )
        .build();
    let doc = converter
        .html_to_tree(html)
        .map_err(|e| Error::Failed(e.to_string()))?;
    let title = find(&doc, &|n: &Node| tag(n) == Some("title")).map(|t| {
        let mut s = String::new();
        for c in t.children.borrow().iter() {
            text_of(c, &mut s);
        }
        s.trim().to_owned()
    });
    let root = content_root(&doc);
    let md = converter.tree_to_markdown(&root);
    Ok((md.trim().to_owned() + "\n", title.filter(|t| !t.is_empty())))
}
