//! ATN configurations and configuration sets used by parser prediction.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::context::{Ctx, merge};
use crate::semantic::SemanticContext;

/// A set of alternative numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct AltSet {
    words: Vec<u64>,
}

impl AltSet {
    pub(crate) fn of(alt: usize) -> Self {
        let mut set = Self::default();
        set.insert(alt);
        set
    }

    pub(crate) fn insert(&mut self, alt: usize) {
        let word = alt / 64;
        if self.words.len() <= word {
            self.words.resize(word + 1, 0);
        }
        self.words[word] |= 1 << (alt % 64);
    }

    pub(crate) fn contains(&self, alt: usize) -> bool {
        self.words
            .get(alt / 64)
            .is_some_and(|w| w & (1 << (alt % 64)) != 0)
    }

    pub(crate) fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// The smallest alternative, or `INVALID_ALT` for an empty set.
    pub(crate) fn min(&self) -> usize {
        for (i, w) in self.words.iter().enumerate() {
            if *w != 0 {
                return i * 64 + w.trailing_zeros() as usize;
            }
        }
        crate::atn::INVALID_ALT
    }

    pub(crate) fn union_with(&mut self, other: &AltSet) {
        if self.words.len() < other.words.len() {
            self.words.resize(other.words.len(), 0);
        }
        for (a, b) in self.words.iter_mut().zip(&other.words) {
            *a |= b;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Config {
    pub(crate) state: usize,
    pub(crate) alt: usize,
    pub(crate) context: Ctx,
    pub(crate) semantic: SemanticContext,
    /// How many times the closure left the decision rule into the outer context.
    pub(crate) outer_context_depth: u32,
    pub(crate) precedence_filter_suppressed: bool,
}

/// Like ANTLR's `ATNConfig.equals()`, equality ignores the outer context depth so that closures
/// terminate when they loop through the outer context.
impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
            && self.alt == other.alt
            && self.context == other.context
            && self.semantic == other.semantic
            && self.precedence_filter_suppressed == other.precedence_filter_suppressed
    }
}

impl Eq for Config {}

impl Hash for Config {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.state.hash(state);
        self.alt.hash(state);
        self.context.hash(state);
        self.semantic.hash(state);
    }
}

impl Config {
    pub(crate) fn new(state: usize, alt: usize, context: Ctx) -> Self {
        Self {
            state,
            alt,
            context,
            semantic: SemanticContext::Empty,
            outer_context_depth: 0,
            precedence_filter_suppressed: false,
        }
    }

    pub(crate) fn with_state(&self, state: usize) -> Self {
        Self {
            state,
            ..self.clone()
        }
    }

    pub(crate) fn with_state_and_context(&self, state: usize, context: Ctx) -> Self {
        Self {
            state,
            context,
            ..self.clone()
        }
    }
}

/// An ordered set of configurations that merges the contexts of configurations that share a state,
/// alternative, and semantic context (`ATNConfigSet`).
#[derive(Clone, Debug)]
pub(crate) struct ConfigSet {
    configs: Vec<Config>,
    lookup: HashMap<(usize, usize, SemanticContext), usize>,
    pub(crate) full_ctx: bool,
    pub(crate) has_semantic_context: bool,
    pub(crate) dips_into_outer_context: bool,
    pub(crate) unique_alt: usize,
    pub(crate) conflicting_alts: Option<AltSet>,
}

impl ConfigSet {
    pub(crate) fn new(full_ctx: bool) -> Self {
        Self {
            configs: Vec::new(),
            lookup: HashMap::new(),
            full_ctx,
            has_semantic_context: false,
            dips_into_outer_context: false,
            unique_alt: crate::atn::INVALID_ALT,
            conflicting_alts: None,
        }
    }

    pub(crate) fn add(&mut self, config: Config) {
        if config.semantic != SemanticContext::Empty {
            self.has_semantic_context = true;
        }
        if config.outer_context_depth > 0 {
            self.dips_into_outer_context = true;
        }
        let key = (config.state, config.alt, config.semantic.clone());
        match self.lookup.get(&key) {
            None => {
                self.lookup.insert(key, self.configs.len());
                self.configs.push(config);
            }
            Some(&i) => {
                let existing = &mut self.configs[i];
                existing.context = merge(&existing.context, &config.context, !self.full_ctx);
                existing.outer_context_depth =
                    existing.outer_context_depth.max(config.outer_context_depth);
                existing.precedence_filter_suppressed |= config.precedence_filter_suppressed;
            }
        }
    }

    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Config> {
        self.configs.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.configs.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

impl<'a> IntoIterator for &'a ConfigSet {
    type Item = &'a Config;
    type IntoIter = std::slice::Iter<'a, Config>;

    fn into_iter(self) -> Self::IntoIter {
        self.configs.iter()
    }
}
