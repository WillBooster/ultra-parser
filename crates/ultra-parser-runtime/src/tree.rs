pub type NodeId = usize;

/// A rule invocation the parser tracks; the host builds the parse tree from the parser's events.
#[derive(Clone, Debug)]
pub(crate) struct RuleNode {
    pub(crate) rule_index: usize,
    pub(crate) parent: Option<NodeId>,
    /// Index of the first token of the rule.
    pub(crate) start: usize,
    /// The ATN state that invoked the rule; `None` for the root.
    pub(crate) invoking_state: Option<usize>,
    /// Whether the rule ended early because of a syntax error in it.
    pub(crate) error: bool,
}

/// The invoking states of the rule invocations from `ctx` outwards, excluding the root.
pub(crate) fn invoking_states(nodes: &[RuleNode], mut ctx: NodeId) -> Vec<usize> {
    let mut states = Vec::new();
    while let (Some(parent), Some(invoking_state)) = (nodes[ctx].parent, nodes[ctx].invoking_state)
    {
        states.push(invoking_state);
        ctx = parent;
    }
    states
}
