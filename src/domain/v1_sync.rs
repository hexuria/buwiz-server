//! Typed V1 stubs for year / forms / drafts / filings.
//!
//! Persistence and HTTP are not required in this slice. Command and event
//! names match the V1 command set. Authz: the application loads the tax
//! profile, resolves the caller's [`ProfileAccess`] (owner, holding firm, or
//! a pass), and each command checks the one permission it needs.
//! COR/OCR ledgers, inference flags, and mailbox secrets are not modeled.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::domain::pass::PassPermission;
use crate::domain::tax_profile::ProfileAccess;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearFormEntry {
    pub form_code: String,
    pub frequency: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncCommand {
    CloneProfileYear {
        tax_profile_id: String,
        from_year: Option<i16>,
        to_year: i16,
        dest_forms_empty: bool,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    UpdateProfileYear {
        profile_year_id: String,
        /// Null fields inherit from the tax profile.
        registered_name: Option<String>,
        rdo_code: Option<String>,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    SetYearForms {
        profile_year_id: String,
        entries: Vec<YearFormEntry>,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    ActivateYearForm {
        profile_year_id: String,
        form_code: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    DeactivateYearForm {
        profile_year_id: String,
        form_code: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    UpsertFormDraft {
        tax_profile_id: String,
        form_code: String,
        tax_year: i16,
        period_key: String,
        payload: String,
        client_rev: u64,
        /// Whether the draft already exists: updating needs `drafts.update`,
        /// a new draft needs `drafts.save`.
        draft_exists: bool,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    MarkDraftSaved {
        form_draft_id: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    /// A checked-off step before anyone publishes.
    MarkDraftReviewed {
        form_draft_id: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    EnqueueFiling {
        form_draft_id: String,
        tax_profile_id: String,
        form_active: bool,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    MarkFilingSubmitted {
        filing_id: String,
        xml_sha256: String,
        submitted_at: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
    },
    ConfirmFilingFromReceipt {
        filing_id: Option<String>,
        tax_profile_id: Option<String>,
        period_key: Option<String>,
        receipt_ref: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    FailFiling {
        filing_id: String,
        error_summary: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    MarkFilingPaid {
        filing_id: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    /// V1.1 stub only.
    SaveFormTemplate {
        tax_profile_id: String,
        form_code: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
    /// V1.1 stub only.
    ApplyFormTemplate {
        tax_profile_id: String,
        template_id: String,
        actor_account_id: String,
        /// The caller's standing on the tax profile, resolved by the application.
        actor_access: ProfileAccess,
        occurred_at: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncEvent {
    ProfileYearCloned {
        tax_profile_id: String,
        from_year: Option<i16>,
        to_year: i16,
        copied_forms: bool,
        actor_account_id: String,
        occurred_at: String,
    },
    ProfileYearUpdated {
        profile_year_id: String,
        actor_account_id: String,
        occurred_at: String,
    },
    YearFormsSet {
        profile_year_id: String,
        entries: Vec<YearFormEntry>,
        actor_account_id: String,
        occurred_at: String,
    },
    YearFormActivated {
        profile_year_id: String,
        form_code: String,
        actor_account_id: String,
        occurred_at: String,
    },
    YearFormDeactivated {
        profile_year_id: String,
        form_code: String,
        actor_account_id: String,
        occurred_at: String,
    },
    FormDraftUpserted {
        tax_profile_id: String,
        form_code: String,
        tax_year: i16,
        period_key: String,
        rev: u64,
        saved_once: bool,
        actor_account_id: String,
        occurred_at: String,
    },
    DraftSaved {
        form_draft_id: String,
        actor_account_id: String,
        occurred_at: String,
    },
    DraftReviewed {
        form_draft_id: String,
        actor_account_id: String,
        occurred_at: String,
    },
    FilingQueued {
        form_draft_id: String,
        tax_profile_id: String,
        actor_account_id: String,
        occurred_at: String,
    },
    FilingSubmitted {
        filing_id: String,
        xml_sha256: String,
        submitted_at: String,
        actor_account_id: String,
    },
    FilingConfirmed {
        filing_id: Option<String>,
        receipt_ref: String,
        actor_account_id: String,
        occurred_at: String,
    },
    FilingFailed {
        filing_id: String,
        error_summary: String,
        actor_account_id: String,
        occurred_at: String,
    },
    FilingPaid {
        filing_id: String,
        actor_account_id: String,
        occurred_at: String,
    },
    FormTemplateSaved {
        tax_profile_id: String,
        form_code: String,
        actor_account_id: String,
        occurred_at: String,
    },
    FormTemplateApplied {
        tax_profile_id: String,
        template_id: String,
        actor_account_id: String,
        occurred_at: String,
    },
}

impl SyncEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ProfileYearCloned { .. } => "profile_year.cloned",
            Self::ProfileYearUpdated { .. } => "profile_year.updated",
            Self::YearFormsSet { .. } => "year_forms.set",
            Self::YearFormActivated { .. } => "year_form.activated",
            Self::YearFormDeactivated { .. } => "year_form.deactivated",
            Self::FormDraftUpserted { .. } => "form_draft.upserted",
            Self::DraftSaved { .. } => "form_draft.saved",
            Self::DraftReviewed { .. } => "form_draft.reviewed",
            Self::FilingQueued { .. } => "filing.queued",
            Self::FilingSubmitted { .. } => "filing.submitted",
            Self::FilingConfirmed { .. } => "filing.confirmed",
            Self::FilingFailed { .. } => "filing.failed",
            Self::FilingPaid { .. } => "filing.paid",
            Self::FormTemplateSaved { .. } => "form_template.saved",
            Self::FormTemplateApplied { .. } => "form_template.applied",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncError {
    /// The caller's standing lacks the permission this step needs.
    NotAllowed { needs: PassPermission },
    Invalid { reason: String },
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAllowed { needs } => {
                write!(f, "this needs the `{}` permission", needs.as_str())
            }
            Self::Invalid { reason } => write!(f, "{reason}"),
        }
    }
}

fn require(access: ProfileAccess, needs: PassPermission) -> Result<(), SyncError> {
    if access.has(needs) {
        Ok(())
    } else {
        Err(SyncError::NotAllowed { needs })
    }
}

impl SyncCommand {
    pub fn handle(self) -> Result<SyncEvent, SyncError> {
        match self {
            Self::CloneProfileYear {
                tax_profile_id,
                from_year,
                to_year,
                dest_forms_empty,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::YearsManage)?;
                if tax_profile_id.trim().is_empty() {
                    return Err(SyncError::Invalid {
                        reason: "tax_profile_id is required".to_owned(),
                    });
                }
                Ok(SyncEvent::ProfileYearCloned {
                    tax_profile_id,
                    from_year,
                    to_year,
                    copied_forms: dest_forms_empty,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::UpdateProfileYear {
                profile_year_id,
                actor_account_id,
                actor_access,
                occurred_at,
                ..
            } => {
                require(actor_access, PassPermission::YearsManage)?;
                Ok(SyncEvent::ProfileYearUpdated {
                    profile_year_id,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::SetYearForms {
                profile_year_id,
                entries,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::YearsManage)?;
                Ok(SyncEvent::YearFormsSet {
                    profile_year_id,
                    entries,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::ActivateYearForm {
                profile_year_id,
                form_code,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::YearsManage)?;
                Ok(SyncEvent::YearFormActivated {
                    profile_year_id,
                    form_code,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::DeactivateYearForm {
                profile_year_id,
                form_code,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::YearsManage)?;
                Ok(SyncEvent::YearFormDeactivated {
                    profile_year_id,
                    form_code,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::UpsertFormDraft {
                tax_profile_id,
                form_code,
                tax_year,
                period_key,
                client_rev,
                draft_exists,
                actor_account_id,
                actor_access,
                occurred_at,
                ..
            } => {
                require(
                    actor_access,
                    if draft_exists {
                        PassPermission::DraftsUpdate
                    } else {
                        PassPermission::DraftsSave
                    },
                )?;
                Ok(SyncEvent::FormDraftUpserted {
                    tax_profile_id,
                    form_code,
                    tax_year,
                    period_key,
                    rev: client_rev.saturating_add(1),
                    saved_once: true,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::MarkDraftSaved {
                form_draft_id,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::DraftsSave)?;
                Ok(SyncEvent::DraftSaved {
                    form_draft_id,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::MarkDraftReviewed {
                form_draft_id,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::DraftsReview)?;
                Ok(SyncEvent::DraftReviewed {
                    form_draft_id,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::EnqueueFiling {
                form_draft_id,
                tax_profile_id,
                form_active,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::FilingsPublish)?;
                if !form_active {
                    return Err(SyncError::Invalid {
                        reason: "form is not active in the year set".to_owned(),
                    });
                }
                Ok(SyncEvent::FilingQueued {
                    form_draft_id,
                    tax_profile_id,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::MarkFilingSubmitted {
                filing_id,
                xml_sha256,
                submitted_at,
                actor_account_id,
                actor_access,
            } => {
                require(actor_access, PassPermission::FilingsPublish)?;
                Ok(SyncEvent::FilingSubmitted {
                    filing_id,
                    xml_sha256,
                    submitted_at,
                    actor_account_id,
                })
            }
            Self::ConfirmFilingFromReceipt {
                filing_id,
                receipt_ref,
                actor_account_id,
                actor_access,
                occurred_at,
                ..
            } => {
                require(actor_access, PassPermission::FilingsPublish)?;
                Ok(SyncEvent::FilingConfirmed {
                    filing_id,
                    receipt_ref,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::FailFiling {
                filing_id,
                error_summary,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::FilingsPublish)?;
                Ok(SyncEvent::FilingFailed {
                    filing_id,
                    error_summary,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::MarkFilingPaid {
                filing_id,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::FilingsPublish)?;
                Ok(SyncEvent::FilingPaid {
                    filing_id,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::SaveFormTemplate {
                tax_profile_id,
                form_code,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::DraftsSave)?;
                Ok(SyncEvent::FormTemplateSaved {
                    tax_profile_id,
                    form_code,
                    actor_account_id,
                    occurred_at,
                })
            }
            Self::ApplyFormTemplate {
                tax_profile_id,
                template_id,
                actor_account_id,
                actor_access,
                occurred_at,
            } => {
                require(actor_access, PassPermission::DraftsSave)?;
                Ok(SyncEvent::FormTemplateApplied {
                    tax_profile_id,
                    template_id,
                    actor_account_id,
                    occurred_at,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pass::{PassPermissions, PassPreset};

    const AT: &str = "2026-01-01T00:00:00Z";

    fn safe_pass() -> ProfileAccess {
        ProfileAccess::Pass(PassPreset::Safe.permissions())
    }

    fn clone_cmd(dest_empty: bool, access: ProfileAccess) -> SyncCommand {
        SyncCommand::CloneProfileYear {
            tax_profile_id: "tp-1".into(),
            from_year: Some(2025),
            to_year: 2026,
            dest_forms_empty: dest_empty,
            actor_account_id: "alice".into(),
            actor_access: access,
            occurred_at: AT.into(),
        }
    }

    #[test]
    fn clone_copies_forms_only_when_dest_empty() {
        let copied = clone_cmd(true, ProfileAccess::Owner).handle().unwrap();
        assert!(matches!(
            copied,
            SyncEvent::ProfileYearCloned {
                copied_forms: true,
                ..
            }
        ));
        let skipped = clone_cmd(false, ProfileAccess::Owner).handle().unwrap();
        assert!(matches!(
            skipped,
            SyncEvent::ProfileYearCloned {
                copied_forms: false,
                ..
            }
        ));
    }

    #[test]
    fn managing_years_needs_years_manage() {
        assert_eq!(
            clone_cmd(true, safe_pass()).handle().unwrap_err(),
            SyncError::NotAllowed {
                needs: PassPermission::YearsManage
            }
        );
        let full = ProfileAccess::Pass(PassPreset::Full.permissions());
        clone_cmd(true, full).handle().unwrap();
    }

    fn enqueue(access: ProfileAccess, form_active: bool) -> SyncCommand {
        SyncCommand::EnqueueFiling {
            form_draft_id: "d1".into(),
            tax_profile_id: "tp-1".into(),
            form_active,
            actor_account_id: "bob".into(),
            actor_access: access,
            occurred_at: AT.into(),
        }
    }

    #[test]
    fn enqueue_requires_publish_and_active_form() {
        for access in [
            ProfileAccess::None,
            ProfileAccess::OwnerReadOnly,
            ProfileAccess::Pass(PassPreset::Full.permissions()),
        ] {
            assert_eq!(
                enqueue(access, true).handle().unwrap_err(),
                SyncError::NotAllowed {
                    needs: PassPermission::FilingsPublish
                }
            );
        }
        let err = enqueue(ProfileAccess::Owner, false).handle().unwrap_err();
        assert!(matches!(err, SyncError::Invalid { .. }));
        let queued = enqueue(ProfileAccess::Owner, true).handle().unwrap();
        assert_eq!(queued.event_type(), "filing.queued");
        let publisher = ProfileAccess::Pass(
            PassPreset::Safe
                .permissions()
                .with(PassPermission::FilingsPublish),
        );
        enqueue(publisher, true).handle().unwrap();
    }

    fn upsert(access: ProfileAccess, draft_exists: bool) -> SyncCommand {
        SyncCommand::UpsertFormDraft {
            tax_profile_id: "tp-1".into(),
            form_code: "1701Q".into(),
            tax_year: 2026,
            period_key: "2026-Q1".into(),
            payload: "{\"a\":1}".into(),
            client_rev: 3,
            draft_exists,
            actor_account_id: "alice".into(),
            actor_access: access,
            occurred_at: AT.into(),
        }
    }

    #[test]
    fn upsert_draft_bumps_rev_and_marks_saved_once() {
        let event = upsert(ProfileAccess::Owner, true).handle().unwrap();
        assert!(matches!(
            event,
            SyncEvent::FormDraftUpserted {
                rev: 4,
                saved_once: true,
                ..
            }
        ));
    }

    #[test]
    fn saving_and_updating_drafts_are_separate_rights() {
        let save_only = ProfileAccess::Pass(PassPermissions::of(&[
            PassPermission::DraftsView,
            PassPermission::DraftsSave,
        ]));
        upsert(save_only, false).handle().unwrap();
        assert_eq!(
            upsert(save_only, true).handle().unwrap_err(),
            SyncError::NotAllowed {
                needs: PassPermission::DraftsUpdate
            }
        );
        let read_only = ProfileAccess::Pass(PassPreset::ReadOnly.permissions());
        assert!(upsert(read_only, false).handle().is_err());
    }

    #[test]
    fn reviewing_needs_drafts_review() {
        let review = |access| SyncCommand::MarkDraftReviewed {
            form_draft_id: "d1".into(),
            actor_account_id: "bob".into(),
            actor_access: access,
            occurred_at: AT.into(),
        };
        let event = review(safe_pass()).handle().unwrap();
        assert_eq!(event.event_type(), "form_draft.reviewed");
        let read_only = ProfileAccess::Pass(PassPreset::ReadOnly.permissions());
        assert!(review(read_only).handle().is_err());
    }

    #[test]
    fn event_shapes_round_trip() {
        let event = SyncEvent::FilingConfirmed {
            filing_id: Some("f1".into()),
            receipt_ref: "R-1".into(),
            actor_account_id: "alice".into(),
            occurred_at: AT.into(),
        };
        let value = serde_json::to_value(&event).unwrap();
        let decoded: SyncEvent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, event);
    }
}
