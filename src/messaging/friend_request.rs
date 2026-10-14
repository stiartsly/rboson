use std::time::SystemTime;
use crate::Id;

/// A pending or resolved friend request.
///
#[derive(Debug, Clone)]
pub struct FriendRequest {
    user_id: Id,
    initiator_id: Id,
    hello: Option<String>,
    accepted: bool,

    created_at: SystemTime,
    accepted_at: Option<SystemTime>,
    updated_at: SystemTime
}

impl FriendRequest {
    const EXPIRATION_SECS: u128 = 60 * 60 * 24 * 7; // 1 week in seconds

    pub(crate) fn new(
        user_id: Id,
        initiator_id: Id,
        hello: Option<String>,
        created_at: SystemTime
    ) -> Self {
        Self {
            user_id,
            initiator_id,
            hello,
            accepted: false,
            created_at,
            accepted_at: None,
            updated_at: created_at,
        }
    }

    pub fn user_id(&self) -> &Id {
        &self.user_id
    }

    pub fn initiator_id(&self) -> &Id {
        &self.initiator_id
    }

    pub fn hello(&self) -> Option<&str> {
        self.hello.as_deref()
    }

    pub fn is_accepted(&self) -> bool {
        self.accepted
    }

    #[allow(unused_mut)]
    pub(crate) fn accept(&mut self) {
        self.accept_at(SystemTime::now());
    }

    pub(crate) fn accept_at(&mut self, accepted_at: SystemTime) {
        self.accepted = true;
        self.accepted_at = Some(accepted_at);
        self.updated_at = SystemTime::now();
    }

    pub fn is_expired(&self) -> bool {
        self.is_expired_at(SystemTime::now())
    }

	/**
	 * Checks whether the request had expired at the given time, e.g. when the other side acted on it.
	 * <p>
	 * The week counts from the creation time, the moment the request was sent, which both sides agree
	 * on; the update time is local (when this device received or changed the record) and would let the
	 * two sides expire at different times. A new request replaces the record with a new creation time,
	 * so resending starts a new week.
	 * </p>
	 *
	 * @param time the time to check, in milliseconds since the epoch.
	 * @return {@code true} if the request was still pending and past its expiry at {@code time}.
	 */
	pub(crate) fn is_expired_at(&self, time: SystemTime) -> bool {
        !self.accepted &&
            crate::elapsed_ms!(time) >= Self::EXPIRATION_SECS * 1000
    }

    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    pub fn accepted_at(&self) -> Option<SystemTime> {
        self.accepted_at
    }

    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }
}
