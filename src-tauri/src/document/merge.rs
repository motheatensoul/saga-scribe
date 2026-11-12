//! Writing an edited word into its XML (step 3 of `docs/DOCUMENT_MODEL.md`).
//!
//! The shorthand shows only part of a word's XML. Compiling the edited
//! shorthand from scratch loses the rest: `<ex>` in a diplomatic-only file,
//! `<supplied resp="…">` in a level, a gap's `extent`, comments. Instead, the
//! change between the old and the new shorthand, both compiled, is applied to
//! the word as it is in the file, level by level:
//!
//! - A level's content is read as *atoms* (characters and entity references)
//!   and *ranges* (the elements over them; milestones such as `<lb/>` and
//!   comments are ranges of length zero).
//! - The two compiled versions are compared atom by atom; each change is
//!   applied where the file's level has the same atoms. A level that doesn't
//!   line up there (a pinned level, e.g. a curated normalized form) keeps its
//!   content and is reported.
//! - Ranges that the compiled old version also has (the markup the shorthand
//!   shows) follow the new version, keeping the file's attributes. Ranges it
//!   doesn't have (markup the shorthand can't show) move with the text: text
//!   typed at their edges stays outside, and one whose text was deleted goes,
//!   and is reported.
//! - The word's start tag, its other levels and everything around it stay as
//!   written; a level the word doesn't have is never added.

use super::tree::{NodeId, NodeKind, Tree};

/// A word with an edit merged in.
#[derive(Debug)]
pub struct MergedWord {
    /// The `<w>` element
    pub xml: String,
    /// Levels that couldn't follow the edit and were kept as they were
    pub kept_levels: Vec<String>,
    /// Elements the shorthand doesn't show whose text was deleted
    pub removed: Vec<String>,
}

/// Merges the edit from `old_compiled` to `new_compiled` (the compiler's XML
/// for the word's old and new shorthand) into `word_xml`, the word as written
/// in the file. `None` if the fragments can't be read or lined up at all; the
/// caller then uses `new_compiled`.
pub fn merge_word_edit(word_xml: &str, old_compiled: &str, new_compiled: &str) -> Option<MergedWord> {
    let (mut word, w) = parse_word(word_xml)?;
    let (old, old_w) = parse_word(old_compiled)?;
    let (new, new_w) = parse_word(new_compiled)?;

    // A level the word has but leaves empty stays empty (one with only
    // markup, such as `<gap/>`, isn't empty)
    let word_levels: Vec<(String, NodeId)> = levels(&word, w)
        .into_iter()
        .filter(|&(_, id)| {
            let flat = Flat::read(&word, id, false);
            !flat.atoms.is_empty() || !flat.spans.is_empty()
        })
        .collect();
    let old_levels = levels(&old, old_w);
    let new_levels = levels(&new, new_w);
    let primary = word_levels.first().map(|(name, _)| name.clone())?;

    let mut kept_levels = Vec::new();
    let mut removed = Vec::new();
    for (name, level) in word_levels {
        let counterpart = |levels: &[(String, NodeId)]| -> Option<NodeId> {
            // Same level; or the word's content and the compiled facs (or
            // content), whichever is the main one
            let find = |n: &str| levels.iter().find(|(l, _)| l == n).map(|(_, id)| *id);
            find(&name).or_else(|| {
                (name == primary).then(|| find("facs").or_else(|| find(CONTENT))).flatten()
            })
        };
        let (Some(old_level), Some(new_level)) = (counterpart(&old_levels), counterpart(&new_levels)) else {
            continue;
        };
        let theirs_old = Flat::read(&old, old_level, true);
        let theirs_new = Flat::read(&new, new_level, true);
        if theirs_old == theirs_new {
            continue;
        }
        let ours = Flat::read(&word, level, false);
        match merge_level(&ours, &theirs_old, &theirs_new) {
            Some((merged, lost)) => match nesting(&merged) {
                Some(order) => {
                    rebuild(&mut word, level, &merged, &order);
                    removed.extend(lost);
                }
                None if name == primary => return None,
                None => kept_levels.push(name),
            },
            // The level the shorthand writes must take its edit: the caller
            // then compiles the word instead
            None if name == primary => return None,
            None => kept_levels.push(name),
        }
    }
    Some(MergedWord { xml: word.node_xml(w).ok()?, kept_levels, removed })
}

/// The name of a word's own content when it has no levels.
const CONTENT: &str = "content";

