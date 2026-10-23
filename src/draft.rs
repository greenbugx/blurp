#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftStatus {
    Review,
    Draft,
    Snoozed,
    Completed,
}

impl DraftStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Draft => "draft",
            Self::Snoozed => "snoozed",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    Casual,
    Warm,
    Professional,
    Direct,
    Firm,
}

impl Tone {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Casual => "casual",
            Self::Warm => "warm",
            Self::Professional => "professional",
            Self::Direct => "direct",
            Self::Firm => "firm",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftCard {
    pub id: u32,
    pub title: String,
    pub recipient: Option<String>,
    pub tone: Tone,
    pub status: DraftStatus,
    pub body: String,
}

/// Temporary in-memory drafts used to build the workspace before SQLite is wired up.
pub fn sample_drafts() -> Vec<DraftCard> {
    vec![
        draft(
            1,
            "Running late tonight",
            Some("Anya"),
            Tone::Casual,
            DraftStatus::Review,
            "I'm really sorry, but I can't make it tonight. Can we move it to tomorrow or later this week?",
        ),
        draft(
            2,
            "Invoice follow-up",
            Some("Dev"),
            Tone::Direct,
            DraftStatus::Review,
            "Hey Dev, quick check-in: has the invoice from last week been paid? Let me know if you need me to resend it.",
        ),
        draft(
            3,
            "Monday submission",
            Some("Professor Lin"),
            Tone::Professional,
            DraftStatus::Review,
            "Hi Professor Lin, I'm finishing the assignment now. Would it be alright if I submitted it on Monday morning?",
        ),
        draft(
            4,
            "Conference follow-up",
            Some("Mira"),
            Tone::Warm,
            DraftStatus::Draft,
            "It was great meeting you at the conference. I'd love to keep talking about the accessibility work you're doing.",
        ),
        draft(
            5,
            "Rent split reminder",
            Some("Jules"),
            Tone::Firm,
            DraftStatus::Draft,
            "Hey Jules, just a reminder that the rent split for this month is still pending. Could you send it by Friday?",
        ),
        draft(
            6,
            "Birthday message",
            Some("Noah"),
            Tone::Warm,
            DraftStatus::Snoozed,
            "Happy birthday, Noah. Hope this year brings you good work and better sleep.",
        ),
        draft(
            7,
            "Thank you for the referral",
            Some("Sam"),
            Tone::Professional,
            DraftStatus::Completed,
            "Thanks again for the referral. I had a good conversation with the team and wanted you to know I appreciate it.",
        ),
    ]
}

fn draft(
    id: u32,
    title: &str,
    recipient: Option<&str>,
    tone: Tone,
    status: DraftStatus,
    body: &str,
) -> DraftCard {
    DraftCard {
        id,
        title: title.to_string(),
        recipient: recipient.map(str::to_string),
        tone,
        status,
        body: body.to_string(),
    }
}
