use chrono::{DateTime, Utc};

use super::issue::{IssueStatus, Label};

/// The minimum filter shape supported by every adapter.
///
/// New fields cost every future adapter one implementation slot;
/// keep it small until a real caller demands more.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IssueFilter {
    pub status: Option<IssueStatus>,
    pub labels: Vec<Label>,
    pub updated_after: Option<DateTime<Utc>>,
    /// Restrict to one milestone, by title.
    ///
    /// By title rather than by id because that is what a person types
    /// and what the tracker shows them; an id would make the filter
    /// unusable from a terminal without a lookup first.
    pub milestone: Option<String>,
}

impl IssueFilter {
    #[must_use]
    pub fn open() -> Self {
        Self {
            status: Some(IssueStatus::Open),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<Label>) -> Self {
        self.labels.push(label.into());
        self
    }

    #[must_use]
    pub fn updated_after(mut self, ts: DateTime<Utc>) -> Self {
        self.updated_after = Some(ts);
        self
    }
}
