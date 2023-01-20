use std::{ops::BitOr, str::FromStr};

use uuid::Uuid;

use crate::{Error, STUB_UUID};

pub const BCRYPT_POW_COST: u32 = 10;

#[derive(Clone, Debug, bolero::generator::TypeGenerator, serde::Deserialize, serde::Serialize)]
pub struct NewSession {
    pub user: String,
    pub password: String,
    pub device: String,

    /// Proof of work, to avoid the user spamming password attempts
    pub pow: String,
}

impl NewSession {
    pub fn new(user: String, password: String, device: String) -> NewSession {
        NewSession {
            pow: bcrypt::hash_with_salt(&password, BCRYPT_POW_COST, [0; 16])
                .expect("failed hashing password")
                .to_string(),
            user,
            password,
            device,
        }
    }

    pub fn validate_except_pow(&self) -> Result<(), Error> {
        crate::validate_string(&self.user)?;
        crate::validate_string(&self.password)?;
        crate::validate_string(&self.device)?;
        crate::validate_string(&self.pow)?;
        Ok(())
    }

    pub fn verify_pow(&self) -> bool {
        let parts = match bcrypt::HashParts::from_str(&self.pow) {
            Ok(parts) => parts,
            Err(_) => return false,
        };
        if parts.get_cost() != BCRYPT_POW_COST || parts.get_salt() != "......................" {
            // this string matches the all-0 salt
            return false;
        }
        bcrypt::verify(&self.password, &self.pow).unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AuthToken(pub Uuid);

impl AuthToken {
    pub fn stub() -> AuthToken {
        AuthToken(STUB_UUID)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct AuthInfo {
    pub can_read: bool,
    pub can_edit: bool,
    pub can_triage: bool,
    pub can_relabel_to_any: bool, // TODO: rename into can_admin?
    pub can_comment: bool,
    pub can_archive: bool,
}

impl AuthInfo {
    pub fn owner() -> AuthInfo {
        Self::all_or_nothing(true)
    }

    pub fn all() -> AuthInfo {
        Self::all_or_nothing(true)
    }

    pub fn none() -> AuthInfo {
        Self::all_or_nothing(false)
    }

    pub fn all_or_nothing(all: bool) -> AuthInfo {
        AuthInfo {
            can_read: all,
            can_edit: all,
            can_triage: all,
            can_relabel_to_any: all,
            can_comment: all,
            can_archive: all,
        }
    }
}

impl BitOr for AuthInfo {
    type Output = Self;

    fn bitor(self, rhs: AuthInfo) -> AuthInfo {
        // TODO: use some bitset crate?
        AuthInfo {
            can_read: self.can_read || rhs.can_read,
            can_edit: self.can_edit || rhs.can_edit,
            can_triage: self.can_triage || rhs.can_triage,
            can_relabel_to_any: self.can_relabel_to_any || rhs.can_relabel_to_any,
            can_comment: self.can_comment || rhs.can_comment,
            can_archive: self.can_archive || rhs.can_archive,
        }
    }
}
