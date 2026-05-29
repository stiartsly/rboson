use crate::messaging::contact::Contact;
use crate::Id;

/// Receives events about changes to the local contact list.
pub trait ContactListener: Send + Sync {
    /// Called when a new contact has been added.
    fn on_contact_added(&self, contact: &dyn Contact) {}

    /// Called when one or more existing contacts were updated.
    fn on_contacts_updated(&self, contacts: &[Box<dyn Contact>]) {}

    /// Called when one or more contacts were removed.
    fn on_contacts_removed(&self, contact_ids: &[Id]) {}

    /// Called when every contact was cleared from the local list.
    fn on_contacts_cleared(&self) {}
}
