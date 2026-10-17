//! Access passes: what a firm (and, later, an AI agent) may do with someone
//! else's tax profile.
//!
//! The owner gives a pass, or approves a firm's request for one, and can end
//! it at any time. Any member of the firm can use an active pass. Sharing,
//! ending passes, archiving and deleting stay with the owner; no pass grants
//! them.
//!
//! The server never files with BIR. `filings.publish` is a right the
//! grantee's own installed buwiz-forms app checks before it sends through its
//! tunnel. It is never part of a preset and must be switched on by itself.

use serde::{Deserialize, Serialize};

/// One thing a pass can allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PassPermission {
    ProfileView,
    ProfileEdit,
    YearsView,
    YearsManage,
    DraftsView,
    DraftsSave,
    DraftsUpdate,
    DraftsReview,
    FilingsView,
    FilingsPublish,
}

impl PassPermission {
    /// Every permission, in display order. `FilingsPublish` is last and
    /// stands apart from the rest.
    pub const ALL: [Self; 10] = [
        Self::ProfileView,
        Self::ProfileEdit,
        Self::YearsView,
        Self::YearsManage,
        Self::DraftsView,
        Self::DraftsSave,
        Self::DraftsUpdate,
        Self::DraftsReview,
        Self::FilingsView,
        Self::FilingsPublish,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ProfileView => "profile.view",
            Self::ProfileEdit => "profile.edit",
            Self::YearsView => "years.view",
            Self::YearsManage => "years.manage",
            Self::DraftsView => "drafts.view",
            Self::DraftsSave => "drafts.save",
            Self::DraftsUpdate => "drafts.update",
            Self::DraftsReview => "drafts.review",
            Self::FilingsView => "filings.view",
            Self::FilingsPublish => "filings.publish",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ProfileView => "See taxpayer details",
            Self::ProfileEdit => "Edit taxpayer details",
            Self::YearsView => "See tax years and their forms",
            Self::YearsManage => "Add tax years, choose forms",
            Self::DraftsView => "See drafts",
            Self::DraftsSave => "Save a new draft",
            Self::DraftsUpdate => "Update an existing draft",
            Self::DraftsReview => "Mark a draft reviewed",
            Self::FilingsView => "See filing history and BIR confirmations",
            Self::FilingsPublish => "Publish to BIR",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        Self::ALL.into_iter().find(|p| p.as_str() == raw)
    }

    /// The read permissions this one cannot work without.
    fn needs(self) -> &'static [Self] {
        match self {
            Self::ProfileEdit => &[Self::ProfileView],
            Self::YearsManage => &[Self::YearsView],
            Self::DraftsSave | Self::DraftsUpdate | Self::DraftsReview => &[Self::DraftsView],
            Self::FilingsPublish => &[Self::DraftsView, Self::FilingsView],
            Self::ProfileView | Self::YearsView | Self::DraftsView | Self::FilingsView => &[],
        }
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

/// A set of [`PassPermission`]s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassPermissions(u16);

impl PassPermissions {
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Everything, publishing included. Only the profile's controller has it.
    pub fn all() -> Self {
        Self::of(&PassPermission::ALL)
    }

    pub fn of(permissions: &[PassPermission]) -> Self {
        permissions.iter().fold(Self::empty(), |set, p| set.with(*p))
    }

    #[must_use]
    pub fn contains(self, permission: PassPermission) -> bool {
        self.0 & permission.bit() != 0
    }

