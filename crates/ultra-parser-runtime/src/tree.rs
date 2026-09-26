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
        // Trees are as deep as the input is long (e.g., left-recursive rules nest one node per
        // operator), so the walk keeps its own stack of (node, next child) instead of recursing.
        let mut stack = vec![(self.root, 0)];
        while let Some((id, next)) = stack.last_mut() {
            let node = &self.nodes[*id];
            if *next == 0 {
                let name = self.rule_names[node.rule_index];
                if node.children.is_empty() {
                    buf.push_str(name);
                    stack.pop();
                    continue;
                }
                buf.push('(');
                buf.push_str(name);
            }
            let Some(&child) = node.children.get(*next) else {
                buf.push(')');
                stack.pop();
                continue;
            };
            *next += 1;
            buf.push(' ');
            match child {
                Child::Rule(child) => stack.push((child, 0)),
                Child::Token(i) | Child::SkippedToken(i) => {
                    push_escaped(&mut buf, &self.tokens[i].text)
                }
                Child::ConjuredToken(i) => push_escaped(&mut buf, &self.conjured_tokens[i].text),
            }
        }
        buf
    }

    /// The invoking states of the rule invocations from `ctx` outwards, excluding the root.
    pub(crate) fn invoking_states(nodes: &[RuleNode], mut ctx: NodeId) -> Vec<usize> {
        let mut states = Vec::new();
        while let (Some(parent), Some(invoking_state)) =
            (nodes[ctx].parent, nodes[ctx].invoking_state)
        {
            states.push(invoking_state);
            ctx = parent;
        }
        states
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
