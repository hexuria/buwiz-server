//! Typed V1 stubs for year / forms / drafts / filings.
//!
//! Persistence and HTTP are not required in this slice. Command and event
//! names match the V1 command set. Authz is UUID load then `account_id`.
//! COR/OCR ledgers, inference flags, and mailbox secrets are not modeled.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

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
        owner_account_id: String,
        occurred_at: String,
    },
    UpdateProfileYear {
        profile_year_id: String,
        /// Null fields inherit from the tax profile.
        registered_name: Option<String>,
        rdo_code: Option<String>,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    SetYearForms {
        profile_year_id: String,
        entries: Vec<YearFormEntry>,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    ActivateYearForm {
        profile_year_id: String,
        form_code: String,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    DeactivateYearForm {
        profile_year_id: String,
        form_code: String,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    UpsertFormDraft {
        tax_profile_id: String,
        form_code: String,
        tax_year: i16,
        period_key: String,
        payload: String,
        client_rev: u64,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    MarkDraftSaved {
        form_draft_id: String,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    EnqueueFiling {
        form_draft_id: String,
        tax_profile_id: String,
        form_active: bool,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    MarkFilingSubmitted {
        filing_id: String,
        xml_sha256: String,
        submitted_at: String,
        actor_account_id: String,
        owner_account_id: String,
    },
    ConfirmFilingFromReceipt {
        filing_id: Option<String>,
        tax_profile_id: Option<String>,
        period_key: Option<String>,
        receipt_ref: String,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    FailFiling {
        filing_id: String,
        error_summary: String,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    MarkFilingPaid {
        filing_id: String,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    /// V1.1 stub only.
    SaveFormTemplate {
        tax_profile_id: String,
        form_code: String,
        actor_account_id: String,
        owner_account_id: String,
        occurred_at: String,
    },
    /// V1.1 stub only.
    ApplyFormTemplate {
        tax_profile_id: String,
        template_id: String,
        actor_account_id: String,
        owner_account_id: String,
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
    NotHolder,
    Invalid { reason: String },
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotHolder => write!(f, "caller is not the tax-profile account"),
            Self::Invalid { reason } => write!(f, "{reason}"),
        }
    }
}

fn require_account(actor: &str, owner: &str) -> Result<(), SyncError> {
    if actor == owner && !actor.is_empty() {
        Ok(())
    } else {
        Err(SyncError::NotHolder)
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
                ..
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                actor_account_id,
                owner_account_id,
                occurred_at,
                ..
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
                Ok(SyncEvent::DraftSaved {
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
                ..
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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
                owner_account_id,
                occurred_at,
            } => {
                require_account(&actor_account_id, &owner_account_id)?;
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

    fn clone_cmd(dest_empty: bool, actor: &str, owner: &str) -> SyncCommand {
        SyncCommand::CloneProfileYear {
            tax_profile_id: "tp-1".into(),
            from_year: Some(2025),
            to_year: 2026,
            dest_forms_empty: dest_empty,
            actor_account_id: actor.into(),
            owner_account_id: owner.into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn clone_copies_forms_only_when_dest_empty() {
        let copied = clone_cmd(true, "alice", "alice").handle().unwrap();
        assert!(matches!(
            copied,
            SyncEvent::ProfileYearCloned {
                copied_forms: true,
                ..
            }
        ));
        let skipped = clone_cmd(false, "alice", "alice").handle().unwrap();
        assert!(matches!(
            skipped,
            SyncEvent::ProfileYearCloned {
                copied_forms: false,
                ..
            }
        ));
    }

    #[test]
    fn enqueue_requires_account_and_active_form() {
        let err = SyncCommand::EnqueueFiling {
            form_draft_id: "d1".into(),
            tax_profile_id: "tp-1".into(),
            form_active: true,
            actor_account_id: "bob".into(),
            owner_account_id: "alice".into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
        .handle()
        .unwrap_err();
        assert_eq!(err, SyncError::NotHolder);
        let err = SyncCommand::EnqueueFiling {
            form_draft_id: "d1".into(),
            tax_profile_id: "tp-1".into(),
            form_active: false,
            actor_account_id: "alice".into(),
            owner_account_id: "alice".into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
        .handle()
        .unwrap_err();
        assert!(matches!(err, SyncError::Invalid { .. }));
        let queued = SyncCommand::EnqueueFiling {
            form_draft_id: "d1".into(),
            tax_profile_id: "tp-1".into(),
            form_active: true,
            actor_account_id: "alice".into(),
            owner_account_id: "alice".into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
        .handle()
        .unwrap();
        assert_eq!(queued.event_type(), "filing.queued");
    }

    #[test]
    fn upsert_draft_bumps_rev_and_marks_saved_once() {
        let event = SyncCommand::UpsertFormDraft {
            tax_profile_id: "tp-1".into(),
            form_code: "1701Q".into(),
            tax_year: 2026,
            period_key: "2026-Q1".into(),
            payload: "{\"a\":1}".into(),
            client_rev: 3,
            actor_account_id: "alice".into(),
            owner_account_id: "alice".into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        }
        .handle()
        .unwrap();
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
    fn event_shapes_round_trip() {
        let event = SyncEvent::FilingConfirmed {
            filing_id: Some("f1".into()),
            receipt_ref: "R-1".into(),
            actor_account_id: "alice".into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
        };
        let value = serde_json::to_value(&event).unwrap();
        let decoded: SyncEvent = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, event);
    }
}
