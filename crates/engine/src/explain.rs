//! How a conversion gathered and ordered its candidates, for whoever builds
//! dictionaries to tell why a candidate came, or did not.

use std::collections::HashMap;

use crate::{CandidateFacts, OkuriHead};

/// How one conversion gathered and ordered its candidates, made on the same
/// path as the conversion, so its candidates are always those converting
/// gives.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Explanation {
    /// As converted: in NFC.
    pub reading: String,
    pub okurigana: Option<String>,
    /// In the order converting gives them.
    pub candidates: Vec<Explained>,
    /// What was found and left out, in the order it was left out.
    pub dropped: Vec<Dropped>,
    /// Each key looked up, dictionary by dictionary, in the order looked up.
    pub lookups: Vec<Lookup>,
    /// The end of the text committed to the field, as ranked against.
    pub context: String,
    /// How many commits of the field were ranked against.
    pub history_len: usize,
    pub ranked_by_model: bool,
    /// Whether candidates split at the okurigana mark were brought to the
    /// front: not once the field has a commit of the reading.
    pub okuri_first_applied: bool,
    /// Whether favorites were brought to the front: not once the field has
    /// a commit of the reading, or of the item a candidate filled.
    pub favor_applied: bool,
}

/// One candidate and how it came to its place.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Explained {
    /// As ranked: of the origin taken.
    pub facts: CandidateFacts,
    /// As the candidate carries it.
    pub source: Option<String>,
    /// Every way its dictionary gave its surface, the one taken first, the
    /// rest in the order found.
    pub origins: Vec<Origin>,
    /// Split where the user marked the okurigana.
    pub okuri: bool,
    pub score: Score,
    /// Its place, from 0, once ranked.
    pub ranked: usize,
    /// Its place once candidates split at the mark came to the front. Its
    /// place in the end is its place among the candidates.
    pub after_okuri_first: usize,
    /// The weight of its picks, when that brings it to the front.
    pub favorite: Option<f64>,
}

/// An item of a dictionary that gave a surface, or would have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The place of its dictionary in the list, from 0.
    pub dictionary: usize,
    /// As ranked: a built form's includes what building adds.
    pub cost: u32,
    pub item: OriginItem,
}

/// The item, as the dictionary gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OriginItem {
    /// A word of the whole reading that does not conjugate.
    Word { reading: String, surface: String },
    /// An okurigana word: the reading of its stem, the head it is filed
    /// under, and its surface before the okurigana.
    Okuri {
        stem: String,
        head: OkuriHead,
        surface: String,
    },
    /// A conjugating word, and the reading that followed its stem.
    Conjugated {
        stem: String,
        surface: String,
        conjugation: String,
        rest: String,
    },
    /// An item with placeholders, held as the engine holds them:
    /// [`crate::show_placeholders`] writes them for people to read.
    Placeholders { reading: String, surface: String },
}

/// What was found and left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dropped {
    /// What it would have been; `None` for an item whose placeholders were
    /// never filled.
    pub surface: Option<String>,
    pub origin: Origin,
    pub reason: DropReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropReason {
    /// The user custom dictionary hides the pair it is recorded as.
    Hidden,
    /// Its placeholders were not filled: a function gave nothing for them,
    /// or okurigana was typed.
    NotFilled,
    /// Its conjugation type does not let what followed the stem follow it.
    NotConjugated,
    /// It does not end with the okurigana typed.
    OkuriganaMismatch,
    /// An earlier dictionary in the list gave the same surface.
    Shadowed {
        /// The place of that dictionary in the list.
        by: usize,
    },
}

/// A key looked up in one dictionary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lookup {
    /// The place of the dictionary in the list, from 0.
    pub dictionary: usize,
    pub key: LookupKey,
    /// How many items the dictionary gave for it, of every kind.
    pub found: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LookupKey {
    /// Words and conjugating stems of a reading.
    Reading(String),
    /// Okurigana words of a stem filed under a head.
    Okuri { stem: String, head: OkuriHead },
}

/// What placed a candidate when it was ranked.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Score {
    /// The model's score, and each of the candidate's features with its
    /// weight. The names of the features are the model's, not a promise:
    /// they change with the model's format.
    Model {
        total: f32,
        features: Vec<(String, f32)>,
    },
    /// Without a model, the group a candidate is ranked in; within a group,
    /// by its dictionary's place, then its cost.
    Rules(RuleGroup),
}

/// The groups a candidate is ranked in without a model, first to last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum RuleGroup {
    /// The surface last committed for the reading in this field.
    LastCommit,
    /// A surface committed in this field.
    Committed,
    Other,
}

/// What gathering and ordering report as they go, while explaining. Kept
/// apart from the candidates, so converting moves no more than it ranks.
#[derive(Default)]
pub(crate) struct Trace {
    /// The place of the dictionary being gathered from.
    pub(crate) dictionary: usize,
    pub(crate) lookups: Vec<Lookup>,
    pub(crate) dropped: Vec<Dropped>,
    pub(crate) okuri_first_applied: bool,
    pub(crate) favor_applied: bool,
    /// The origins of each surface the dictionary being gathered from gave,
    /// until it is kept or left out.
    gathering: HashMap<String, Vec<Origin>>,
    /// Each candidate kept, by its surface: no two share one.
    pub(crate) notes: HashMap<String, Notes>,
}

/// What explaining tells of a candidate besides its facts.
#[derive(Default)]
pub(crate) struct Notes {
    pub(crate) origins: Vec<Origin>,
    pub(crate) score: Option<Score>,
    pub(crate) ranked: usize,
    pub(crate) after_okuri_first: usize,
    pub(crate) favorite: Option<f64>,
}

impl Trace {
    fn origin(&self, cost: u32, item: OriginItem) -> Origin {
        Origin {
            dictionary: self.dictionary,
            cost,
            item,
        }
    }

    /// Records an item the dictionary gave `surface` by: first when it is
    /// the one taken.
    pub(crate) fn gathered(&mut self, surface: &str, cost: u32, item: OriginItem, taken: bool) {
        let origin = self.origin(cost, item);
        let origins = self.gathering.entry(surface.to_owned()).or_default();
        let at = if taken { 0 } else { origins.len() };
        origins.insert(at, origin);
    }

    /// Keeps what the dictionary gave `surface` by as a candidate's.
    pub(crate) fn kept(&mut self, surface: &str) {
        let origins = self.gathering.remove(surface).unwrap_or_default();
        self.notes.entry(surface.to_owned()).or_default().origins = origins;
    }

    /// Leaves out every item the dictionary gave `surface` by.
    pub(crate) fn left_out_gathered(&mut self, surface: &str, reason: DropReason) {
        let origins = self.gathering.remove(surface).unwrap_or_default();
        self.dropped
            .extend(origins.into_iter().map(|origin| Dropped {
                surface: Some(surface.to_owned()),
                origin,
                reason: reason.clone(),
            }));
    }

    pub(crate) fn note(&mut self, surface: &str) -> &mut Notes {
        self.notes.entry(surface.to_owned()).or_default()
    }

    pub(crate) fn looked_up(&mut self, key: LookupKey, found: usize) {
        self.lookups.push(Lookup {
            dictionary: self.dictionary,
            key,
            found,
        });
    }

    pub(crate) fn left_out(
        &mut self,
        surface: Option<String>,
        cost: u32,
        item: OriginItem,
        reason: DropReason,
    ) {
        let origin = self.origin(cost, item);
        self.dropped.push(Dropped {
            surface,
            origin,
            reason,
        });
    }
}