/// Parses a word fragment, with the namespaces a MENOTA word uses declared.
fn parse_word(xml: &str) -> Option<(Tree, NodeId)> {
    let wrapped = format!(
        "<saga-scribe-word xmlns=\"http://www.tei-c.org/ns/1.0\" xmlns:me=\"http://www.menota.org/ns/1.0\">{xml}</saga-scribe-word>"
    );
    let tree = Tree::parse(&wrapped).ok()?;
    let root = tree.root_element()?;
    let w = tree
        .children(root)
        .iter()
        .copied()
        .find(|&id| tree.element(id).is_some_and(|e| local(&e.name) == "w"))?;
    Some((tree, w))
}

fn local(name: &str) -> &str {
    name.split_once(':').map_or(name, |(_, l)| l)
}

/// A word's levels in order (facs, dipl, norm, as present), or its own
/// content when it has none.
fn levels(tree: &Tree, w: NodeId) -> Vec<(String, NodeId)> {
    let mut found = Vec::new();
    for level in ["facs", "dipl", "norm"] {
        let node = tree
            .descendants(w)
            .into_iter()
            .find(|&id| tree.element(id).is_some_and(|e| local(&e.name) == level));
        if let Some(node) = node {
            found.push((level.to_string(), node));
        }
    }
    if found.is_empty() {
        found.push((CONTENT.to_string(), w));
    }
    found
}

// ============================================================================
// A level as atoms and ranges
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
enum Atom {
    Char(char),
    Entity(String),
}

impl Atom {
    /// What the atom reads as, for comparing (`&thorn;` and `þ` are equal).
    fn key(&self) -> String {
        match self {
            Atom::Char(c) => c.to_string(),
            Atom::Entity(name) => crate::entities::bundled()
                .get(name.as_str())
                .map_or_else(|| format!("&{name};"), |e| e.text.clone()),
        }
    }
}

/// An element over atoms `start..end`, or a node of length zero there
/// (a comment, a processing instruction, CDATA).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Span {
    node: NodeKind,
    start: usize,
    end: usize,
    /// Opening order in the document, to keep nesting and order
    order: usize,
}

