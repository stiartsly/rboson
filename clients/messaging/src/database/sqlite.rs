// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::{fs, fmt, path::Path};
use diesel::{
    connection::SimpleConnection,
    sql_query,
    sql_types::{
        BigInt,
        Binary,
        Integer,
        Nullable,
        Text,
    },
    Connection as _,
    OptionalExtension,
    QueryableByName,
    RunQueryDsl,
    SqliteConnection,
};

use boson_api::{
    errors::StateError,
    Id,
    Result,
    Error,
};
use super::{
    dialect::*,
    MessagingStore,
};

pub(crate) struct SqliteStore {
    connection: Option<SqliteConnection>,
}

impl SqliteStore {
    pub(crate) fn new() -> Self {
        Self { connection: None }
    }

    fn conn(&mut self) -> Result<&mut SqliteConnection> {
        self.connection.as_mut().ok_or_else(||
            StateError::new("The sqlite database is not open").into()
        )
    }
}

fn db_err(error: impl fmt::Display) -> Error {
    StateError::new(error.to_string()).into()
}

fn now_ms() -> i64 {
    boson_api::now_ms() as i64
}

#[derive(QueryableByName)]
struct SchemaVersion {
    #[diesel(sql_type = Integer)]
    user_version: i32,
}

#[derive(QueryableByName)]
struct ContactsRevision {
    #[diesel(sql_type = Integer)]
    revision: i32,
}

#[derive(QueryableByName)]
struct MessageRid {
    #[diesel(sql_type = BigInt)]
    rid: i64,
}

#[derive(QueryableByName)]
struct ContactExists {
    #[diesel(sql_type = Integer)]
    exists: i32,
}

#[derive(QueryableByName, Clone)]
pub(crate) struct RecordMessage {
    #[diesel(sql_type = BigInt)]
    pub(crate) rid: i64,
    #[diesel(sql_type = Binary)]
    pub(crate) id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    pub(crate) conversation_id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    pub(crate) version: i32,
    #[diesel(sql_type = Binary)]
    pub(crate) recipient: Vec<u8>,
    #[diesel(sql_type = Integer)]
    #[diesel(column_name = "type")]
    pub(crate) message_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) from_id: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    pub(crate) created_at: i64,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) content_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) content_disposition: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) payload: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    pub(crate) sent_at: i64,
    #[diesel(sql_type = BigInt)]
    pub(crate) received_at: i64,
}

#[derive(QueryableByName, Clone)]
pub(crate) struct RecordContact {
    #[diesel(sql_type = Binary)]
    pub(crate) id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    #[diesel(column_name = "type")]
    pub(crate) contact_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) session_key: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) remark: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) tags: Option<String>,
    #[diesel(sql_type = Integer)]
    pub(crate) muted: i32,
    #[diesel(sql_type = Integer)]
    pub(crate) blocked: i32,
    #[diesel(sql_type = Integer)]
    pub(crate) revision: i32,
    #[diesel(sql_type = BigInt)]
    pub(crate) created_at: i64,
    #[diesel(sql_type = BigInt)]
    pub(crate) updated_at: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) owner: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Integer>)]
    pub(crate) permission: Option<i32>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) notice: Option<String>,
    #[diesel(sql_type = Nullable<Integer>)]
    pub(crate) announce: Option<i32>,
}

#[derive(QueryableByName, Clone)]
pub(crate) struct RecordConversation {
    #[diesel(sql_type = Binary)]
    pub(crate) id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    #[diesel(column_name = "type")]
    pub(crate) contact_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) session_key: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) remark: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) tags: Option<String>,
    #[diesel(sql_type = Integer)]
    pub(crate) muted: i32,
    #[diesel(sql_type = Integer)]
    pub(crate) blocked: i32,
    #[diesel(sql_type = Integer)]
    pub(crate) revision: i32,
    #[diesel(sql_type = BigInt)]
    pub(crate) created_at: i64,
    #[diesel(sql_type = BigInt)]
    pub(crate) updated_at: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) owner: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Integer>)]
    pub(crate) permission: Option<i32>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) notice: Option<String>,
    #[diesel(sql_type = Nullable<Integer>)]
    pub(crate) announce: Option<i32>,
    #[diesel(sql_type = BigInt)]
    pub(crate) last_message_rid: i64,
    #[diesel(sql_type = Binary)]
    pub(crate) last_message_id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    pub(crate) last_message_conversation_id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    pub(crate) last_message_version: i32,
    #[diesel(sql_type = Binary)]
    pub(crate) last_message_recipient: Vec<u8>,
    #[diesel(sql_type = Integer)]
    pub(crate) last_message_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) last_message_from_id: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    pub(crate) last_message_created_at: i64,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) last_message_content_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) last_message_content_disposition: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    pub(crate) last_message_payload: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    pub(crate) last_message_sent_at: i64,
    #[diesel(sql_type = BigInt)]
    pub(crate) last_message_received_at: i64,
}

