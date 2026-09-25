use crate::token::Token;

pub type NodeId = usize;

/// A child of a rule node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Child {
    Rule(NodeId),
    /// A matched token (an index into `ParseTree::tokens`).
    Token(usize),
    /// A token skipped during error recovery (an index into `ParseTree::tokens`).
    SkippedToken(usize),
    /// A token the parser made up during error recovery (an index into
    /// `ParseTree::conjured_tokens`).
    ConjuredToken(usize),
}

#[derive(Clone, Debug)]
pub struct RuleNode {
    pub rule_index: usize,
    pub parent: Option<NodeId>,
    pub children: Vec<Child>,
    /// Index of the first token of the rule.
    pub start: usize,
    /// Index of the last token of the rule; `None` when the rule matched nothing at the start of
    /// the input.
    pub stop: Option<usize>,
    /// The ATN state that invoked the rule; `None` for the root.
    pub(crate) invoking_state: Option<usize>,
}

/// A parse tree stored as an arena of rule nodes.
#[derive(Clone, Debug)]
pub struct ParseTree {
    pub nodes: Vec<RuleNode>,
    pub root: NodeId,
    pub tokens: Vec<Token>,
    /// Tokens made up during error recovery, such as `<missing ')'>`. They take the position of the
    /// token where they were made up.
    pub conjured_tokens: Vec<Token>,
    pub rule_names: &'static [&'static str],
}

impl ParseTree {
    /// Formats the tree as an S-expression like ANTLR's `Trees.toStringTree`, e.g.,
    /// `(expr (expr 1) + (expr 2))`.
    pub fn to_string_tree(&self) -> String {
        let mut buf = String::new();
        self.write_node(self.root, &mut buf);
        buf
    }

    fn write_node(&self, id: NodeId, buf: &mut String) {
        let node = &self.nodes[id];
        let name = self.rule_names[node.rule_index];
        if node.children.is_empty() {
            buf.push_str(name);
            return;
        }
        buf.push('(');
        buf.push_str(name);
        for child in &node.children {
            buf.push(' ');
            match *child {
                Child::Rule(child) => self.write_node(child, buf),
                Child::Token(i) | Child::SkippedToken(i) => push_escaped(buf, &self.tokens[i].text),
                Child::ConjuredToken(i) => push_escaped(buf, &self.conjured_tokens[i].text),
            }
        }
        buf.push(')');
    }
}

fn push_escaped(buf: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '\t' => buf.push_str("\\t"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            c => buf.push(c),
        }
    }
}