impl Span {
    fn name(&self) -> Option<&str> {
        match &self.node {
            NodeKind::Element(e) => Some(local(&e.name)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Flat {
    atoms: Vec<Atom>,
    spans: Vec<Span>,
}

impl Flat {
    /// The content of `level`. In `compiled` XML, a line break with the
    /// indentation after it is layout (the compiler puts one after `<lb/>`),
    /// not text.
    fn read(tree: &Tree, level: NodeId, compiled: bool) -> Flat {
        let mut flat = Flat { atoms: Vec::new(), spans: Vec::new() };
        for &child in tree.children(level) {
            flat.read_node(tree, child, compiled);
        }
        flat
    }

    fn read_node(&mut self, tree: &Tree, id: NodeId, compiled: bool) {
        match tree.kind(id) {
            NodeKind::Text(text) if compiled => {
                let mut layout = false;
                for c in text.chars() {
                    layout = c == '\n' || (layout && c == ' ');
                    if !layout {
                        self.atoms.push(Atom::Char(c));
                    }
                }
            }
            NodeKind::Text(text) => self.atoms.extend(text.chars().map(Atom::Char)),
            NodeKind::EntityRef(name) => self.atoms.push(Atom::Entity(name.clone())),
            NodeKind::Element(element) => {
                let index = self.spans.len();
                let start = self.atoms.len();
                self.spans.push(Span {
                    node: NodeKind::Element(element.clone()),
                    start,
                    end: start,
                    order: index,
                });
                for &child in tree.children(id) {
                    self.read_node(tree, child, compiled);
                }
                self.spans[index].end = self.atoms.len();
            }
            other => {
                let at = self.atoms.len();
                let order = self.spans.len();
                self.spans.push(Span { node: other.clone(), start: at, end: at, order });
            }
        }
    }

    fn keys(&self) -> Vec<String> {
        self.atoms.iter().map(Atom::key).collect()
    }
}

// ============================================================================
// Merging
// ============================================================================

/// Matched index pairs of a longest common subsequence of `a` and `b`.
fn common(a: &[String], b: &[String]) -> Option<Vec<(usize, usize)>> {
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > 4_000_000 {
        return None;
    }
    // lengths[i][j]: LCS of a[i..] and b[j..]
    let mut lengths = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lengths[at(i, j)] = if a[i] == b[j] {
                lengths[at(i + 1, j + 1)] + 1
            } else {
                lengths[at(i + 1, j)].max(lengths[at(i, j + 1)])
            };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if lengths[at(i + 1, j)] >= lengths[at(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    Some(pairs)
}

/// A change: atoms `old` of one sequence became atoms `new` of the other.
#[derive(Debug, Clone)]
struct Hunk {
    old: std::ops::Range<usize>,
    new: std::ops::Range<usize>,
}

/// The changes between two sequences, from their matched pairs.
fn hunks(pairs: &[(usize, usize)], old_len: usize, new_len: usize) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let (mut i, mut j) = (0, 0);
    for &(pi, pj) in pairs.iter().chain(std::iter::once(&(old_len, new_len))) {
        if pi > i || pj > j {
            hunks.push(Hunk { old: i..pi, new: j..pj });
        }
        i = pi + 1;
        j = pj + 1;
    }
    hunks
}

/// Where position `pos` (between atoms) goes when `hunks` are applied.
/// `right`: text inserted exactly there ends up before the position.
fn map_position(hunks: &[Hunk], pos: usize, right: bool) -> usize {
    let mut delta: isize = 0;
    for hunk in hunks {
        let (start, end) = (hunk.old.start, hunk.old.end);
        let new_start = (start as isize + delta) as usize;
        let inserted = hunk.new.len();
        if pos < start {
            break;
        }
        if pos == start {
            return if right && start == end { new_start + inserted } else { new_start };
        }
        if pos <= end {
            return new_start + (pos - start).min(inserted);
        }
        delta += inserted as isize - (end - start) as isize;
    }
    (pos as isize + delta) as usize
}

/// Merges the change from `theirs_old` to `theirs_new` into `ours`. Returns
/// the merged level and the names of elements removed with their text, or
/// `None` if the change can't be placed in `ours`.
fn merge_level(ours: &Flat, theirs_old: &Flat, theirs_new: &Flat) -> Option<(Flat, Vec<String>)> {
    let (old_keys, new_keys, our_keys) = (theirs_old.keys(), theirs_new.keys(), ours.keys());
    let edit = common(&old_keys, &new_keys)?;
    let edit_hunks = hunks(&edit, old_keys.len(), new_keys.len());

    // Where each atom of the old version is in ours
    let mut to_ours = vec![None; old_keys.len()];
    for (i, j) in common(&old_keys, &our_keys)? {
        to_ours[i] = Some(j);
    }
    // A position between atoms of the old version, in ours
    // (from the atoms on either side; where ours has other atoms between
    // them, the position is ambiguous)
    // (the start and the end of the old version are those of ours)
    let position_in_ours = |p: usize| -> Option<usize> {
        // (an empty old version: what is typed goes after what ours has)
        if p == old_keys.len() {
            return Some(our_keys.len());
        }
        if p == 0 {
            return Some(0);
        }
        let left = to_ours[p - 1].map(|w| w + 1);
        let right = to_ours[p];
        match (left, right) {
            (Some(a), Some(b)) => (a == b).then_some(a),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    };

    // The edit, placed in ours
    let mut our_hunks = Vec::new();
    for hunk in &edit_hunks {
        let start = position_in_ours(hunk.old.start)?;
        let matched = hunk.old.clone().map(|i| to_ours[i]).collect::<Option<Vec<usize>>>()?;
        // The changed atoms are together in ours, with nothing else at either end
        if matched.iter().enumerate().any(|(k, &w)| w != start + k)
            || position_in_ours(hunk.old.end)? != start + matched.len()
        {
            return None;
        }
        our_hunks.push(Hunk { old: start..start + matched.len(), new: hunk.new.clone() });
    }

    // New atoms: ours, with each hunk's atoms from the new version
    let mut atoms = Vec::with_capacity(ours.atoms.len() + 8);
    // Where each atom of the new version ends up
    let mut new_to_merged = vec![None; new_keys.len()];
    let mut ours_to_merged = vec![0; ours.atoms.len() + 1];
    let mut next = 0;
    for hunk in &our_hunks {
        while next < hunk.old.start {
            ours_to_merged[next] = atoms.len();
            atoms.push(ours.atoms[next].clone());
            next += 1;
        }
        for j in hunk.new.clone() {
            new_to_merged[j] = Some(atoms.len());
            atoms.push(theirs_new.atoms[j].clone());
        }
        while next < hunk.old.end {
            ours_to_merged[next] = atoms.len();
            next += 1;
        }
    }
    while next < ours.atoms.len() {
        ours_to_merged[next] = atoms.len();
        atoms.push(ours.atoms[next].clone());
        next += 1;
    }
    ours_to_merged[ours.atoms.len()] = atoms.len();
    // Unchanged atoms of the new version: via the old version, to ours
    for &(i, j) in &edit {
        if let Some(w) = to_ours[i] {
            new_to_merged[j] = Some(ours_to_merged[w]);
        }
    }
    // A position between atoms of the new version, in the merged level:
    // before the atom after it, or else after the atom before it
    let new_position = |p: usize| -> Option<usize> {
        let before = new_to_merged.get(p).copied().flatten();
        let after = p.checked_sub(1).and_then(|q| new_to_merged[q]).map(|m| m + 1);
        before.or(after).or((p == 0).then_some(0))
    };

    // Which of our spans the shorthand shows: the old version has one of the
    // same element over the same atoms
    let old_in_ours = |span: &Span| -> Option<(usize, usize)> {
        Some((position_in_ours(span.start)?, position_in_ours(span.end)?))
    };
    let mut shown: Vec<Option<usize>> = vec![None; ours.spans.len()];
    let mut old_taken = vec![false; theirs_old.spans.len()];
    for (k, span) in ours.spans.iter().enumerate() {
        let Some(name) = span.name() else { continue };
        for (o, old_span) in theirs_old.spans.iter().enumerate() {
            if !old_taken[o]
                && old_span.name() == Some(name)
                && old_in_ours(old_span) == Some((span.start, span.end))
            {
                shown[k] = Some(o);
                old_taken[o] = true;
                break;
            }
        }
    }
    // Each old span's counterpart in the new version
    let mut new_taken = vec![false; theirs_new.spans.len()];
    let mut old_to_new: Vec<Option<usize>> = vec![None; theirs_old.spans.len()];
    for (o, old_span) in theirs_old.spans.iter().enumerate() {
        // Text typed at an edge may have gone inside or outside
        let starts = [map_position(&edit_hunks, old_span.start, true), map_position(&edit_hunks, old_span.start, false)];
        let ends = [map_position(&edit_hunks, old_span.end, true), map_position(&edit_hunks, old_span.end, false)];
        let found = theirs_new.spans.iter().enumerate().position(|(n, s)| {
            !new_taken[n]
                && s.name() == old_span.name()
                && s.node_kind_matches(&old_span.node)
                && starts.contains(&s.start)
                && ends.contains(&s.end)
                && (s.start == s.end) == (old_span.start == old_span.end)
        });
        if let Some(n) = found {
            new_taken[n] = true;
            old_to_new[o] = Some(n);
        }
    }

    let mut spans = Vec::new();
    let mut removed = Vec::new();
    for (k, span) in ours.spans.iter().enumerate() {
        match shown[k] {
            // Shown: follows the new version, with the file's attributes and
            // the changes the edit made to them
            Some(o) => {
                let Some(n) = old_to_new[o] else { continue };
                let new_span = &theirs_new.spans[n];
                let (start, end) = (new_position(new_span.start)?, new_position(new_span.end)?);
                let node = with_attribute_changes(&span.node, &theirs_old.spans[o].node, &new_span.node);
                spans.push(Span { node, start, end, order: span.order });
            }
            // Not shown: moves with the text
            None => {
                let start = map_position(&our_hunks, span.start, span.start != span.end);
                let end = if span.start == span.end { start } else { map_position(&our_hunks, span.end, false) };
                if span.start != span.end && start >= end {
                    removed.push(span.name().unwrap_or("?").to_string());
                    continue;
                }
                spans.push(Span { node: span.node.clone(), start, end, order: span.order });
            }
        }
    }
    // Markup the edit added
    for (n, new_span) in theirs_new.spans.iter().enumerate() {
        if new_taken[n] {
            continue;
        }
        let (start, end) = (new_position(new_span.start)?, new_position(new_span.end)?);
        spans.push(Span { node: new_span.node.clone(), start, end, order: usize::MAX / 2 + n });
    }
    Some((Flat { atoms, spans }, removed))
}

impl Span {
    fn node_kind_matches(&self, other: &NodeKind) -> bool {
        std::mem::discriminant(&self.node) == std::mem::discriminant(other)
    }
}

/// `ours` with the attribute changes from `old` to `new` applied.
fn with_attribute_changes(ours: &NodeKind, old: &NodeKind, new: &NodeKind) -> NodeKind {
    let (NodeKind::Element(ours), NodeKind::Element(old), NodeKind::Element(new)) = (ours, old, new) else {
        return ours.clone();
    };
    let mut element = ours.clone();
    for attribute in &old.attributes {
        if new.attribute(&attribute.name).is_none() {
            element.attributes.retain(|a| a.name != attribute.name);
        }
    }
    for attribute in &new.attributes {
        if old.attribute(&attribute.name) != Some(attribute.value.as_str()) {
            match element.attributes.iter_mut().find(|a| a.name == attribute.name) {
                Some(existing) => existing.value = attribute.value.clone(),
                None => element.attributes.push(attribute.clone()),
            }
        }
    }
    NodeKind::Element(element)
}

/// An order in which the spans open that nests them (outer before inner
/// where they start together), or `None` if they cross. Spans from the file
/// keep their order; then by length.
fn nesting(flat: &Flat) -> Option<Vec<usize>> {
    let by_order = |a: &Span, b: &Span| a.start.cmp(&b.start).then(a.order.cmp(&b.order));
    let by_length = |a: &Span, b: &Span| a.start.cmp(&b.start).then(b.end.cmp(&a.end)).then(a.order.cmp(&b.order));
    for compare in [&by_order as &dyn Fn(&Span, &Span) -> std::cmp::Ordering, &by_length] {
        let mut order: Vec<usize> = (0..flat.spans.len()).collect();
        order.sort_by(|&a, &b| compare(&flat.spans[a], &flat.spans[b]));
        if nests(flat, &order) {
            return Some(order);
        }
    }
    None
}

fn nests(flat: &Flat, order: &[usize]) -> bool {
    // The ends of the elements open at this point, innermost last
    let mut open: Vec<usize> = Vec::new();
    for &i in order {
        let span = &flat.spans[i];
        while open.last().is_some_and(|&end| end <= span.start) {
            open.pop();
        }
        if open.last().is_some_and(|&end| span.end > end) {
            return false;
        }
        if span.end > span.start {
            open.push(span.end);
        }
    }
    true
}

/// Replaces the content of `level` with `flat`, opening spans in `order`
/// (from [`nesting`]).
fn rebuild(tree: &mut Tree, level: NodeId, flat: &Flat, order: &[usize]) {
    let spans: Vec<&Span> = order.iter().map(|&i| &flat.spans[i]).collect();

    for child in tree.children(level).to_vec() {
        tree.remove(child);
    }
    // Open elements: (node, end)
    let mut stack: Vec<(NodeId, usize)> = vec![(level, usize::MAX)];
    let mut next_span = 0;
    let mut text = String::new();
    let flush = |tree: &mut Tree, parent: NodeId, text: &mut String| {
        if !text.is_empty() {
            let at = tree.children(parent).len();
            tree.insert(parent, at, NodeKind::Text(std::mem::take(text)));
        }
    };
    let top = |stack: &[(NodeId, usize)]| stack.last().map_or(level, |&(node, _)| node);
    for pos in 0..=flat.atoms.len() {
        // Close what ends here
        while stack.len() > 1 && stack.last().is_some_and(|&(_, end)| end == pos) {
            flush(tree, top(&stack), &mut text);
            stack.pop();
        }
        // Open what starts here
        while next_span < spans.len() && spans[next_span].start == pos {
            let span = spans[next_span];
            next_span += 1;
            let parent = top(&stack);
            flush(tree, parent, &mut text);
            let at = tree.children(parent).len();
            let node = tree.insert(parent, at, span.node.clone());
            if matches!(span.node, NodeKind::Element(_)) && span.end > span.start {
                stack.push((node, span.end));
            }
        }
        if let Some(atom) = flat.atoms.get(pos) {
            match atom {
                Atom::Char(c) => text.push(*c),
                Atom::Entity(name) => {
                    let parent = top(&stack);
                    flush(tree, parent, &mut text);
                    let at = tree.children(parent).len();
                    tree.insert(parent, at, NodeKind::EntityRef(name.clone()));
                }
            }
        }
    }
    flush(tree, top(&stack), &mut text);
}
