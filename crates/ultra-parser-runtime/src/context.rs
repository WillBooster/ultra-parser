//! Graph-structured rule invocation stacks used during prediction, ported from ANTLR's
//! `PredictionContext` family.

use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::atn::{Atn, Transition};

/// The return state that marks the bottom of a stack (`$`).
pub(crate) const EMPTY_RETURN_STATE: usize = usize::MAX;

pub(crate) type Ctx = Rc<PredictionContext>;

#[derive(Debug)]
pub(crate) enum PredictionContext {
    Empty,
    Singleton {
        parent: Ctx,
        return_state: usize,
        hash: u64,
    },
    /// Return states are sorted; a `None` parent pairs with `EMPTY_RETURN_STATE`.
    Array {
        parents: Vec<Option<Ctx>>,
        return_states: Vec<usize>,
        hash: u64,
    },
}

fn mix(hash: u64, value: u64) -> u64 {
    (hash.rotate_left(5) ^ value).wrapping_mul(0x517c_c1b7_2722_0a95)
}

fn hash_of(ctx: Option<&Ctx>) -> u64 {
    ctx.map_or(0, |c| c.hash_value())
}

impl PredictionContext {
    pub(crate) fn empty() -> Ctx {
        Rc::new(Self::Empty)
    }

    pub(crate) fn singleton(parent: Option<Ctx>, return_state: usize) -> Ctx {
        match parent {
            None => {
                debug_assert_eq!(return_state, EMPTY_RETURN_STATE);
                Self::empty()
            }
            Some(parent) => {
                let hash = mix(mix(1, parent.hash_value()), return_state as u64);
                Rc::new(Self::Singleton {
                    parent,
                    return_state,
                    hash,
                })
            }
        }
    }

    fn array(parents: Vec<Option<Ctx>>, return_states: Vec<usize>) -> Ctx {
        let mut hash = 2;
        for parent in &parents {
            hash = mix(hash, hash_of(parent.as_ref()));
        }
        for &return_state in &return_states {
            hash = mix(hash, return_state as u64);
        }
        Rc::new(Self::Array {
            parents,
            return_states,
            hash,
        })
    }

    /// Builds the stack of follow states for the rule invocations in `invoking_states`, which are
    /// listed from the innermost invocation outwards.
    pub(crate) fn from_invoking_states(atn: &Atn, invoking_states: &[usize]) -> Ctx {
        let mut ctx = Self::empty();
        for &state in invoking_states.iter().rev() {
            let Transition::Rule { follow_state, .. } = atn.states[state].transitions[0] else {
                unreachable!("an invoking state must start with a rule transition");
            };
            ctx = Self::singleton(Some(ctx), follow_state);
        }
        ctx
    }

    fn hash_value(&self) -> u64 {
        match self {
            Self::Empty => 1,
            Self::Singleton { hash, .. } | Self::Array { hash, .. } => *hash,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Empty | Self::Singleton { .. } => 1,
            Self::Array { return_states, .. } => return_states.len(),
        }
    }

    pub(crate) fn parent(&self, i: usize) -> Option<&Ctx> {
        match self {
            Self::Empty => None,
            Self::Singleton { parent, .. } => Some(parent),
            Self::Array { parents, .. } => parents[i].as_ref(),
        }
    }

    pub(crate) fn return_state(&self, i: usize) -> usize {
        match self {
            Self::Empty => EMPTY_RETURN_STATE,
            Self::Singleton { return_state, .. } => *return_state,
            Self::Array { return_states, .. } => return_states[i],
        }
    }

    pub(crate) fn has_empty_path(&self) -> bool {
        self.return_state(self.len() - 1) == EMPTY_RETURN_STATE
    }

    fn to_array_parts(&self) -> (Vec<Option<Ctx>>, Vec<usize>) {
        match self {
            Self::Empty => (vec![None], vec![EMPTY_RETURN_STATE]),
            Self::Singleton {
                parent,
                return_state,
                ..
            } => (vec![Some(parent.clone())], vec![*return_state]),
            Self::Array {
                parents,
                return_states,
                ..
            } => (parents.clone(), return_states.clone()),
        }
    }
}

impl PartialEq for PredictionContext {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        if self.hash_value() != other.hash_value() {
            return false;
        }
        match (self, other) {
            (Self::Empty, Self::Empty) => true,
            (
                Self::Singleton {
                    parent: p1,
                    return_state: r1,
                    ..
                },
                Self::Singleton {
                    parent: p2,
                    return_state: r2,
                    ..
                },
            ) => r1 == r2 && p1 == p2,
            (
                Self::Array {
                    parents: p1,
                    return_states: r1,
                    ..
                },
                Self::Array {
                    parents: p2,
                    return_states: r2,
                    ..
                },
            ) => r1 == r2 && p1 == p2,
            _ => false,
        }
    }
}

impl Eq for PredictionContext {}

impl Hash for PredictionContext {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash_value());
    }
}

/// Merges two stacks. With `root_is_wildcard` (SLL prediction), the empty stack stands for any
/// stack and absorbs the other one.
pub(crate) fn merge(a: &Ctx, b: &Ctx, root_is_wildcard: bool) -> Ctx {
    if Rc::ptr_eq(a, b) || a == b {
        return a.clone();
    }
    let a_single = !matches!(**a, PredictionContext::Array { .. });
    let b_single = !matches!(**b, PredictionContext::Array { .. });
    if a_single && b_single {
        return merge_singletons(a, b, root_is_wildcard);
    }
    if root_is_wildcard {
        if a.is_empty() {
            return a.clone();
        }
        if b.is_empty() {
            return b.clone();
        }
    }
    merge_arrays(a, b, root_is_wildcard)
}

