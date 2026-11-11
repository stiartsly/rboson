use serde::{
    Deserialize,
    Serialize,
};
use crate::Permission;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct ChannelInfoUpdate {
    #[serde(rename = "p", skip_serializing_if = "Option::is_none")]
    permission: Option<i32>,
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    name: Option<Option<String>>,
    #[serde(rename = "nt", skip_serializing_if = "Option::is_none")]
    notice: Option<Option<String>>,
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")]
    announced: Option<bool>,
}

impl ChannelInfoUpdate {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn permission(mut self, permission: Permission) -> Self {
        self.permission = Some(permission as i32);
        self
    }

    pub(crate) fn name(mut self, name: Option<String>) -> Self {
        self.name = Some(name);
        self
    }

    pub(crate) fn notice(mut self, notice: Option<String>) -> Self {
        self.notice = Some(notice);
        self
    }

    pub(crate) fn announced(mut self, announced: bool) -> Self {
        self.announced = Some(announced);
        self
    }

    pub(crate) fn get_permission(&self) -> Option<Permission> {
        self.permission.map(Permission::from_i32)
    }

    pub(crate) fn get_name(&self) -> Option<Option<&str>> {
        self.name.as_ref().map(|name| name.as_deref())
    }

    pub(crate) fn get_notice(&self) -> Option<Option<&str>> {
        self.notice.as_ref().map(|notice| notice.as_deref())
    }

    pub(crate) fn get_announced(&self) -> Option<bool> {
        self.announced
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.permission.is_none()
            && self.name.is_none()
            && self.notice.is_none()
            && self.announced.is_none()
    }
}
