//! The model contract for brain-dump drafting.
//!
//! The model is asked to reply with a JSON batch of drafts:
//!
//! {
//!   "drafts": [
//!     {
//!       "id": "1",
//!       "title": "Running late tonight",
//!       "recipient": "Anya",
//!       "body": "Hey Anya, I can't make it tonight. Rain check?",
//!       "tone": "casual",
//!       "missingDetails": ["What time was the plan?"],
//!       "status": "review"
//!     }
//!   ]
//! }
//!
//! Replies wrapped in code fences or prose are tolerated, and a bare
//! top-level array of drafts is accepted as a batch.

use crate::draft::{DraftStatus, Tone};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const SYSTEM_PROMPT: &str = r#"Your are blurp and you turn a raw brain dump into message drafts.

The user message is one unstructured dump. It may hold several unrelated intentions, written in fragments with typos. Reply with JSON only, in exactly this shape:

{
  "drafts": [
    {
      "id": "1",
      "title": "Plans tonight",
      "recipient": "Sam",
      "body": "Hey Sam, I can't make it tonight. Does Thursday work instead?",
      "tone": "casual",
      "missingDetails": [],
      "status": "draft"
    }
  ]
}

Fields:
- id: the draft's position in the list, starting at 1.
- title: 2 to 6 words naming the purpose of the message.
- recipient: the exact name or address the user gave, or null if the dump names no one.
- body: the complete message, ready to send once reviewed. Greet the recipient when one is known. No subject line and no signature.
- tone: one of "casual", "warm", "professional", "direct", "firm", chosen from the relationship and the situation.
- missingDetails: short questions covering anything a responsible message needs but the dump does not supply. Empty list when nothing is missing.
- status: "review" when missingDetails is not empty or the intention is unclear, otherwise "draft". Never any other value.

Rules:
- Each distinct intention becomes its own draft. Never merge two intentions into one body.
- Use only what the dump supplies. Never invent recipients, dates, times, amounts, promises, reasons, events, or personal history.
- When a fact is missing, leave it out of the body and list it in missingDetails instead of guessing.
- Keep each body as short as the situation allows, with no filler phrases.
- Preserve the user's meaning and keep their wording wherever it already works.
- If the user message begins with a "Requested tone:" line, use that tone for every draft.
- Output the JSON object and nothing else. No explanations and no code fences."#;

pub const REGENERATE_PROMPT: &str = r#"You rewrite one existing message draft in a requested tone.

The user message contains the original brain dump, the current draft, and the requested tone. Reply with JSON only, in exactly this shape:

{
  "drafts": [
    {
      "id": "1",
      "title": "Plans tonight",
      "recipient": "Sam",
      "body": "Hey Sam, I can't make it tonight. Does Thursday work instead?",
      "tone": "casual",
      "missingDetails": [],
      "status": "draft"
    }
  ]
}

Rules:
- Return exactly one draft.
- Keep the same recipient and the same intention as the current draft.
- Set tone to the requested tone, always.
- Use only facts from the brain dump and the current draft. Never invent recipients, dates, times, amounts, promises, reasons, events, or personal history.
- When a needed fact is missing, leave it out of the body and list it in missingDetails instead of guessing.
- status: "review" when missingDetails is not empty, otherwise "draft". Never any other value.
- Keep the body as short as the situation allows, with no filler phrases.
- Output the JSON object and nothing else. No explanations and no code fences."#;

/// A brain dump submitted for drafting.
///
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftRequest {
    pub dump: String,
    pub tone: Option<Tone>,
}

impl DraftRequest {
    /// Validates the dump before any model work is spent on it.
    pub fn new(dump: impl Into<String>, tone: Option<Tone>) -> Result<Self, ContractError> {
        let dump = dump.into();
        if dump.trim().is_empty() {
            return Err(ContractError::EmptyDump);
        }

        Ok(Self { dump, tone })
    }

    pub fn user_message(&self) -> String {
        match self.tone {
            Some(tone) => format!("Requested tone: {}\n\n{}", tone.label(), self.dump),
            None => self.dump.clone(),
        }
    }
}

