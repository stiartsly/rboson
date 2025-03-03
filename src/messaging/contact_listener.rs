
use crate::Id;
use super::contact::Contact;

pub trait ContactListener {
    fn on_update_contacts(&self, contacts: &[&Contact]);
    fn on_remove_contacts(&self, contacts: &[&Id]);
}
