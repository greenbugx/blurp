use crate::contract::Draft;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DraftStatus {
    Review,
    Draft,
    Snoozed,
    Completed,
}

impl DraftStatus {
    pub const ALL: [Self; 4] = [Self::Review, Self::Draft, Self::Snoozed, Self::Completed];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Draft => "draft",
            Self::Snoozed => "snoozed",
            Self::Completed => "completed",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        let label = label.trim();
        Self::ALL
            .iter()
            .copied()
            .find(|status| status.label().eq_ignore_ascii_case(label))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tone {
    Casual,
    Warm,
    Professional,
    Direct,
    Firm,
}

impl Tone {
    pub const ALL: [Self; 5] = [
        Self::Casual,
        Self::Warm,
        Self::Professional,
        Self::Direct,
        Self::Firm,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Casual => "casual",
            Self::Warm => "warm",
            Self::Professional => "professional",
            Self::Direct => "direct",
            Self::Firm => "firm",
        }
    }

    /// Matches a tone by label, ignoring case and surrounding whitespace.
    pub fn from_label(label: &str) -> Option<Self> {
        let label = label.trim();
        Self::ALL
            .iter()
            .copied()
            .find(|tone| tone.label().eq_ignore_ascii_case(label))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftCard {
    pub id: String,
    pub title: String,
    pub recipient: Option<String>,
    pub tone: Tone,
    pub status: DraftStatus,
    pub body: String,
}

impl From<&Draft> for DraftCard {
    fn from(draft: &Draft) -> Self {
        Self {
            id: draft.id.to_string(),
            title: draft.title.clone(),
            recipient: draft.recipient.clone(),
            tone: draft.tone,
            status: draft.status,
            body: draft.body.clone(),
        }
    }
}

/// In-memory drafts for the explicit `--demo` mode only.
///
pub(crate) fn sample_drafts() -> Vec<DraftCard> {
    vec![
        draft(
            "1",
            "Running late tonight",
            Some("Anya"),
            Tone::Casual,
            DraftStatus::Review,
            "I'm really sorry, but I can't make it tonight. Can we move it to tomorrow or later this week?",
        ),
        draft(
            "2",
            "Invoice follow-up",
            Some("Dev"),
            Tone::Direct,
            DraftStatus::Review,
            "Hey Dev, quick check-in: has the invoice from last week been paid? Let me know if you need me to resend it.",
        ),
        draft(
            "3",
            "Monday submission",
            Some("Professor Lin"),
            Tone::Professional,
            DraftStatus::Review,
            "Hi Professor Lin, I'm finishing the assignment now. Would it be alright if I submitted it on Monday morning?",
        ),
        draft(
            "4",
            "Conference follow-up",
            Some("Mira"),
            Tone::Warm,
            DraftStatus::Draft,
            "It was great meeting you at the conference. I'd love to keep talking about the accessibility work you're doing.",
        ),
        draft(
            "5",
            "Rent split reminder",
            Some("Jules"),
            Tone::Firm,
            DraftStatus::Draft,
            "Hey Jules, just a reminder that the rent split for this month is still pending. Could you send it by Friday?",
        ),
        draft(
            "6",
            "Birthday message",
            Some("Noah"),
            Tone::Warm,
            DraftStatus::Snoozed,
            "Happy birthday, Noah. Hope this year brings you good work and better sleep.",
        ),
        draft(
            "7",
            "Thank you for the referral",
            Some("Sam"),
            Tone::Professional,
            DraftStatus::Completed,
            "Thanks again for the referral. I had a good conversation with the team and wanted you to know I appreciate it.",
        ),
    ]
}

fn draft(
    id: &str,
    title: &str,
    recipient: Option<&str>,
    tone: Tone,
    status: DraftStatus,
    body: &str,
) -> DraftCard {
    DraftCard {
        id: id.to_string(),
        title: title.to_string(),
        recipient: recipient.map(str::to_string),
        tone,
        status,
        body: body.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_labels_round_trip_case_insensitively() {
        for tone in Tone::ALL {
            assert_eq!(Tone::from_label(tone.label()), Some(tone));
            assert_eq!(Tone::from_label(&tone.label().to_uppercase()), Some(tone));
        }

        assert_eq!(Tone::from_label("  direct "), Some(Tone::Direct));
        assert_eq!(Tone::from_label("assertive"), None);
        assert_eq!(Tone::from_label(""), None);
    }

    #[test]
    fn status_labels_round_trip_case_insensitively() {
        for status in DraftStatus::ALL {
            assert_eq!(DraftStatus::from_label(status.label()), Some(status));
            assert_eq!(
                DraftStatus::from_label(&status.label().to_uppercase()),
                Some(status)
            );
        }

        assert_eq!(
            DraftStatus::from_label("  review "),
            Some(DraftStatus::Review)
        );
        assert_eq!(DraftStatus::from_label("archived"), None);
        assert_eq!(DraftStatus::from_label(""), None);
    }
}
