use boson::Id;

pub(crate) mod announce_peer;
pub(crate) mod find_node;
pub(crate) mod find_peer;
pub(crate) mod find_value;
pub(crate) mod identity;
pub(crate) mod log;
pub(crate) mod login;
pub(crate) mod me;
pub(crate) mod status;
pub(crate) mod store_value;

pub(crate) fn parse_id(text: &str) -> Option<Id> {
    match Id::try_from(text) {
        Ok(id) => Some(id),
        Err(e) => {
            println!("\x1b[31mInvalid id '{text}': {e}\x1b[0m");
            None
        }
    }
}