pub fn regenerate_message(dump: &str, body: &str, tone: Tone) -> String {
    format!(
        "Original brain dump:\n{}\n\nCurrent draft:\n{}\n\nRequested tone: {}",
        dump,
        body,
        tone.label()
    )
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub id: Uuid,
    pub title: String,
    pub recipient: Option<String>,
    pub body: String,
    pub tone: Tone,
    pub status: DraftStatus,
    pub missing_details: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DraftBatch {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub dump: String,
    pub drafts: Vec<Draft>,
}

/// Everything that can go wrong between the model's reply and a saved batch.
/// Each variant is recoverable which means the UI can show the message and the user
/// keeps their original dump for a retry.
#[derive(Debug, Error)]
pub enum ContractError {
    #[error("the dump is empty; write a thought before drafting")]
    EmptyDump,
    #[error("the model reply contained no JSON object")]
    NoJson,
    #[error("the model reply was not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("the model returned no drafts")]
    NoDrafts,
    #[error("draft #{index} has no title")]
    MissingTitle { index: usize },
    #[error("draft #{index} has no message body")]
    MissingBody { index: usize },
}

/// The JSON object the model is asked to return, before validation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBatch {
    #[serde(default)]
    drafts: Option<Vec<RawDraft>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDraft {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    recipient: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    tone: Option<String>,
    #[serde(default)]
    missing_details: Option<Vec<String>>,
    #[serde(default)]
    status: Option<String>,
}

/// Parses and validates a model reply into a batch ready to save.
pub fn parse_draft_response(
    request: &DraftRequest,
    reply: &str,
) -> Result<DraftBatch, ContractError> {
    let raw = parse_raw_batch(extract_json(reply)?)?;
    let raw_drafts = raw.drafts.unwrap_or_default();
    if raw_drafts.is_empty() {
        return Err(ContractError::NoDrafts);
    }

    let drafts = raw_drafts
        .iter()
        .enumerate()
        .map(|(offset, draft)| normalize_draft(draft, offset + 1, request.tone))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DraftBatch {
        id: Uuid::new_v4(),
        created_at: Utc::now(),
        dump: request.dump.clone(),
        drafts,
    })
}

fn parse_raw_batch(json: &str) -> Result<RawBatch, ContractError> {
    match serde_json::from_str::<RawBatch>(json) {
        Ok(batch) => Ok(batch),
        Err(batch_error) => {
            // Small local models sometimes return a bare array of drafts.
            let drafts = serde_json::from_str(json).map_err(|_| batch_error)?;
            Ok(RawBatch {
                drafts: Some(drafts),
            })
        }
    }
}

fn extract_json(reply: &str) -> Result<&str, ContractError> {
    let reply = reply.trim();
    let start = match (reply.find('{'), reply.find('[')) {
        (Some(object), Some(array)) => object.min(array),
        (Some(found), None) | (None, Some(found)) => found,
        (None, None) => return Err(ContractError::NoJson),
    };
    let end = match reply.rfind('}').max(reply.rfind(']')) {
        Some(end) if end >= start => end,
        _ => reply.len() - 1,
    };

    Ok(&reply[start..=end])
}

