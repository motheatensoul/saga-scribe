mod inflections;
mod onp;

pub use inflections::{InflectedForm, InflectionStore};
pub use onp::{OnpEntry, OnpFullEntry, OnpRegistry};

/// Whether two morphosyntactic analyses (`me:msa`, e.g. "xNC gM nS cN sI")
/// are the same: the same tags, in any order. Menota suggests an order, but
/// it carries no meaning.
pub fn same_analysis(a: &str, b: &str) -> bool {
    fn tags(s: &str) -> Vec<&str> {
        let mut tags: Vec<&str> = s.split_whitespace().collect();
        tags.sort_unstable();
        tags
    }
    tags(a) == tags(b)
}

#[cfg(test)]
mod analysis_tests {
    use super::same_analysis;

    #[test]
    fn test_analyses_are_tag_sets() {
        assert!(same_analysis("xNC gM nS cN sI", "xNC nS gM  sI cN"));
        assert!(!same_analysis("xNC gM nS", "xNC gF nS"));
    }
}
