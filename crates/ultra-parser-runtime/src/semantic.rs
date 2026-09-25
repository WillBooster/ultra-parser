//! Semantic predicates collected during prediction, ported from ANTLR's `SemanticContext`.
//!
//! The runtime interprets grammars without their embedded code, so user predicates always
//! succeed (like `ParserInterpreter`); only precedence predicates are evaluated.

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SemanticContext {
    /// The always-true context (`SemanticContext.Empty`).
    Empty,
    Predicate {
        rule_index: usize,
        pred_index: usize,
        ctx_dependent: bool,
    },
    Precedence(i32),
    And(Vec<SemanticContext>),
    Or(Vec<SemanticContext>),
}

impl SemanticContext {
    pub(crate) fn and(a: &Self, b: &Self) -> Self {
        if *a == Self::Empty {
            return b.clone();
        }
        if *b == Self::Empty {
            return a.clone();
        }
        let mut operands = Vec::new();
        for context in [a, b] {
            match context {
                Self::And(inner) => inner.iter().for_each(|c| push_unique(&mut operands, c)),
                other => push_unique(&mut operands, other),
            }
        }
        reduce_precedence_predicates(&mut operands, Iterator::min);
        if operands.len() == 1 {
            operands.pop().unwrap()
        } else {
            Self::And(operands)
        }
    }

    pub(crate) fn or(a: &Self, b: &Self) -> Self {
        if *a == Self::Empty || *b == Self::Empty {
            return Self::Empty;
        }
        let mut operands = Vec::new();
        for context in [a, b] {
            match context {
                Self::Or(inner) => inner.iter().for_each(|c| push_unique(&mut operands, c)),
                other => push_unique(&mut operands, other),
            }
        }
        reduce_precedence_predicates(&mut operands, Iterator::max);
        if operands.len() == 1 {
            operands.pop().unwrap()
        } else {
            Self::Or(operands)
        }
    }

    /// Evaluates the context while the parser's current precedence is `precedence`.
    pub(crate) fn eval(&self, precedence: i32) -> bool {
        match self {
            Self::Empty | Self::Predicate { .. } => true,
            Self::Precedence(p) => *p >= precedence,
            Self::And(operands) => operands.iter().all(|c| c.eval(precedence)),
            Self::Or(operands) => operands.iter().any(|c| c.eval(precedence)),
        }
    }

    /// Evaluates the precedence predicates in the context and simplifies it; `None` means the
    /// context can no longer succeed.
    pub(crate) fn eval_precedence(&self, precedence: i32) -> Option<Self> {
        match self {
            Self::Empty | Self::Predicate { .. } => Some(self.clone()),
            Self::Precedence(p) => (*p >= precedence).then_some(Self::Empty),
            Self::And(operands) => {
                let mut differs = false;
                let mut remaining = Vec::new();
                for context in operands {
                    let evaluated = context.eval_precedence(precedence)?;
                    differs |= evaluated != *context;
                    if evaluated != Self::Empty {
                        remaining.push(evaluated);
                    }
                }
                if !differs {
                    return Some(self.clone());
                }
                Some(
                    remaining
                        .iter()
                        .fold(Self::Empty, |result, c| Self::and(&result, c)),
                )
            }
            Self::Or(operands) => {
                let mut differs = false;
                let mut remaining = Vec::new();
                for context in operands {
                    let evaluated = context.eval_precedence(precedence);
                    differs |= evaluated.as_ref() != Some(context);
                    match evaluated {
                        Some(Self::Empty) => return Some(Self::Empty),
                        Some(evaluated) => remaining.push(evaluated),
                        None => {}
                    }
                }
                if !differs {
                    return Some(self.clone());
                }
                let mut iter = remaining.into_iter();
                let first = iter.next()?;
                Some(iter.fold(first, |result, c| Self::or(&result, &c)))
            }
        }
    }
}

fn push_unique(operands: &mut Vec<SemanticContext>, context: &SemanticContext) {
    if !operands.contains(context) {
        operands.push(context.clone());
    }
}

/// Replaces the precedence predicates in `operands` with the one `pick` selects, appended last.
fn reduce_precedence_predicates(
    operands: &mut Vec<SemanticContext>,
    pick: fn(std::vec::IntoIter<i32>) -> Option<i32>,
) {
    let mut precedences = Vec::new();
    operands.retain(|c| match c {
        SemanticContext::Precedence(p) => {
            precedences.push(*p);
            false
        }
        _ => true,
    });
    if let Some(p) = pick(precedences.into_iter()) {
        operands.push(SemanticContext::Precedence(p));
    }
}

#[cfg(test)]
mod tests {
    use super::SemanticContext::*;
    use super::*;

    #[test]
    fn keeps_the_strongest_precedence_predicate() {
        assert_eq!(
            SemanticContext::and(&Precedence(2), &Precedence(5)),
            Precedence(2)
        );
        assert_eq!(
            SemanticContext::or(&Precedence(2), &Precedence(5)),
            Precedence(5)
        );
        assert_eq!(SemanticContext::or(&Precedence(2), &Empty), Empty);
    }

    #[test]
    fn evaluates_precedence_predicates() {
        assert_eq!(Precedence(3).eval_precedence(2), Some(Empty));
        assert_eq!(Precedence(1).eval_precedence(2), None);
        let pred = Predicate {
            rule_index: 0,
            pred_index: 0,
            ctx_dependent: false,
        };
        let and = SemanticContext::and(&pred, &Precedence(3));
        assert_eq!(and.eval_precedence(2), Some(pred));
    }
}