#[derive(QueryableByName, Clone)]
pub(crate) struct RecordChannelMember {
    #[diesel(sql_type = Binary)]
    pub(crate) id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    pub(crate) channel_id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    pub(crate) role: i32,
    #[diesel(sql_type = BigInt)]
    pub(crate) joined: i64,
}

#[derive(QueryableByName, Clone)]
pub(crate) struct RecordFriendRequest {
    #[diesel(sql_type = Binary)]
    pub(crate) id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    pub(crate) initiator: Vec<u8>,
    #[diesel(sql_type = Nullable<Text>)]
    pub(crate) hello: Option<String>,
    #[diesel(sql_type = BigInt)]
    pub(crate) created_at: i64,
    #[diesel(sql_type = BigInt)]
    pub(crate) updated_at: i64,
    #[diesel(sql_type = Integer)]
    pub(crate) accepted: i32,
    #[diesel(sql_type = BigInt)]
    pub(crate) accepted_at: i64,
}

fn upsert_revision(conn: &mut SqliteConnection, revision: i32) -> diesel::result::QueryResult<()> {
    sql_query(UPSERT_CONTACTS_REVISION)
    .bind::<Integer, _>(revision)
    .bind::<BigInt, _>(now_ms())
    .execute(conn)?;
    Ok(())
}

fn put_contact_row(
    conn: &mut SqliteConnection,
    contact: RecordContact,
) -> diesel::result::QueryResult<()> {
    sql_query(UPSERT_CONTACT)
    .bind::<Binary, _>(&contact.id)
    .bind::<Integer, _>(contact.contact_type)
    .bind::<Nullable<Binary>, _>(&contact.session_key)
    .bind::<Nullable<Text>, _>(&contact.name)
    .bind::<Nullable<Text>, _>(&contact.remark)
    .bind::<Nullable<Text>, _>(&contact.tags)
    .bind::<Integer, _>(contact.muted)
    .bind::<Integer, _>(contact.blocked)
    .bind::<Integer, _>(contact.revision)
    .bind::<BigInt, _>(contact.created_at)
    .bind::<BigInt, _>(contact.updated_at)
    .execute(conn)?;

    if contact.contact_type == 2 {
        let owner = contact.owner.as_ref()
            .ok_or(diesel::result::Error::RollbackTransaction)?;
        let permission = contact.permission
            .ok_or(diesel::result::Error::RollbackTransaction)?;
        sql_query(UPSERT_CHANNEL)
        .bind::<Binary, _>(&contact.id)
        .bind::<Binary, _>(owner)
        .bind::<Integer, _>(permission)
        .bind::<Nullable<Text>, _>(&contact.notice)
        .bind::<Integer, _>(contact.announce.unwrap_or(0))
        .execute(conn)?;
    }

    Ok(())
}

impl MessagingStore for SqliteStore {
    fn open(&mut self, path: &Path) -> Result<()> {
        if self.connection.is_some() {
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(db_err)?;
            }
        }

        let database_url = path.to_string_lossy();
        let mut connection = SqliteConnection::establish(&database_url).map_err(db_err)?;
        connection.batch_execute(PRAGMAS).map_err(db_err)?;

        let version = sql_query(SELECT_USER_VERSION)
            .get_result::<SchemaVersion>(&mut connection)
            .map_err(db_err)?
            .user_version;
        if version > 1 {
            return Err(StateError::new(format!(
                "Unsupported messaging database schema version: {version}"
            ))
            .into());
        }
        if version == 0 {
            connection.batch_execute(CREATE_SCHEMA).map_err(db_err)?;
            connection.batch_execute(SET_USER_VERSION).map_err(db_err)?;
        }

