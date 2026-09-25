use std::fmt::Write;

use crate::token::Vocabulary;

/// A set of integers stored as sorted, disjoint, inclusive ranges.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IntervalSet {
    intervals: Vec<(i32, i32)>,
}

impl IntervalSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn of(a: i32, b: i32) -> Self {
        let mut set = Self::new();
        set.add_range(a, b);
        set
    }

    pub fn add(&mut self, value: i32) {
        self.add_range(value, value);
    }

    pub fn add_range(&mut self, a: i32, b: i32) {
        if b < a {
            return;
        }
        // Find the first interval that ends at or after `a - 1`, i.e., the first one that can merge.
        let start = self
            .intervals
            .partition_point(|&(_, hi)| hi < a.saturating_sub(1));
        let mut lo = a;
        let mut hi = b;
        let mut end = start;
        while end < self.intervals.len() && self.intervals[end].0 <= hi.saturating_add(1) {
            lo = lo.min(self.intervals[end].0);
            hi = hi.max(self.intervals[end].1);
            end += 1;
        }
        self.intervals.splice(start..end, [(lo, hi)]);
    }

    pub fn add_set(&mut self, other: &IntervalSet) {
        for &(a, b) in &other.intervals {
            self.add_range(a, b);
        }
    }

    pub fn remove(&mut self, value: i32) {
        if let Some(i) = self
            .intervals
            .iter()
            .position(|&(a, b)| a <= value && value <= b)
        {
            let (a, b) = self.intervals[i];
            let mut replacement = Vec::with_capacity(2);
            if a < value {
                replacement.push((a, value - 1));
            }
            if value < b {
                replacement.push((value + 1, b));
            }
            self.intervals.splice(i..=i, replacement);
        }
    }

    pub fn contains(&self, value: i32) -> bool {
        let i = self.intervals.partition_point(|&(_, hi)| hi < value);
        i < self.intervals.len() && self.intervals[i].0 <= value
    }

    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    pub fn len(&self) -> usize {
        self.intervals
            .iter()
            .map(|&(a, b)| (b - a + 1) as usize)
            .sum()
    }

    pub fn min_element(&self) -> Option<i32> {
        self.intervals.first().map(|&(a, _)| a)
    }

    /// Returns the values in `min..=max` that are not in this set.
    pub fn complement(&self, min: i32, max: i32) -> IntervalSet {
        let mut result = IntervalSet::new();
        let mut next = min;
        for &(a, b) in &self.intervals {
            if b < min {
                continue;
            }
            if a > max {
                break;
            }
            if next < a {
                result.add_range(next, a - 1);
            }
            next = next.max(b.saturating_add(1));
        }
        if next <= max {
            result.add_range(next, max);
        }
        result
    }

    pub fn iter(&self) -> impl Iterator<Item = i32> + '_ {
        self.intervals.iter().flat_map(|&(a, b)| a..=b)
    }

    /// Formats the set like ANTLR's `IntervalSet.toString(Vocabulary)`, e.g., `{'+', NUMBER}`.
    pub fn to_string_with(&self, vocabulary: &Vocabulary) -> String {
        if self.is_empty() {
            return "{}".to_string();
        }
        let names: Vec<String> = self
            .iter()
            .map(|t| match t {
                crate::token::EOF => "<EOF>".to_string(),
                crate::token::EPSILON => "<EPSILON>".to_string(),
                _ => vocabulary.display_name(t),
            })
            .collect();
        if names.len() == 1 {
            return names.into_iter().next().unwrap();
        }
        let mut buf = String::from("{");
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            let _ = write!(buf, "{name}");
        }
        buf.push('}');
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_adjacent_and_overlapping_ranges() {
        let mut set = IntervalSet::new();
        set.add_range(5, 7);
        set.add_range(1, 2);
        set.add(3);
        set.add_range(6, 10);
        assert_eq!(set.intervals, vec![(1, 3), (5, 10)]);
        set.add(4);
        assert_eq!(set.intervals, vec![(1, 10)]);
    }

    #[test]
    fn removes_and_complements() {
        let mut set = IntervalSet::of(1, 5);
        set.remove(3);
        assert_eq!(set.intervals, vec![(1, 2), (4, 5)]);
        assert!(!set.contains(3));
        assert_eq!(set.complement(0, 6).intervals, vec![(0, 0), (3, 3), (6, 6)]);
    }
}
