use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingRecord {
    pub id: String,
    pub token: String,
    pub code: String,
    pub requested_at: String,
    pub expires_at: String,
    pub approved: bool,
    pub claimed_at: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct ViewerSession {
    pub id: String,
    pub secret: String,
    pub label: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub expires_at: String,
}

pub fn request_pairing(label: Option<&str>) -> PairingRecord {
    let now = Utc::now();
    let token = Uuid::new_v4().to_string();
    let code: String = token
        .chars()
        .filter(|ch| *ch != '-')
        .take(8)
        .flat_map(char::to_uppercase)
        .collect();

    PairingRecord {
        id: Uuid::new_v4().to_string(),
        token,
        code,
        requested_at: now.to_rfc3339(),
        expires_at: (now + Duration::minutes(10)).to_rfc3339(),
        approved: false,
        claimed_at: None,
        label: label.unwrap_or("New companion device").to_string(),
    }
}

pub fn approve_pairing(record: &PairingRecord) -> Option<PairingRecord> {
    let expires_at = DateTime::parse_from_rfc3339(&record.expires_at)
        .ok()?
        .with_timezone(&Utc);
    if expires_at < Utc::now() {
        return None;
    }
    Some(PairingRecord {
        approved: true,
        ..record.clone()
    })
}

pub fn pairing_matches(record: &PairingRecord, input: &str) -> bool {
    let needle = input.trim();
    !needle.is_empty()
        && (record.code.eq_ignore_ascii_case(needle) || record.token.eq_ignore_ascii_case(needle))
}

pub fn claim_pairing(
    record: &PairingRecord,
    device_label: Option<&str>,
) -> Option<(PairingRecord, ViewerSession)> {
    let expires_at = DateTime::parse_from_rfc3339(&record.expires_at)
        .ok()?
        .with_timezone(&Utc);
    if expires_at < Utc::now() || record.claimed_at.is_some() {
        return None;
    }

    let now = Utc::now();
    let paired_at = now.to_rfc3339();
    let updated = PairingRecord {
        approved: true,
        claimed_at: Some(paired_at.clone()),
        ..record.clone()
    };
    let session = ViewerSession {
        id: Uuid::new_v4().to_string(),
        secret: Uuid::new_v4().to_string(),
        label: device_label
            .filter(|l| !l.trim().is_empty())
            .unwrap_or(&record.label)
            .to_string(),
        last_seen_at: Some(paired_at.clone()),
        paired_at,
        expires_at: (now + Duration::days(30)).to_rfc3339(),
    };

    Some((updated, session))
}

pub fn session_is_expired(session: &ViewerSession) -> bool {
    DateTime::parse_from_rfc3339(&session.expires_at)
        .map(|dt| dt < Utc::now())
        .unwrap_or(true)
}

pub fn touch_viewer_session(session: &ViewerSession) -> ViewerSession {
    ViewerSession {
        last_seen_at: Some(Utc::now().to_rfc3339()),
        ..session.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_generates_token() {
        let record = request_pairing(None);
        assert!(!record.token.is_empty());
        assert_eq!(record.code.len(), 8);
    }
    #[test]
    fn expired_pairing_cannot_be_approved() {
        let record = PairingRecord {
            id: "id".into(),
            token: "t".into(),
            code: "ABCDEFGH".into(),
            requested_at: Utc::now().to_rfc3339(),
            expires_at: (Utc::now() - Duration::minutes(1)).to_rfc3339(),
            approved: false,
            claimed_at: None,
            label: "x".into(),
        };
        assert!(approve_pairing(&record).is_none());
    }

    #[test]
    fn claim_pairing_creates_viewer_session() {
        let record = request_pairing(Some("Office iPad"));
        let (_, session) = claim_pairing(&record, None).expect("pairing should claim");
        assert_eq!(session.label, "Office iPad");
        assert!(!session.secret.is_empty());
    }
}
