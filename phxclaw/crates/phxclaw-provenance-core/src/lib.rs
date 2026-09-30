#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseClass {
    Permissive,
    Copyleft,
    Proprietary,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionDecision {
    Allow,
    Quarantine,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonCode {
    VerifiedPermissiveLicense,
    LicenseEvidenceMissing,
    UnknownProvenance,
    CopyleftNeedsCompatibilityReview,
    ProprietarySource,
    DeclaredLeak,
    RedistributionProhibited,
    HashEvidenceMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceClaim<'a> {
    pub source_name: &'a str,
    pub license_class: LicenseClass,
    pub local_license_evidence: bool,
    pub archive_hash_verified: bool,
    pub origin_known: bool,
    pub declared_leak: bool,
    pub redistribution_prohibited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub decision: IngestionDecision,
    pub reasons: Vec<ReasonCode>,
}

pub fn evaluate(claim: &ProvenanceClaim<'_>) -> Evaluation {
    let mut reasons = Vec::new();

    if claim.declared_leak {
        reasons.push(ReasonCode::DeclaredLeak);
    }
    if claim.redistribution_prohibited {
        reasons.push(ReasonCode::RedistributionProhibited);
    }
    if claim.license_class == LicenseClass::Proprietary {
        reasons.push(ReasonCode::ProprietarySource);
    }
    if !reasons.is_empty() {
        return Evaluation {
            decision: IngestionDecision::Deny,
            reasons,
        };
    }

    if !claim.origin_known {
        reasons.push(ReasonCode::UnknownProvenance);
    }
    if !claim.local_license_evidence {
        reasons.push(ReasonCode::LicenseEvidenceMissing);
    }
    if !claim.archive_hash_verified {
        reasons.push(ReasonCode::HashEvidenceMissing);
    }
    if claim.license_class == LicenseClass::Copyleft {
        reasons.push(ReasonCode::CopyleftNeedsCompatibilityReview);
    }
    if claim.license_class == LicenseClass::Unknown {
        reasons.push(ReasonCode::UnknownProvenance);
    }

    if !reasons.is_empty() {
        return Evaluation {
            decision: IngestionDecision::Quarantine,
            reasons,
        };
    }

    Evaluation {
        decision: IngestionDecision::Allow,
        reasons: vec![ReasonCode::VerifiedPermissiveLicense],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ProvenanceClaim<'static> {
        ProvenanceClaim {
            source_name: "fixture",
            license_class: LicenseClass::Permissive,
            local_license_evidence: true,
            archive_hash_verified: true,
            origin_known: true,
            declared_leak: false,
            redistribution_prohibited: false,
        }
    }

    #[test]
    fn allows_verified_permissive_source() {
        assert_eq!(evaluate(&base()).decision, IngestionDecision::Allow);
    }

    #[test]
    fn quarantines_missing_local_license() {
        let mut c = base();
        c.local_license_evidence = false;
        assert_eq!(evaluate(&c).decision, IngestionDecision::Quarantine);
    }

    #[test]
    fn quarantines_unknown_origin() {
        let mut c = base();
        c.origin_known = false;
        assert_eq!(evaluate(&c).decision, IngestionDecision::Quarantine);
    }

    #[test]
    fn quarantines_missing_hash() {
        let mut c = base();
        c.archive_hash_verified = false;
        assert_eq!(evaluate(&c).decision, IngestionDecision::Quarantine);
    }

    #[test]
    fn quarantines_copyleft_for_review() {
        let mut c = base();
        c.license_class = LicenseClass::Copyleft;
        assert_eq!(evaluate(&c).decision, IngestionDecision::Quarantine);
    }

    #[test]
    fn denies_proprietary() {
        let mut c = base();
        c.license_class = LicenseClass::Proprietary;
        assert_eq!(evaluate(&c).decision, IngestionDecision::Deny);
    }

    #[test]
    fn denies_declared_leak() {
        let mut c = base();
        c.declared_leak = true;
        assert_eq!(evaluate(&c).decision, IngestionDecision::Deny);
    }

    #[test]
    fn denies_redistribution_prohibited() {
        let mut c = base();
        c.redistribution_prohibited = true;
        assert_eq!(evaluate(&c).decision, IngestionDecision::Deny);
    }
}
