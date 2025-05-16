
use crate::Id;
use super::contact::Contact;

pub trait ContactListener {
    fn on_contacts_updating(&self, _version_id: &str, _contacts: Vec<Contact>) {}
    fn on_contacts_updated(&self, _base_version_id: &str, _new_version_id: &str, _contacts: Vec<Contact>) {}
    fn on_contacts_cleared(&self) {}
    fn on_contact_profile(&self, _contact_id: &Id, _profile: &Contact) {}
}
