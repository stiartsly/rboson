
use crate::{
    Id,
    messaging::Contact,
};

pub trait ContactListener {
    fn on_contacts_updating(&self,
        version_id: &str,
        contacts: Vec<Contact>
    );

    fn on_contacts_updated(&self,
        base_version_id: &str,
        new_version_id: &str,
        contacts: Vec<Contact>
    );

    fn on_contacts_cleared(&self);

    fn on_contact_profile(&self,
        contact_id: &Id,
        profile: &Contact
    );
}