    #[must_use]
    pub fn with(self, permission: PassPermission) -> Self {
        Self(self.0 | permission.bit())
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[must_use]
    pub fn without(self, permission: PassPermission) -> Self {
        Self(self.0 & !permission.bit())
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = PassPermission> {
        PassPermission::ALL
            .into_iter()
            .filter(move |p| self.contains(*p))
    }

    pub fn ids(self) -> Vec<String> {
        self.iter().map(|p| p.as_str().to_owned()).collect()
    }

    /// Adds the read permission each write permission needs, so a pass can
    /// never edit what it cannot see.
    #[must_use]
    pub fn with_needs(self) -> Self {
        self.iter().fold(self, |set, p| {
            p.needs().iter().fold(set, |set, need| set.with(*need))
        })
    }

    /// Permissions as chosen on the give-access screen. Publishing is its own
    /// switch (`allow_publish`); naming it in the list is refused so no
    /// preset or copied list can carry it by accident.
    pub fn from_choice(ids: &[String], allow_publish: bool) -> Result<Self, String> {
        let mut set = Self::empty();
        for id in ids {
            let permission =
                PassPermission::parse(id).ok_or_else(|| format!("unknown permission `{id}`"))?;
            if permission == PassPermission::FilingsPublish {
                return Err(
                    "publishing to BIR is a separate switch, not part of the permission list"
                        .to_owned(),
                );
            }
            set = set.with(permission);
        }
        if allow_publish {
            set = set.with(PassPermission::FilingsPublish);
        }
        let set = set.with_needs();
        if set.is_empty() {
            return Err("a pass needs at least one permission".to_owned());
        }
        Ok(set)
    }

    /// Permissions read back from a stored event. Unknown ids are skipped so
    /// an older server can still replay a newer stream.
    pub fn from_stored(ids: &[String]) -> Self {
        ids.iter()
            .filter_map(|id| PassPermission::parse(id))
            .fold(Self::empty(), Self::with)
    }

    /// The preset these permissions match exactly, if any.
    pub fn preset(self) -> Option<PassPreset> {
        PassPreset::ALL
            .into_iter()
            .find(|preset| preset.permissions() == self)
    }
}

/// Starting points on the give-access screen. None of them can publish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassPreset {
    ReadOnly,
    Safe,
    Full,
}

impl PassPreset {
    pub const ALL: [Self; 3] = [Self::ReadOnly, Self::Safe, Self::Full];
    /// Selected when the give-access screen opens.
    pub const DEFAULT: Self = Self::Safe;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Safe => "safe",
            Self::Full => "full",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "Read-only",
            Self::Safe => "Safe",
            Self::Full => "Full",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        Self::ALL.into_iter().find(|preset| preset.as_str() == raw)
    }

    pub fn permissions(self) -> PassPermissions {
        use PassPermission::*;
        let read_only = PassPermissions::of(&[ProfileView, YearsView, DraftsView, FilingsView]);
        match self {
            Self::ReadOnly => read_only,
            Self::Safe => read_only
                .with(DraftsSave)
                .with(DraftsUpdate)
                .with(DraftsReview),
            Self::Full => PassPermissions::of(&[
                ProfileView,
                ProfileEdit,
                YearsView,
                YearsManage,
                DraftsView,
                DraftsSave,
                DraftsUpdate,
                DraftsReview,
                FilingsView,
            ]),
        }
    }
}

/// Who a pass is for. Agents join in a later phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassGrantee {
    Organization { organization_id: String },
}

impl PassGrantee {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Organization { .. } => "organization",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Organization { organization_id } => organization_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassStatus {
    /// A firm asked; waiting for the owner.
    Requested,
    /// The owner offered; waiting for the firm.
    Offered,
    Active,
    /// Ended before it started.
    Declined,
    /// Ended after it started.
    Revoked,
    /// Ran past its end date (`TaxProfileEvent::PassExpired`).
    Expired,
}

impl PassStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Offered => "offered",
            Self::Active => "active",
            Self::Declined => "declined",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    pub fn is_open(self) -> bool {
        matches!(self, Self::Requested | Self::Offered | Self::Active)
    }

    pub fn is_pending(self) -> bool {
        matches!(self, Self::Requested | Self::Offered)
    }
}

/// One pass on one tax profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pass {
    pub pass_id: String,
    pub grantee: PassGrantee,
    pub permissions: PassPermissions,
    pub status: PassStatus,
    /// The pass stops working at this instant; `None` means until ended.
    pub expires_at: Option<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub ended_by: Option<String>,
}

