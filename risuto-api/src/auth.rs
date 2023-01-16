use std::ops::BitOr;

use uuid::Uuid;

use crate::STUB_UUID;

#[derive(Clone, Debug, bolero::generator::TypeGenerator, serde::Deserialize, serde::Serialize)]
pub struct NewSession {
    pub user: String,
    pub password: String,
    pub device: String,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
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
        }
    }
}
