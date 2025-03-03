use crate::Id;
use super::contact::Contact;

pub trait ContactManager {
    fn contact(&self, contact_id: &Id) -> Option<&Contact>;
    fn get_contacts(&self) -> [&Contact];

    fn exists(&self, contact_id: &Id) -> bool {
        self.contact(contact_id).is_some()
    }

    fn put_contact(&self, contact: Contact);
    fn put_contacts(&self, contacts: &[Contact]);

    fn remove_contact(&self, contact_id: &Id);
    fn remove_contacts(&self, contact_ids: [&Id]);
}