impl Pass {
    /// Active and not past its end date.
    pub fn is_usable(&self, now: &str) -> bool {
        self.status == PassStatus::Active && !self.is_expired(now)
    }

    pub fn is_expired(&self, now: &str) -> bool {
        self.expires_at
            .as_deref()
            .is_some_and(|end| !instant_before(now, end))
    }

    /// Status as people see it: an active pass past its end date reads
    /// "expired". (The pass list computes the same in SQL.)
    #[cfg(test)]
    pub fn status_label(&self, now: &str) -> &'static str {
        if self.status == PassStatus::Active && self.is_expired(now) {
            "expired"
        } else {
            self.status.as_str()
        }
    }
}

/// Whether RFC 3339 instant `a` is before `b`, to the second, with any UTC
/// offset (`Z`, `+08:00`, `-05:00`). An instant that does not parse is never
/// "before" anything, so a broken end date reads as already passed (fails
/// closed).
pub fn instant_before(a: &str, b: &str) -> bool {
    match (utc_seconds(a), utc_seconds(b)) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

/// Seconds since 1970-01-01T00:00:00Z of an RFC 3339 date-time
/// (`YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`); fractions are dropped.
pub fn utc_seconds(value: &str) -> Option<i64> {
    let b = value.trim().as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b't' | b' ')
        || b[13] != b':' || b[16] != b':'
    {
        return None;
    }
    let num = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = b.get(range)?;
        part.iter().all(u8::is_ascii_digit).then(|| {
            part.iter().fold(0_i64, |n, d| n * 10 + i64::from(d - b'0'))
        })
    };
    let (year, month, day) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (hour, minute, second) = (num(11..13)?, num(14..16)?, num(17..19)?);
    if !valid_date(year, month, day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut rest = &b[19..];
    if let Some(fraction) = rest.strip_prefix(b".") {
        let digits = fraction.iter().take_while(|d| d.is_ascii_digit()).count();
        if digits == 0 {
            return None;
        }
        rest = &fraction[digits..];
    }
    let offset = match rest {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), h1, h2, b':', m1, m2] => {
            let digits = [*h1, *h2, *m1, *m2];
            if !digits.iter().all(u8::is_ascii_digit) {
                return None;
            }
            let hours = i64::from(h1 - b'0') * 10 + i64::from(h2 - b'0');
            let minutes = i64::from(m1 - b'0') * 10 + i64::from(m2 - b'0');
            if hours > 23 || minutes > 59 {
                return None;
            }
            let total = hours * 3600 + minutes * 60;
            if *sign == b'+' { total } else { -total }
        }
        _ => return None,
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Whether `year-month-day` is a real calendar date (proleptic Gregorian).
fn valid_date(year: i64, month: i64, day: i64) -> bool {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// Days from 1970-01-01 to `y-m-d` (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The instant a pass with end date `date` (`YYYY-MM-DD`) stops working: the
/// end of that day in Philippine time (UTC+8), i.e. 16:00 UTC.
pub fn pass_end_from_date(date: &str) -> Result<String, String> {
    let date = date.trim();
    let bytes = date.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !shaped {
        return Err("end date must look like 2027-03-31".to_owned());
    }
    let part = |range: std::ops::Range<usize>| date[range].parse::<i64>().unwrap_or(0);
    if !valid_date(part(0..4), part(5..7), part(8..10)) {
        return Err(format!("{date} is not a calendar date"));
    }
    Ok(format!("{date}T16:00:00.000Z"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn stored_names_and_labels_are_fixed() {
        // These strings are written to the database and shown to people.
        let statuses: Vec<_> = [
            PassStatus::Requested,
            PassStatus::Offered,
            PassStatus::Active,
            PassStatus::Declined,
            PassStatus::Revoked,
        ]
        .map(PassStatus::as_str)
        .to_vec();
        assert_eq!(statuses, ["requested", "offered", "active", "declined", "revoked"]);
        assert_eq!(
            PassGrantee::Organization { organization_id: "o".into() }.kind(),
            "organization"
        );
        assert_eq!(PassPermission::FilingsPublish.label(), "Publish to BIR");
        assert_eq!(PassPreset::Safe.label(), "Safe");
        let labels: std::collections::BTreeSet<_> =
            PassPermission::ALL.iter().map(|p| p.label()).collect();
        assert_eq!(labels.len(), PassPermission::ALL.len(), "labels are distinct");
        assert!(labels.iter().all(|label| !label.is_empty()));
    }

    #[test]
    fn union_of_overlapping_sets_keeps_each_once() {
        let safe = PassPreset::Safe.permissions();
        let read = PassPreset::ReadOnly.permissions();
        assert_eq!(safe.union(read), safe);
        assert_eq!(read.union(read), read);
    }

    #[test]
    fn leap_years_follow_the_gregorian_rule() {
        for ok in ["2024-02-29", "2000-02-29"] {
            assert!(pass_end_from_date(ok).is_ok(), "{ok}");
        }
        for bad in ["1900-02-29", "2100-02-29", "2023-02-29"] {
            assert!(pass_end_from_date(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn malformed_instants_do_not_parse() {
        assert_eq!(utc_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(utc_seconds("1970-01-01T00:00:00.5Z"), Some(0));
        // RFC 3339 allows a leap second.
        assert!(utc_seconds("2016-12-31T23:59:60Z").is_some());
        for bad in [
            "2027-03-31X16:00:00Z",
            "2027/03-31T16:00:00Z",
            "2027-03/31T16:00:00Z",
            "2027-03-31T16-00:00Z",
            "2027-03-31T16:00-00Z",
            "2027-03-31T24:00:00Z",
            "2027-03-31T23:60:00Z",
            "2027-03-31T23:59:61Z",
            "2027-03-31T16:00:00+24:00",
            "2027-03-31T16:00:00+08:60",
            "2027-03-31T16:00:00+0a:00",
            "2027-03-31T16:00:00.Z",
            "2027-03-31T16:00:00",
        ] {
            assert_eq!(utc_seconds(bad), None, "{bad}");
        }
    }

    #[test]
    fn end_dates_must_be_calendar_dates() {
        assert_eq!(pass_end_from_date("2027-03-31").unwrap(), "2027-03-31T16:00:00.000Z");
        assert!(pass_end_from_date("2028-02-29").is_ok(), "leap day");
        for bad in ["2027-02-29", "2027-02-30", "2027-04-31", "2027-13-01", "2027-00-10", "27-03-31", "2027/03/31", ""] {
            assert!(pass_end_from_date(bad).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn instants_compare_across_offsets_and_fail_closed() {
        // 16:00Z and 00:00+08:00 the next day are the same instant.
        assert!(!instant_before("2027-04-01T00:00:00+08:00", "2027-03-31T16:00:00.000Z"));
        assert!(!instant_before("2027-03-31T16:00:00.000Z", "2027-04-01T00:00:00+08:00"));
        assert!(instant_before("2027-03-31T23:59:59+08:00", "2027-03-31T16:00:00Z"));
        assert!(!instant_before("garbage", "2027-03-31T16:00:00Z"));
        assert!(!instant_before("2027-03-31T16:00:00Z", "garbage"));
        assert!(!instant_before("2027-02-30T00:00:00Z", "2099-01-01T00:00:00Z"));
    }

    mod oracle {
        use super::super::*;
        use proptest::prelude::*;

        proptest! {
            /// Oracle: chrono's RFC 3339 parser.
            #[test]
            fn utc_seconds_agrees_with_chrono(
                secs in -2_000_000_000_i64..8_000_000_000_i64,
                offset_minutes in -(23 * 60 + 59)..=(23 * 60 + 59_i32),
                millis in proptest::option::of(0u32..1000),
            ) {
                use chrono::{FixedOffset, TimeZone, SecondsFormat};
                let tz = FixedOffset::east_opt(offset_minutes * 60).unwrap();
                let at = tz.timestamp_opt(secs, millis.unwrap_or(0) * 1_000_000).unwrap();
                let text = at.to_rfc3339_opts(
                    if millis.is_some() { SecondsFormat::Millis } else { SecondsFormat::Secs },
                    offset_minutes == 0,
                );
                prop_assert_eq!(utc_seconds(&text), Some(secs), "{}", text);
            }

            #[test]
            fn utc_seconds_never_panics(raw in "\\PC{0,40}") {
                let _ = utc_seconds(&raw);
            }
        }
    }

    #[test]
    fn no_preset_can_publish() {
        for preset in PassPreset::ALL {
            assert!(!preset.permissions().contains(PassPermission::FilingsPublish));
        }
        assert_eq!(PassPreset::DEFAULT, PassPreset::Safe);
    }

    #[test]
    fn presets_match_the_agreed_lists() {
        assert_eq!(
            PassPreset::ReadOnly.permissions().ids(),
            ids(&["profile.view", "years.view", "drafts.view", "filings.view"])
        );
        assert_eq!(
            PassPreset::Safe.permissions().ids(),
            ids(&[
                "profile.view",
                "years.view",
                "drafts.view",
                "drafts.save",
                "drafts.update",
                "drafts.review",
                "filings.view"
            ])
        );
        assert_eq!(PassPreset::Full.permissions().ids().len(), 9);
        assert_eq!(
            PassPermissions::all(),
            PassPreset::Full.permissions().with(PassPermission::FilingsPublish)
        );
    }

    #[test]
    fn publish_only_comes_from_its_own_switch() {
        let err = PassPermissions::from_choice(&ids(&["filings.publish"]), false).unwrap_err();
        assert!(err.contains("separate switch"));
        let set = PassPermissions::from_choice(&ids(&["profile.view"]), true).unwrap();
        assert!(set.contains(PassPermission::FilingsPublish));
        // Publishing needs to see drafts and filings.
        assert!(set.contains(PassPermission::DraftsView));
        assert!(set.contains(PassPermission::FilingsView));
    }

    #[test]
    fn writes_bring_their_reads() {
        let set = PassPermissions::from_choice(&ids(&["profile.edit", "drafts.update"]), false)
            .unwrap();
        assert!(set.contains(PassPermission::ProfileView));
        assert!(set.contains(PassPermission::DraftsView));
        assert!(!set.contains(PassPermission::YearsView));
    }

    #[test]
    fn bad_choices_are_refused() {
        assert!(PassPermissions::from_choice(&ids(&["drafts.delete"]), false).is_err());
        assert!(PassPermissions::from_choice(&[], false).is_err());
    }

    #[test]
    fn preset_is_recognised_from_its_permissions() {
        let safe = PassPermissions::from_choice(&PassPreset::Safe.permissions().ids(), false)
            .unwrap();
        assert_eq!(safe.preset(), Some(PassPreset::Safe));
        assert_eq!(safe.with(PassPermission::FilingsPublish).preset(), None);
        assert_eq!(PassPreset::parse("Read-only"), Some(PassPreset::ReadOnly));
    }

    #[test]
    fn end_date_is_end_of_day_in_manila() {
        assert_eq!(
            pass_end_from_date("2027-03-31").unwrap(),
            "2027-03-31T16:00:00.000Z"
        );
        assert!(pass_end_from_date("31/03/2027").is_err());
        assert!(pass_end_from_date("2027-13-01").is_err());
        assert!(instant_before("2027-03-31T15:59:59.999Z", "2027-03-31T16:00:00.000Z"));
        assert!(!instant_before("2027-03-31T16:00:00Z", "2027-03-31T16:00:00.000Z"));
    }
}