fn normalize_draft(
    raw: &RawDraft,
    index: usize,
    requested_tone: Option<Tone>,
) -> Result<Draft, ContractError> {
    let title = non_empty(raw.title.as_deref()).ok_or(ContractError::MissingTitle { index })?;
    let body = non_empty(raw.body.as_deref()).ok_or(ContractError::MissingBody { index })?;

    let tone = requested_tone.unwrap_or_else(|| {
        raw.tone
            .as_deref()
            .and_then(Tone::from_label)
            .unwrap_or(Tone::Professional)
    });

    let status = match raw.status.as_deref().map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("draft") => DraftStatus::Draft,
        _ => DraftStatus::Review,
    };

    let missing_details = raw
        .missing_details
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|detail| non_empty(Some(detail.as_str())))
        .collect();

    Ok(Draft {
        id: Uuid::new_v4(),
        title,
        recipient: non_empty(raw.recipient.as_deref()),
        body,
        tone,
        status,
        missing_details,
    })
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(dump: &str) -> DraftRequest {
        DraftRequest::new(dump, None).unwrap()
    }

    const MIXED_FIXTURE: &str = r#"```json
{
  "drafts": [
    {
      "id": "a1",
      "title": "Running late tonight",
      "recipient": "Anya",
      "body": "Hey Anya, I can't make it tonight. Rain check?",
      "tone": "Casual",
      "missingDetails": [],
      "status": "DRAFT"
    },
    {
      "title": "Invoice status",
      "recipient": "Dev",
      "body": "Quick check-in: has last week's invoice been paid?",
      "tone": "direct",
      "missingDetails": ["Which invoice number?", "When was it sent?"],
      "status": "review"
    },
    {
      "title": "Monday submission",
      "body": "Would Monday morning work for my submission?",
      "tone": "assertive",
      "missingDetails": null,
      "status": null,
      "extra": "ignored"
    }
  ]
}
```
Hope this helps!"#;

    const CASUAL_DUMP: &str = "tell sam i cant make it tonight, ask if thursday works instead";
    const CASUAL_FIXTURE: &str = r#"{
  "drafts": [
    {
      "id": "1",
      "title": "Cancel tonight",
      "recipient": "Sam",
      "body": "Hey Sam, I can't make it tonight. Does Thursday work instead?",
      "tone": "casual",
      "missingDetails": [],
      "status": "draft"
    }
  ]
}"#;

    const PROFESSIONAL_DUMP: &str = "Need to email Priya about invoice 1042. It was due last Friday and is still unpaid. Ask when it will be settled.";
    const PROFESSIONAL_FIXTURE: &str = r#"{
  "drafts": [
    {
      "id": "1",
      "title": "Invoice 1042 follow-up",
      "recipient": "Priya",
      "body": "Hi Priya, I'm following up on invoice 1042, which was due last Friday and is still unpaid. Could you let me know when it will be settled?",
      "tone": "professional",
      "missingDetails": [],
      "status": "draft"
    }
  ]
}"#;

    const UNDERSPECIFIED_DUMP: &str = "thank my professor for the extension";
    const UNDERSPECIFIED_FIXTURE: &str = r#"{
  "drafts": [
    {
      "id": "1",
      "title": "Thank you for the extension",
      "recipient": null,
      "body": "Thank you for granting the extension.",
      "tone": "warm",
      "missingDetails": [
        "Which professor?",
        "What was the extension for?"
      ],
      "status": "review"
    }
  ]
}"#;

    #[test]
    fn mixed_fixture_parses_into_a_validated_batch() {
        let request = request("Tell Anya I can't make it tonight, ask Dev about the invoice.");
        let batch = parse_draft_response(&request, MIXED_FIXTURE).unwrap();

        assert_eq!(batch.dump, request.dump);
        assert_eq!(batch.drafts.len(), 3);

        let first = &batch.drafts[0];
        assert_eq!(first.title, "Running late tonight");
        assert_eq!(first.recipient.as_deref(), Some("Anya"));
        assert_eq!(first.tone, Tone::Casual);
        assert_eq!(first.status, DraftStatus::Draft);
        assert!(first.missing_details.is_empty());

        let second = &batch.drafts[1];
        assert_eq!(second.tone, Tone::Direct);
        assert_eq!(second.status, DraftStatus::Review);
        assert_eq!(
            second.missing_details,
            vec![
                "Which invoice number?".to_string(),
                "When was it sent?".to_string()
            ]
        );

        let third = &batch.drafts[2];
        assert_eq!(third.recipient, None);
        assert_eq!(third.tone, Tone::Professional);
        assert_eq!(third.status, DraftStatus::Review);
        assert!(third.missing_details.is_empty());

        // The model's ids are ignored; the app assigns stable UUIDs.
        assert_ne!(batch.drafts[0].id, batch.drafts[1].id);
    }

    #[test]
    fn casual_fixture_parses_into_a_sendable_draft() {
        let request = DraftRequest::new(CASUAL_DUMP, None).unwrap();
        let batch = parse_draft_response(&request, CASUAL_FIXTURE).unwrap();

        assert_eq!(batch.dump, CASUAL_DUMP);
        assert_eq!(batch.drafts.len(), 1);

        let draft = &batch.drafts[0];
        assert_eq!(draft.tone, Tone::Casual);
        assert_eq!(draft.status, DraftStatus::Draft);
        assert_eq!(draft.recipient.as_deref(), Some("Sam"));
        assert!(draft.missing_details.is_empty());
        assert!(draft.body.contains("tonight"));
        assert!(draft.body.contains("Thursday"));
    }

    #[test]
    fn professional_fixture_preserves_the_supplied_facts() {
        let request = DraftRequest::new(PROFESSIONAL_DUMP, None).unwrap();
        let batch = parse_draft_response(&request, PROFESSIONAL_FIXTURE).unwrap();

        assert_eq!(batch.drafts.len(), 1);

        let draft = &batch.drafts[0];
        assert_eq!(draft.tone, Tone::Professional);
        assert_eq!(draft.status, DraftStatus::Draft);
        assert_eq!(draft.recipient.as_deref(), Some("Priya"));
        assert!(draft.missing_details.is_empty());
        assert!(draft.body.contains("1042"));
        assert!(draft.body.contains("last Friday"));
        assert!(draft.body.contains("unpaid"));
    }

    #[test]
    fn underspecified_fixture_lands_in_review_with_missing_details() {
        let request = DraftRequest::new(UNDERSPECIFIED_DUMP, None).unwrap();
        let batch = parse_draft_response(&request, UNDERSPECIFIED_FIXTURE).unwrap();

        assert_eq!(batch.drafts.len(), 1);

        let draft = &batch.drafts[0];
        assert_eq!(draft.status, DraftStatus::Review);
        assert_eq!(draft.recipient, None);
        assert_eq!(draft.tone, Tone::Warm);
        assert_eq!(draft.missing_details.len(), 2);
        assert!(draft.body.contains("extension"));
    }

    #[test]
    fn the_system_prompt_matches_the_contract() {
        for field in [
            "id",
            "title",
            "recipient",
            "body",
            "tone",
            "missingDetails",
            "status",
        ] {
            assert!(SYSTEM_PROMPT.contains(field), "missing field {field}");
        }

        for tone in Tone::ALL {
            assert!(
                SYSTEM_PROMPT.contains(tone.label()),
                "missing tone {}",
                tone.label()
            );
        }

        assert!(SYSTEM_PROMPT.contains("\"review\""));
        assert!(SYSTEM_PROMPT.contains("\"draft\""));
    }

    #[test]
    fn the_system_prompt_example_is_a_valid_contract_reply() {
        let request = request("contract check");
        let batch = parse_draft_response(&request, SYSTEM_PROMPT).unwrap();

        assert_eq!(batch.drafts.len(), 1);

        let draft = &batch.drafts[0];
        assert_eq!(draft.title, "Plans tonight");
        assert_eq!(draft.recipient.as_deref(), Some("Sam"));
        assert_eq!(draft.tone, Tone::Casual);
        assert_eq!(draft.status, DraftStatus::Draft);
        assert!(draft.missing_details.is_empty());
    }

    #[test]
    fn user_messages_carry_the_requested_tone_line() {
        let plain = request("hello there");
        assert_eq!(plain.user_message(), "hello there");

        let toned = DraftRequest::new("hello there", Some(Tone::Firm)).unwrap();
        assert_eq!(toned.user_message(), "Requested tone: firm\n\nhello there");
    }

    #[test]
    fn the_regenerate_prompt_matches_the_contract() {
        for field in [
            "id",
            "title",
            "recipient",
            "body",
            "tone",
            "missingDetails",
            "status",
        ] {
            assert!(REGENERATE_PROMPT.contains(field), "missing field {field}");
        }
        assert!(REGENERATE_PROMPT.contains("exactly one draft"));
        assert!(REGENERATE_PROMPT.contains("\"review\""));
        assert!(REGENERATE_PROMPT.contains("\"draft\""));

        let request = request("contract check");
        let batch = parse_draft_response(&request, REGENERATE_PROMPT).unwrap();
        assert_eq!(batch.drafts.len(), 1);
    }

    #[test]
    fn regenerate_messages_carry_the_dump_body_and_tone() {
        let message = regenerate_message("tell sam i cant make it", "Hey Sam.", Tone::Firm);

        assert!(message.contains("tell sam i cant make it"));
        assert!(message.contains("Hey Sam."));
        assert!(message.ends_with("Requested tone: firm"));
    }

    #[test]
    fn bare_arrays_and_prose_wrapped_replies_are_accepted() {
        let request = request("Thank Sam for the referral.");
        let reply = "Here you go: [{\"title\": \"Thanks\", \"body\": \"Thanks again!\"}]";

        let batch = parse_draft_response(&request, reply).unwrap();

        assert_eq!(batch.drafts.len(), 1);
        assert_eq!(batch.drafts[0].title, "Thanks");
    }

    #[test]
    fn non_json_replies_are_rejected() {
        let request = request("hello");

        let error = parse_draft_response(&request, "I could not process that.").unwrap_err();
        assert!(matches!(error, ContractError::NoJson));

        let error = parse_draft_response(&request, "{\"drafts\": [").unwrap_err();
        assert!(matches!(error, ContractError::InvalidJson(_)));
        assert!(error.to_string().contains("not valid JSON"));
    }

    #[test]
    fn empty_draft_lists_are_rejected() {
        let request = request("hello");

        for reply in ["{}", "{\"drafts\": []}", "{\"drafts\": null}"] {
            let error = parse_draft_response(&request, reply).unwrap_err();
            assert!(matches!(error, ContractError::NoDrafts), "{error}");
        }
    }

    #[test]
    fn drafts_with_missing_required_fields_are_rejected() {
        let request = request("hello");

        let reply = "{\"drafts\": [{\"body\": \"no title here\"}]}";
        let error = parse_draft_response(&request, reply).unwrap_err();
        assert!(matches!(error, ContractError::MissingTitle { index: 1 }));

        let reply = "{\"drafts\": [{\"title\": \"one\", \"body\": \"ok\"}, \
                     {\"title\": \"   \", \"body\": \"   \"}]}";
        let error = parse_draft_response(&request, reply).unwrap_err();
        assert!(matches!(error, ContractError::MissingTitle { index: 2 }));

        let reply = "{\"drafts\": [{\"title\": \"one\", \"body\": \"  \"}]}";
        let error = parse_draft_response(&request, reply).unwrap_err();
        assert!(matches!(error, ContractError::MissingBody { index: 1 }));
        assert_eq!(error.to_string(), "draft #1 has no message body");
    }

    #[test]
    fn a_requested_tone_applies_to_every_draft() {
        let request = DraftRequest::new("hello", Some(Tone::Firm)).unwrap();
        let reply = "{\"drafts\": [{\"title\": \"t\", \"body\": \"b\", \"tone\": \"casual\"}]}";

        let batch = parse_draft_response(&request, reply).unwrap();

        assert_eq!(batch.drafts[0].tone, Tone::Firm);
    }

    #[test]
    fn unknown_tones_fall_back_to_professional() {
        let request = request("hello");

        for reply in [
            "{\"drafts\": [{\"title\": \"t\", \"body\": \"b\", \"tone\": \"assertive\"}]}",
            "{\"drafts\": [{\"title\": \"t\", \"body\": \"b\", \"tone\": null}]}",
            "{\"drafts\": [{\"title\": \"t\", \"body\": \"b\"}]}",
        ] {
            let batch = parse_draft_response(&request, reply).unwrap();
            assert_eq!(batch.drafts[0].tone, Tone::Professional, "{reply}");
        }

        let reply =
            "{\"drafts\": [{\"title\": \"t\", \"body\": \"b\", \"tone\": \"  PROFESSIONAL \"}]}";
        let batch = parse_draft_response(&request, reply).unwrap();
        assert_eq!(batch.drafts[0].tone, Tone::Professional);
    }

    #[test]
    fn the_model_cannot_set_terminal_statuses() {
        let request = request("hello");

        for (raw_status, expected) in [
            (Some("completed"), DraftStatus::Review),
            (Some("snoozed"), DraftStatus::Review),
            (Some("review"), DraftStatus::Review),
            (Some("garbage"), DraftStatus::Review),
            (Some("DRAFT"), DraftStatus::Draft),
            (None, DraftStatus::Review),
        ] {
            let reply = match raw_status {
                Some(raw_status) => format!(
                    "{{\"drafts\": [{{\"title\": \"t\", \"body\": \"b\", \
                     \"status\": \"{raw_status}\"}}]}}"
                ),
                None => "{\"drafts\": [{\"title\": \"t\", \"body\": \"b\"}]}".to_string(),
            };

            let batch = parse_draft_response(&request, &reply).unwrap();
            assert_eq!(batch.drafts[0].status, expected, "status {raw_status:?}");
        }
    }

    #[test]
    fn recipients_and_missing_details_are_normalized() {
        let request = request("hello");
        let reply = "{\"drafts\": [{\"title\": \" t \", \"body\": \" b \", \
                     \"recipient\": \"   \", \
                     \"missingDetails\": [\"\", \"   \", \" Which assignment? \"]}]}";

        let batch = parse_draft_response(&request, reply).unwrap();

        let draft = &batch.drafts[0];
        assert_eq!(draft.title, "t");
        assert_eq!(draft.body, "b");
        assert_eq!(draft.recipient, None);
        assert_eq!(draft.missing_details, vec!["Which assignment?".to_string()]);
    }

    #[test]
    fn empty_dumps_are_rejected_before_any_model_work() {
        assert!(matches!(
            DraftRequest::new("   \n\t", None),
            Err(ContractError::EmptyDump)
        ));
    }

    #[test]
    fn batches_round_trip_through_serde() {
        let request = request("Tell Anya I can't make it tonight.");
        let batch = parse_draft_response(&request, MIXED_FIXTURE).unwrap();

        let json = serde_json::to_string(&batch).unwrap();
        let parsed: DraftBatch = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, batch);
    }
}
