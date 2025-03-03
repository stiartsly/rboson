use log::error;

use crate::{
    Identity,
    signature::KeyPair,
};

pub struct ClientBuilder {

    user:   Option<Identity>,
    device: Option<Identity>
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            user: None,
            device: None,
        }
    }

    pub fn with_user_key(&mut self, keypair: KeyPair) -> &mut Self {
        unimplemented!()
    }

    pub fn with_device_key(&mut self, keypair: KeyPair) -> &mut Self {
        unimplemented!()
    }



    async fn eligible_check(&self) -> Result<()> {
        unimplemented!()
    }

    pub async fn build(&self) -> Result<MessagingClient> {
        let Err(e) = self.eligible_check() {
            error!("{e}");
            return Err(e)
        }

        unimplemented!()
    }
}