        self.connection = Some(connection);
        Ok(())
    }

    fn close(&mut self) {
        if let Some(connection) = self.connection.take() {
            drop(connection);
        }
    }

    fn is_open(&self) -> bool {
        self.connection.is_some()
    }

    fn get_friend_request(&mut self, user_id: &Id) -> Result<Option<RecordFriendRequest>> {
        let row = sql_query(SELECT_FRIEND_REQUEST_BY_ID)
            .bind::<Binary, _>(user_id.as_bytes().to_vec())
            .get_result::<RecordFriendRequest>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        Ok(row)
    }

    fn get_friend_requests(&mut self) -> Result<Vec<RecordFriendRequest>> {
        let rows = sql_query(SELECT_FRIEND_REQUESTS)
            .load::<RecordFriendRequest>(self.conn()?)
            .map_err(db_err)?;
        Ok(rows)
    }

    fn remove_friend_request(&mut self, user_id: &Id) -> Result<bool> {
        let affected = sql_query(DELETE_FRIEND_REQUEST_BY_ID)
            .bind::<Binary, _>(user_id.as_bytes().to_vec())
            .execute(self.conn()?)
            .map_err(db_err)?;
        Ok(affected > 0)
    }

    fn remove_friend_requests(&mut self, user_ids: &[Id]) -> Result<bool> {
        if user_ids.is_empty() {
            return Ok(false);
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let mut removed = false;
            for id in user_ids {
                let affected = sql_query(DELETE_FRIEND_REQUEST_BY_ID)
                    .bind::<Binary, _>(id.as_bytes().to_vec())
                    .execute(conn)?;
                removed |= affected > 0;
            }
            Ok(removed)
        })
        .map_err(db_err)
    }

    fn clear_friend_requests(&mut self) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(DELETE_ALL_FRIEND_REQUESTS).execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn get_contacts_revision(&mut self) -> Result<i32> {
        let row = sql_query(SELECT_CONTACTS_REVISION)
            .get_result::<ContactsRevision>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        Ok(row.map(|row| row.revision).unwrap_or(0))
    }

    fn put_contact(&mut self, revision: i32, contact: RecordContact) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            put_contact_row(conn, contact)?;
            upsert_revision(conn, revision)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn put_contacts(&mut self, revision: i32, contacts: Vec<RecordContact>) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for contact in contacts {
                put_contact_row(conn, contact)?;
            }
            upsert_revision(conn, revision)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn put_contact_locally(&mut self, contact: RecordContact) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn|
            put_contact_row(conn, contact)
        )
        .map_err(db_err)
    }

    fn remove_contact(&mut self, revision: i32, contact_id: &Id) -> Result<bool> {
        self.remove_contacts(revision, &[*contact_id])
    }

    fn remove_contacts(&mut self, revision: i32, contact_ids: &[Id]) -> Result<bool> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for id in contact_ids {
                sql_query(DELETE_CONTACT_BY_ID)
                    .bind::<Binary, _>(id.as_bytes().to_vec())
                    .execute(conn)?;
                sql_query(DELETE_MESSAGES_BY_CONVERSATION)
                    .bind::<Binary, _>(id.as_bytes().to_vec())
                    .execute(conn)?;
            }
            upsert_revision(conn, revision)?;
            Ok(true)
        })
        .map_err(db_err)
    }

    fn remove_contact_locally(&mut self, contact_id: &Id) -> Result<bool> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let removed = sql_query(DELETE_CONTACT_BY_ID)
                .bind::<Binary, _>(contact_id.as_bytes().to_vec())
                .execute(conn)?
                > 0;
            sql_query(DELETE_MESSAGES_BY_CONVERSATION)
                .bind::<Binary, _>(contact_id.as_bytes().to_vec())
                .execute(conn)?;
            Ok(removed)
        })
        .map_err(db_err)
    }

    fn clear_contacts(&mut self, revision: i32) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(DELETE_ALL_CONTACTS).execute(conn)?;
            upsert_revision(conn, revision)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn get_contact(&mut self, contact_id: &Id) -> Result<Option<RecordContact>> {
        let row = sql_query(SELECT_CONTACT_BY_ID)
            .bind::<Binary, _>(contact_id.as_bytes().to_vec())
            .get_result::<RecordContact>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        Ok(row)
    }

    fn get_contacts(&mut self, contact_ids: &[Id]) -> Result<Vec<RecordContact>> {
        if contact_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut contacts = Vec::with_capacity(contact_ids.len());
        for id in contact_ids {
            if let Some(contact) = self.get_contact(id)? {
                contacts.push(contact);
            }
        }
        Ok(contacts)
    }

    fn get_all_contacts(&mut self) -> Result<Vec<RecordContact>> {
        let rows = sql_query(CONTACT_SELECT)
            .load::<RecordContact>(self.conn()?)
            .map_err(db_err)?;
        Ok(rows)
    }

    fn exists_contact(&mut self, contact_id: &Id) -> Result<bool> {
        let row = sql_query(EXISTS_CONTACT)
            .bind::<Binary, _>(contact_id.as_bytes().to_vec())
            .get_result::<ContactExists>(self.conn()?)
            .map_err(db_err)?;
        Ok(row.exists != 0)
    }

    fn put_friend_request(&mut self, request: RecordFriendRequest) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(UPSERT_FRIEND_REQUEST)
            .bind::<Binary, _>(&request.id)
            .bind::<Binary, _>(&request.initiator)
            .bind::<Nullable<Text>, _>(&request.hello)
            .bind::<BigInt, _>(request.created_at)
            .bind::<BigInt, _>(request.updated_at)
            .bind::<Integer, _>(request.accepted)
            .bind::<BigInt, _>(request.accepted_at)
            .execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn update_channel_ownership(
        &mut self,
        channel_id: &Id,
        old_owner_id: &Id,
        new_owner_id: &Id,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(UPDATE_CHANNEL_OWNER)
                .bind::<Binary, _>(new_owner_id.as_bytes().to_vec())
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .execute(conn)?;
            sql_query(UPDATE_CHANNEL_MEMBER_ROLE)
                .bind::<Integer, _>(2) // Set old owner's role to 2 (member)
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .bind::<Binary, _>(old_owner_id.as_bytes().to_vec())
                .execute(conn)?;
            sql_query(UPDATE_CHANNEL_MEMBER_ROLE)
                .bind::<Integer, _>(0) // Set new owner's role to 0 (owner)
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .bind::<Binary, _>(new_owner_id.as_bytes().to_vec())
                .execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn put_channel_members(
        &mut self,
        channel_id: &Id,
        members: Vec<RecordChannelMember>,
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for member in members {
                sql_query(UPSERT_CHANNEL_MEMBER)
                .bind::<Binary, _>(&member.id)
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .bind::<Integer, _>(member.role)
                .bind::<BigInt, _>(member.joined)
                .execute(conn)?;
            }
            Ok(())
        })
        .map_err(db_err)
    }

    fn refill_channel_members(
        &mut self,
        channel_id: &Id,
        members: &[RecordChannelMember],
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(DELETE_CHANNEL_MEMBERS_BY_CHANNEL)
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .execute(conn)?;
            for member in members {
                sql_query(UPSERT_CHANNEL_MEMBER)
                .bind::<Binary, _>(&member.id)
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .bind::<Integer, _>(member.role)
                .bind::<BigInt, _>(member.joined)
                .execute(conn)?;
            }
            Ok(())
        })
        .map_err(db_err)
    }

    fn get_channel_member(
        &mut self,
        channel_id: &Id,
        member_id: &Id,
    ) -> Result<Option<RecordChannelMember>> {
        let row = sql_query(SELECT_CHANNEL_MEMBER)
            .bind::<Binary, _>(channel_id.as_bytes().to_vec())
            .bind::<Binary, _>(member_id.as_bytes().to_vec())
            .get_result::<RecordChannelMember>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        Ok(row)
    }

    fn get_channel_members(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
    ) -> Result<Vec<RecordChannelMember>> {
        if member_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut members = Vec::with_capacity(member_ids.len());
        for id in member_ids {
            if let Some(member) = self.get_channel_member(channel_id, id)? {
                members.push(member);
            }
        }
        members.sort_by_key(|member| member.joined);
        Ok(members)
    }

    fn get_all_channel_members(&mut self, channel_id: &Id) -> Result<Vec<RecordChannelMember>> {
        let rows = sql_query(SELECT_CHANNEL_MEMBERS)
            .bind::<Binary, _>(channel_id.as_bytes().to_vec())
            .load::<RecordChannelMember>(self.conn()?)
            .map_err(db_err)?;
        Ok(rows)
    }

    fn update_channel_members_role(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
        role: i32,
    ) -> Result<bool> {
        if member_ids.is_empty() {
            return Ok(true);
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let mut updated = false;
            for id in member_ids {
                let affected = sql_query(UPDATE_CHANNEL_MEMBER_ROLE)
                .bind::<Integer, _>(role)
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .bind::<Binary, _>(id.as_bytes().to_vec())
                .execute(conn)?;
                updated |= affected > 0;
            }
            Ok(updated)
        })
        .map_err(db_err)
    }

    fn remove_channel_members(&mut self, channel_id: &Id, member_ids: &[Id]) -> Result<bool> {
        if member_ids.is_empty() {
            return Ok(true);
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let mut removed = false;
            for id in member_ids {
                let affected = sql_query(DELETE_CHANNEL_MEMBER)
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .bind::<Binary, _>(id.as_bytes().to_vec())
                .execute(conn)?;
                removed |= affected > 0;
            }
            Ok(removed)
        })
        .map_err(db_err)
    }

    fn get_conversation(&mut self, conversation_id: &Id) -> Result<Option<RecordConversation>> {
        let row = sql_query(SELECT_CONVERSATION_BY_ID)
            .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
            .get_result::<RecordConversation>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        Ok(row)
    }

    fn get_all_conversations(&mut self) -> Result<Vec<RecordConversation>> {
        let rows = sql_query(SELECT_CONVERSATIONS)
            .load::<RecordConversation>(self.conn()?)
            .map_err(db_err)?;
        Ok(rows)
    }

    fn remove_conversations(&mut self, conversation_ids: &[Id]) -> Result<bool> {
        if conversation_ids.is_empty() {
            return Ok(true);
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let mut removed = false;
            for id in conversation_ids {
                let affected = sql_query(DELETE_MESSAGES_BY_CONVERSATION)
                    .bind::<Binary, _>(id.as_bytes().to_vec())
                    .execute(conn)?;
                removed |= affected > 0;
            }
            Ok(removed)
        })
        .map_err(db_err)
    }

    fn get_messages_in_range(
        &mut self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<RecordMessage>> {
        let rows = sql_query(SELECT_MESSAGES_IN_RANGE)
        .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
        .bind::<BigInt, _>(begin)
        .bind::<BigInt, _>(end)
        .load::<RecordMessage>(self.conn()?)
        .map_err(db_err)?;
        Ok(rows)
    }

    fn get_messages_before(
        &mut self,
        conversation_id: &Id,
        until: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<RecordMessage>> {
        let rows = sql_query(SELECT_MESSAGES_BEFORE)
        .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
        .bind::<BigInt, _>(until)
        .bind::<BigInt, _>(limit as i64)
        .bind::<BigInt, _>(offset as i64)
        .load::<RecordMessage>(self.conn()?)
        .map_err(db_err)?;
        Ok(rows)
    }

    fn put_message(&mut self, message: RecordMessage) -> Result<i64> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let row = sql_query(INSERT_MESSAGE)
            .bind::<Binary, _>(&message.id)
            .bind::<Binary, _>(&message.conversation_id)
            .bind::<Integer, _>(message.version)
            .bind::<Binary, _>(&message.recipient)
            .bind::<Integer, _>(message.message_type)
            .bind::<Nullable<Binary>, _>(&message.from_id)
            .bind::<BigInt, _>(message.created_at)
            .bind::<BigInt, _>(message.sent_at)
            .bind::<BigInt, _>(message.received_at)
            .bind::<Nullable<Text>, _>(&message.content_type)
            .bind::<Nullable<Text>, _>(&message.content_disposition)
            .bind::<Nullable<Binary>, _>(&message.payload)
            .get_result::<MessageRid>(conn)?;
            Ok(row.rid)
        })
        .map_err(db_err)
    }

    fn update_message_sent_time(&mut self, message_id: &Id, sent_at: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(UPDATE_MESSAGE_SENT_AT)
                .bind::<BigInt, _>(sent_at)
                .bind::<Binary, _>(message_id.as_bytes().to_vec())
                .execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn remove_messages(&mut self, rids: &[i64]) -> Result<bool> {
        if rids.is_empty() {
            return Ok(true);
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let mut removed = false;
            for rid in rids {
                let affected = sql_query(DELETE_MESSAGE_BY_RID)
                    .bind::<BigInt, _>(*rid)
                    .execute(conn)?;
                removed |= affected > 0;
            }
            Ok(removed)
        })
        .map_err(db_err)
    }

    fn remove_messages_by_conversation(&mut self, conversation_id: &Id) -> Result<bool> {
        let affected = sql_query(DELETE_MESSAGES_BY_CONVERSATION)
            .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
            .execute(self.conn()?)
            .map_err(db_err)?;
        Ok(affected > 0)
    }

    fn clear_messages(&mut self) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(DELETE_ALL_MESSAGES).execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }
}