fn merge_singletons(a: &Ctx, b: &Ctx, root_is_wildcard: bool) -> Ctx {
    if let Some(merged) = merge_root(a, b, root_is_wildcard) {
        return merged;
    }
    let (a_parent, a_return) = (a.parent(0).expect("non-empty singleton"), a.return_state(0));
    let (b_parent, b_return) = (b.parent(0).expect("non-empty singleton"), b.return_state(0));
    if a_return == b_return {
        let parent = merge(a_parent, b_parent, root_is_wildcard);
        if parent == *a_parent {
            return a.clone();
        }
        if parent == *b_parent {
            return b.clone();
        }
        return PredictionContext::singleton(Some(parent), a_return);
    }
    let (first, second) = if a_return < b_return {
        ((a_parent, a_return), (b_parent, b_return))
    } else {
        ((b_parent, b_return), (a_parent, a_return))
    };
    if a_parent == b_parent {
        return PredictionContext::array(
            vec![Some(a_parent.clone()), Some(a_parent.clone())],
            vec![first.1, second.1],
        );
    }
    PredictionContext::array(
        vec![Some(first.0.clone()), Some(second.0.clone())],
        vec![first.1, second.1],
    )
}

fn merge_root(a: &Ctx, b: &Ctx, root_is_wildcard: bool) -> Option<Ctx> {
    if root_is_wildcard {
        if a.is_empty() || b.is_empty() {
            return Some(PredictionContext::empty());
        }
        return None;
    }
    match (a.is_empty(), b.is_empty()) {
        (true, true) => Some(PredictionContext::empty()),
        (true, false) => Some(PredictionContext::array(
            vec![b.parent(0).cloned(), None],
            vec![b.return_state(0), EMPTY_RETURN_STATE],
        )),
        (false, true) => Some(PredictionContext::array(
            vec![a.parent(0).cloned(), None],
            vec![a.return_state(0), EMPTY_RETURN_STATE],
        )),
        (false, false) => None,
    }
}

fn merge_arrays(a: &Ctx, b: &Ctx, root_is_wildcard: bool) -> Ctx {
    let (a_parents, a_returns) = a.to_array_parts();
    let (b_parents, b_returns) = b.to_array_parts();
    let mut parents = Vec::with_capacity(a_returns.len() + b_returns.len());
    let mut returns = Vec::with_capacity(a_returns.len() + b_returns.len());
    let (mut i, mut j) = (0, 0);
    while i < a_returns.len() && j < b_returns.len() {
        let a_parent = &a_parents[i];
        let b_parent = &b_parents[j];
        if a_returns[i] == b_returns[j] {
            let payload = a_returns[i];
            let both_dollars =
                payload == EMPTY_RETURN_STATE && a_parent.is_none() && b_parent.is_none();
            let same_parent = matches!((a_parent, b_parent), (Some(x), Some(y)) if x == y);
            if both_dollars || same_parent {
                parents.push(a_parent.clone());
            } else {
                let (Some(x), Some(y)) = (a_parent, b_parent) else {
                    unreachable!("only the empty return state has no parent");
                };
                parents.push(Some(merge(x, y, root_is_wildcard)));
            }
            returns.push(payload);
            i += 1;
            j += 1;
        } else if a_returns[i] < b_returns[j] {
            parents.push(a_parent.clone());
            returns.push(a_returns[i]);
            i += 1;
        } else {
            parents.push(b_parent.clone());
            returns.push(b_returns[j]);
            j += 1;
        }
    }
    parents.extend(a_parents[i..].iter().cloned());
    returns.extend_from_slice(&a_returns[i..]);
    parents.extend(b_parents[j..].iter().cloned());
    returns.extend_from_slice(&b_returns[j..]);

    if returns.len() == 1 {
        return PredictionContext::singleton(parents.pop().unwrap(), returns[0]);
    }
    let merged = PredictionContext::array(parents, returns);
    if merged == *a {
        return a.clone();
    }
    if merged == *b {
        return b.clone();
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(return_states: &[usize]) -> Ctx {
        let mut ctx = PredictionContext::empty();
        for &r in return_states {
            ctx = PredictionContext::singleton(Some(ctx), r);
        }
        ctx
    }

    #[test]
    fn wildcard_root_absorbs_other_stacks() {
        let empty = PredictionContext::empty();
        assert!(merge(&empty, &stack(&[1]), true).is_empty());
        assert!(merge(&stack(&[1]), &empty, true).is_empty());
    }

    #[test]
    fn full_context_root_keeps_both_paths() {
        let merged = merge(&PredictionContext::empty(), &stack(&[1]), false);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.return_state(0), 1);
        assert!(merged.has_empty_path());
    }

    #[test]
    fn merges_shared_prefixes() {
        let merged = merge(&stack(&[5, 1]), &stack(&[5, 2]), false);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.return_state(0), 1);
        assert_eq!(merged.return_state(1), 2);
        assert_eq!(merged.parent(0), merged.parent(1));

        let merged = merge(&stack(&[1, 7]), &stack(&[2, 7]), false);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged.return_state(0), 7);
        assert_eq!(merged.parent(0).unwrap().len(), 2);
    }
}
