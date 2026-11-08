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

use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

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
};

use super::{
    MessagingStore,
    StoredChannel,
    StoredChannelMember,
    StoredContact,
    StoredConversation,
    StoredFriendRequest,
    StoredMessage,
};

const PRAGMAS: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA foreign_keys = ON;
    PRAGMA busy_timeout = 5000;
    PRAGMA locking_mode = NORMAL;
    PRAGMA synchronous = FULL;
";

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS contacts_revision (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        revision INTEGER NOT NULL,
        updated_at BIGINT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS contacts (
        id BLOB NOT NULL PRIMARY KEY,
        type SMALLINT NOT NULL,
        session_key BLOB,
        name VARCHAR(256),
        remark VARCHAR(256),
        tags VARCHAR(256),
        muted BOOLEAN NOT NULL DEFAULT FALSE,
        blocked BOOLEAN NOT NULL DEFAULT FALSE,
        revision INTEGER NOT NULL,
        created_at BIGINT NOT NULL,
        updated_at BIGINT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_contacts_name ON contacts (name);
    CREATE INDEX IF NOT EXISTS idx_contacts_remark ON contacts (remark);

    CREATE TABLE IF NOT EXISTS channels (
        id BLOB NOT NULL PRIMARY KEY,
        owner BLOB NOT NULL,
        permission SMALLINT NOT NULL CHECK(permission IN(0, 1, 2, 3)),
        notice VARCHAR(1024),
        announce BOOLEAN DEFAULT FALSE,
        FOREIGN KEY (id) REFERENCES contacts (id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS channel_members (
       id BLOB NOT NULL,
       channel_id BLOB NOT NULL,
       role SMALLINT NOT NULL CHECK(role IN(-1, 0, 1, 2)),
       joined BIGINT NOT NULL DEFAULT 0,
       PRIMARY KEY (id, channel_id),
       FOREIGN KEY (channel_id) REFERENCES channels (id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_channel_members_channel_id_joined
        ON channel_members (channel_id, joined ASC);

    CREATE TABLE IF NOT EXISTS friend_requests (
        id BLOB NOT NULL PRIMARY KEY,
        initiator BLOB NOT NULL,
        hello VARCHAR(512),
        created_at BIGINT NOT NULL,
        updated_at BIGINT NOT NULL,
        accepted BOOLEAN NOT NULL DEFAULT FALSE,
        accepted_at BIGINT DEFAULT 0
    );

    CREATE INDEX IF NOT EXISTS idx_friend_requests_created_at ON friend_requests (created_at);

    CREATE TABLE IF NOT EXISTS messages (
        rid INTEGER PRIMARY KEY AUTOINCREMENT,
        id BLOB NOT NULL UNIQUE,
        conversation_id BLOB NOT NULL,
        version SMALLINT NOT NULL,
        recipient BLOB NOT NULL,
        type SMALLINT NOT NULL,
        from_id BLOB,
        created_at BIGINT NOT NULL,
        content_type VARCHAR(128) DEFAULT NULL,
        content_disposition VARCHAR(512) DEFAULT NULL,
        payload BLOB DEFAULT NULL,
        sent_at BIGINT DEFAULT 0,
        received_at BIGINT DEFAULT 0
    );

    CREATE INDEX IF NOT EXISTS idx_messages_conversation_id ON messages (conversation_id);
";

const CONTACT_SELECT: &str = "
    SELECT c.*, ch.owner, ch.permission, ch.notice, ch.announce
    FROM contacts c
    LEFT JOIN channels ch ON c.id = ch.id
";

const CONVERSATION_SELECT: &str = "
    SELECT c.*, ch.owner, ch.permission, ch.notice, ch.announce,
        m.rid as last_message_rid, m.id as last_message_id,
        m.conversation_id as last_message_conversation_id, m.version as last_message_version,
        m.recipient as last_message_recipient, m.type as last_message_type, m.from_id as last_message_from_id,
        m.created_at as last_message_created_at, m.content_type as last_message_content_type,
        m.content_disposition as last_message_content_disposition, m.payload as last_message_payload,
        m.sent_at as last_message_sent_at, m.received_at as last_message_received_at
    FROM contacts c
    LEFT JOIN channels ch ON c.id = ch.id
";

pub(crate) struct SqliteMessagingStore {
    connection: Option<SqliteConnection>,
}

impl SqliteMessagingStore {
    pub(crate) fn new() -> Self {
        Self { connection: None }
    }

    fn conn(&mut self) -> Result<&mut SqliteConnection> {
        self.connection
            .as_mut()
            .ok_or_else(|| StateError::new("Messaging database is not open").into())
    }
}

fn db_err(error: impl std::fmt::Display) -> boson_api::Error {
    StateError::new(error.to_string()).into()
}

fn id_from(bytes: &[u8]) -> Result<Id> {
    Id::try_from(bytes)
        .map_err(|error| StateError::new(format!("Invalid database id: {error}")).into())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}


#[derive(QueryableByName)]
struct VersionRow {
    #[diesel(sql_type = Integer)]
    user_version: i32,
}

#[derive(QueryableByName)]
struct RevisionRow {
    #[diesel(sql_type = Integer)]
    revision: i32,
}

#[derive(QueryableByName)]
struct RidRow {
    #[diesel(sql_type = BigInt)]
    rid: i64,
}

#[derive(QueryableByName)]
struct ExistsRow {
    #[diesel(sql_type = Integer)]
    exists: i32,
}

#[derive(QueryableByName)]
struct MessageRow {
    #[diesel(sql_type = BigInt)]
    rid: i64,
    #[diesel(sql_type = Binary)]
    id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    conversation_id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    version: i32,
    #[diesel(sql_type = Binary)]
    recipient: Vec<u8>,
    #[diesel(sql_type = Integer)]
    #[diesel(column_name = "type")]
    message_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    from_id: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    created_at: i64,
    #[diesel(sql_type = Nullable<Text>)]
    content_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    content_disposition: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    payload: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    sent_at: i64,
    #[diesel(sql_type = BigInt)]
    received_at: i64,
}

#[derive(QueryableByName)]
struct ContactRow {
    #[diesel(sql_type = Binary)]
    id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    #[diesel(column_name = "type")]
    contact_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    session_key: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    remark: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    tags: Option<String>,
    #[diesel(sql_type = Integer)]
    muted: i32,
    #[diesel(sql_type = Integer)]
    blocked: i32,
    #[diesel(sql_type = Integer)]
    revision: i32,
    #[diesel(sql_type = BigInt)]
    created_at: i64,
    #[diesel(sql_type = BigInt)]
    updated_at: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    owner: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Integer>)]
    permission: Option<i32>,
    #[diesel(sql_type = Nullable<Text>)]
    notice: Option<String>,
    #[diesel(sql_type = Nullable<Integer>)]
    announce: Option<i32>,
}

#[derive(QueryableByName)]
struct ConversationRow {
    #[diesel(sql_type = Binary)]
    id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    #[diesel(column_name = "type")]
    contact_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    session_key: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    remark: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    tags: Option<String>,
    #[diesel(sql_type = Integer)]
    muted: i32,
    #[diesel(sql_type = Integer)]
    blocked: i32,
    #[diesel(sql_type = Integer)]
    revision: i32,
    #[diesel(sql_type = BigInt)]
    created_at: i64,
    #[diesel(sql_type = BigInt)]
    updated_at: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    owner: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Integer>)]
    permission: Option<i32>,
    #[diesel(sql_type = Nullable<Text>)]
    notice: Option<String>,
    #[diesel(sql_type = Nullable<Integer>)]
    announce: Option<i32>,
    #[diesel(sql_type = BigInt)]
    last_message_rid: i64,
    #[diesel(sql_type = Binary)]
    last_message_id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    last_message_conversation_id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    last_message_version: i32,
    #[diesel(sql_type = Binary)]
    last_message_recipient: Vec<u8>,
    #[diesel(sql_type = Integer)]
    last_message_type: i32,
    #[diesel(sql_type = Nullable<Binary>)]
    last_message_from_id: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    last_message_created_at: i64,
    #[diesel(sql_type = Nullable<Text>)]
    last_message_content_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    last_message_content_disposition: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    last_message_payload: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    last_message_sent_at: i64,
    #[diesel(sql_type = BigInt)]
    last_message_received_at: i64,
}

#[derive(QueryableByName)]
struct ChannelMemberRow {
    #[diesel(sql_type = Binary)]
    id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    channel_id: Vec<u8>,
    #[diesel(sql_type = Integer)]
    role: i32,
    #[diesel(sql_type = BigInt)]
    joined: i64,
}

#[derive(QueryableByName)]
struct FriendRequestRow {
    #[diesel(sql_type = Binary)]
    id: Vec<u8>,
    #[diesel(sql_type = Binary)]
    initiator: Vec<u8>,
    #[diesel(sql_type = Nullable<Text>)]
    hello: Option<String>,
    #[diesel(sql_type = BigInt)]
    created_at: i64,
    #[diesel(sql_type = BigInt)]
    updated_at: i64,
    #[diesel(sql_type = Integer)]
    accepted: i32,
    #[diesel(sql_type = BigInt)]
    accepted_at: i64,
}

fn message_from_row(row: MessageRow) -> Result<StoredMessage> {
    Ok(StoredMessage {
        rid: row.rid,
        id: id_from(&row.id)?,
        conversation_id: id_from(&row.conversation_id)?,
        version: row.version,
        recipient: id_from(&row.recipient)?,
        message_type: row.message_type,
        from: row.from_id.as_deref().map(id_from).transpose()?,
        created_at: row.created_at,
        content_type: row.content_type,
        content_disposition: row.content_disposition,
        payload: row.payload,
        sent_at: row.sent_at,
        received_at: row.received_at,
    })
}

fn contact_from_row(row: ContactRow) -> Result<StoredContact> {
    let channel = if row.contact_type == 2 {
        Some(StoredChannel {
            id: id_from(&row.id)?,
            owner: id_from(
                row.owner
                    .as_deref()
                    .ok_or_else(|| StateError::new("Channel owner is missing"))?,
            )?,
            permission: row
                .permission
                .ok_or_else(|| StateError::new("Channel permission is missing"))?,
            notice: row.notice,
            announce: row.announce.unwrap_or(0) != 0,
        })
    } else {
        None
    };

    Ok(StoredContact {
        id: id_from(&row.id)?,
        contact_type: row.contact_type,
        session_key: row.session_key,
        name: row.name,
        remark: row.remark,
        tags: row.tags,
        muted: row.muted != 0,
        blocked: row.blocked != 0,
        revision: row.revision,
        created_at: row.created_at,
        updated_at: row.updated_at,
        channel,
    })
}

fn conversation_from_row(row: ConversationRow) -> Result<StoredConversation> {
    let contact = contact_from_row(ContactRow {
        id: row.id,
        contact_type: row.contact_type,
        session_key: row.session_key,
        name: row.name,
        remark: row.remark,
        tags: row.tags,
        muted: row.muted,
        blocked: row.blocked,
        revision: row.revision,
        created_at: row.created_at,
        updated_at: row.updated_at,
        owner: row.owner,
        permission: row.permission,
        notice: row.notice,
        announce: row.announce,
    })?;

    let last_message = StoredMessage {
        rid: row.last_message_rid,
        id: id_from(&row.last_message_id)?,
        conversation_id: id_from(&row.last_message_conversation_id)?,
        version: row.last_message_version,
        recipient: id_from(&row.last_message_recipient)?,
        message_type: row.last_message_type,
        from: row.last_message_from_id.as_deref().map(id_from).transpose()?,
        created_at: row.last_message_created_at,
        content_type: row.last_message_content_type,
        content_disposition: row.last_message_content_disposition,
        payload: row.last_message_payload,
        sent_at: row.last_message_sent_at,
        received_at: row.last_message_received_at,
    };

    Ok(StoredConversation {
        contact,
        last_message: Some(last_message),
    })
}

fn channel_member_from_row(row: ChannelMemberRow) -> Result<StoredChannelMember> {
    Ok(StoredChannelMember {
        id: id_from(&row.id)?,
        channel_id: id_from(&row.channel_id)?,
        role: row.role,
        joined: row.joined,
    })
}

fn friend_request_from_row(row: FriendRequestRow) -> Result<StoredFriendRequest> {
    Ok(StoredFriendRequest {
        id: id_from(&row.id)?,
        initiator: id_from(&row.initiator)?,
        hello: row.hello,
        created_at: row.created_at,
        updated_at: row.updated_at,
        accepted: row.accepted != 0,
        accepted_at: row.accepted_at,
    })
}

fn upsert_revision(conn: &mut SqliteConnection, revision: i32) -> diesel::result::QueryResult<()> {
    sql_query(
        "INSERT INTO contacts_revision (id, revision, updated_at) \
         VALUES (1, ?, ?) \
         ON CONFLICT (id) DO UPDATE SET \
             revision = EXCLUDED.revision, \
             updated_at = EXCLUDED.updated_at",
    )
    .bind::<Integer, _>(revision)
    .bind::<BigInt, _>(now_ms())
    .execute(conn)?;
    Ok(())
}

fn put_contact_row(
    conn: &mut SqliteConnection,
    contact: &StoredContact,
) -> diesel::result::QueryResult<()> {
    sql_query(
        "INSERT INTO contacts \
         (id, type, session_key, name, remark, tags, muted, blocked, revision, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (id) DO UPDATE SET \
             type = EXCLUDED.type, \
             session_key = EXCLUDED.session_key, \
             name = EXCLUDED.name, \
             remark = EXCLUDED.remark, \
             tags = EXCLUDED.tags, \
             muted = EXCLUDED.muted, \
             blocked = EXCLUDED.blocked, \
             revision = EXCLUDED.revision, \
             updated_at = EXCLUDED.updated_at",
    )
    .bind::<Binary, _>(contact.id.as_bytes().to_vec())
    .bind::<Integer, _>(contact.contact_type)
    .bind::<Nullable<Binary>, _>(contact.session_key.clone())
    .bind::<Nullable<Text>, _>(contact.name.clone())
    .bind::<Nullable<Text>, _>(contact.remark.clone())
    .bind::<Nullable<Text>, _>(contact.tags.clone())
    .bind::<Integer, _>(if contact.muted { 1 } else { 0 })
    .bind::<Integer, _>(if contact.blocked { 1 } else { 0 })
    .bind::<Integer, _>(contact.revision)
    .bind::<BigInt, _>(contact.created_at)
    .bind::<BigInt, _>(contact.updated_at)
    .execute(conn)?;

    if contact.contact_type == 2 {
        let channel = contact
            .channel
            .as_ref()
            .ok_or(diesel::result::Error::RollbackTransaction)?;
        sql_query(
            "INSERT INTO channels (id, owner, permission, notice, announce) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (id) DO UPDATE SET \
                 owner = EXCLUDED.owner, \
                 permission = EXCLUDED.permission, \
                 notice = EXCLUDED.notice, \
                 announce = EXCLUDED.announce",
        )
        .bind::<Binary, _>(channel.id.as_bytes().to_vec())
        .bind::<Binary, _>(channel.owner.as_bytes().to_vec())
        .bind::<Integer, _>(channel.permission)
        .bind::<Nullable<Text>, _>(channel.notice.clone())
        .bind::<Integer, _>(if channel.announce { 1 } else { 0 })
        .execute(conn)?;
    }

    Ok(())
}

impl MessagingStore for SqliteMessagingStore {
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

        let version = sql_query("PRAGMA user_version")
            .get_result::<VersionRow>(&mut connection)
            .map_err(db_err)?
            .user_version;
        if version > 1 {
            return Err(StateError::new(format!(
                "Unsupported messaging database schema version: {version}"
            ))
            .into());
        }
        if version == 0 {
            connection.batch_execute(SCHEMA).map_err(db_err)?;
            connection
                .batch_execute("PRAGMA user_version = 1")
                .map_err(db_err)?;
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

    fn get_contacts_revision(&mut self) -> Result<i32> {
        let row = sql_query("SELECT revision FROM contacts_revision WHERE id = 1")
            .get_result::<RevisionRow>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        Ok(row.map(|row| row.revision).unwrap_or(0))
    }

    fn put_contact(&mut self, revision: i32, contact: &StoredContact) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            put_contact_row(conn, contact)?;
            upsert_revision(conn, revision)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn put_contacts(&mut self, revision: i32, contacts: &[StoredContact]) -> Result<()> {
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

    fn put_contact_locally(&mut self, contact: &StoredContact) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| put_contact_row(conn, contact))
            .map_err(db_err)
    }

    fn remove_contact_locally(&mut self, contact_id: &Id) -> Result<bool> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let removed = sql_query("DELETE FROM contacts WHERE id = ?")
                .bind::<Binary, _>(contact_id.as_bytes().to_vec())
                .execute(conn)?
                > 0;
            sql_query("DELETE FROM messages WHERE conversation_id = ?")
                .bind::<Binary, _>(contact_id.as_bytes().to_vec())
                .execute(conn)?;
            Ok(removed)
        })
        .map_err(db_err)
    }

    fn remove_contact(&mut self, revision: i32, contact_id: &Id) -> Result<bool> {
        self.remove_contacts(revision, &[*contact_id])
    }

    fn remove_contacts(&mut self, revision: i32, contact_ids: &[Id]) -> Result<bool> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for id in contact_ids {
                sql_query("DELETE FROM contacts WHERE id = ?")
                    .bind::<Binary, _>(id.as_bytes().to_vec())
                    .execute(conn)?;
                sql_query("DELETE FROM messages WHERE conversation_id = ?")
                    .bind::<Binary, _>(id.as_bytes().to_vec())
                    .execute(conn)?;
            }
            upsert_revision(conn, revision)?;
            Ok(true)
        })
        .map_err(db_err)
    }

    fn clear_contacts(&mut self, revision: i32) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query("DELETE FROM contacts").execute(conn)?;
            upsert_revision(conn, revision)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn get_contact(&mut self, contact_id: &Id) -> Result<Option<StoredContact>> {
        let sql = format!("{CONTACT_SELECT} WHERE c.id = ?");
        let row = sql_query(sql)
            .bind::<Binary, _>(contact_id.as_bytes().to_vec())
            .get_result::<ContactRow>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        row.map(contact_from_row).transpose()
    }

    fn get_contacts(&mut self, contact_ids: &[Id]) -> Result<Vec<StoredContact>> {
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

    fn get_all_contacts(&mut self) -> Result<Vec<StoredContact>> {
        let rows = sql_query(CONTACT_SELECT)
            .load::<ContactRow>(self.conn()?)
            .map_err(db_err)?;
        rows.into_iter().map(contact_from_row).collect()
    }

    fn exists_contact(&mut self, contact_id: &Id) -> Result<bool> {
        let row = sql_query("SELECT EXISTS(SELECT 1 FROM contacts WHERE id = ?) AS exists")
            .bind::<Binary, _>(contact_id.as_bytes().to_vec())
            .get_result::<ExistsRow>(self.conn()?)
            .map_err(db_err)?;
        Ok(row.exists != 0)
    }

    fn put_message(&mut self, message: &StoredMessage) -> Result<i64> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let row = sql_query(
                "INSERT INTO messages \
                 (id, conversation_id, version, recipient, type, from_id, created_at, sent_at, received_at, content_type, content_disposition, payload) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
                 RETURNING rid",
            )
            .bind::<Binary, _>(message.id.as_bytes().to_vec())
            .bind::<Binary, _>(message.conversation_id.as_bytes().to_vec())
            .bind::<Integer, _>(message.version)
            .bind::<Binary, _>(message.recipient.as_bytes().to_vec())
            .bind::<Integer, _>(message.message_type)
            .bind::<Nullable<Binary>, _>(message.from.map(|id| id.as_bytes().to_vec()))
            .bind::<BigInt, _>(message.created_at)
            .bind::<BigInt, _>(message.sent_at)
            .bind::<BigInt, _>(message.received_at)
            .bind::<Nullable<Text>, _>(message.content_type.clone())
            .bind::<Nullable<Text>, _>(message.content_disposition.clone())
            .bind::<Nullable<Binary>, _>(message.payload.clone())
            .get_result::<RidRow>(conn)?;
            Ok(row.rid)
        })
        .map_err(db_err)
    }

    fn update_message_sent_time(&mut self, message_id: &Id, sent_at: i64) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query("UPDATE messages SET sent_at = ? WHERE id = ?")
                .bind::<BigInt, _>(sent_at)
                .bind::<Binary, _>(message_id.as_bytes().to_vec())
                .execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn get_messages_in_range(
        &mut self,
        conversation_id: &Id,
        begin: i64,
        end: i64,
    ) -> Result<Vec<StoredMessage>> {
        let rows = sql_query(
            "SELECT * FROM messages \
             WHERE conversation_id = ? AND created_at >= ? AND created_at < ? \
             ORDER BY rid ASC",
        )
        .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
        .bind::<BigInt, _>(begin)
        .bind::<BigInt, _>(end)
        .load::<MessageRow>(self.conn()?)
        .map_err(db_err)?;
        rows.into_iter().map(message_from_row).collect()
    }

    fn get_messages_before(
        &mut self,
        conversation_id: &Id,
        until: i64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<StoredMessage>> {
        let rows = sql_query(
            "SELECT * FROM messages \
             WHERE conversation_id = ? AND created_at <= ? \
             ORDER BY rid ASC \
             LIMIT ? OFFSET ?",
        )
        .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
        .bind::<BigInt, _>(until)
        .bind::<BigInt, _>(limit as i64)
        .bind::<BigInt, _>(offset as i64)
        .load::<MessageRow>(self.conn()?)
        .map_err(db_err)?;
        rows.into_iter().map(message_from_row).collect()
    }

    fn remove_messages(&mut self, rids: &[i64]) -> Result<bool> {
        if rids.is_empty() {
            return Ok(true);
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let mut removed = false;
            for rid in rids {
                let affected = sql_query("DELETE FROM messages WHERE rid = ?")
                    .bind::<BigInt, _>(*rid)
                    .execute(conn)?;
                removed |= affected > 0;
            }
            Ok(removed)
        })
        .map_err(db_err)
    }

    fn remove_messages_by_conversation(&mut self, conversation_id: &Id) -> Result<bool> {
        let affected = sql_query("DELETE FROM messages WHERE conversation_id = ?")
            .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
            .execute(self.conn()?)
            .map_err(db_err)?;
        Ok(affected > 0)
    }

    fn clear_messages(&mut self) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query("DELETE FROM messages").execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn put_friend_request(&mut self, request: &StoredFriendRequest) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query(
                "INSERT INTO friend_requests \
                 (id, initiator, hello, created_at, updated_at, accepted, accepted_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT (id) DO UPDATE SET \
                     initiator = EXCLUDED.initiator, \
                     hello = EXCLUDED.hello, \
                     created_at = EXCLUDED.created_at, \
                     updated_at = EXCLUDED.updated_at, \
                     accepted = EXCLUDED.accepted, \
                     accepted_at = EXCLUDED.accepted_at",
            )
            .bind::<Binary, _>(request.id.as_bytes().to_vec())
            .bind::<Binary, _>(request.initiator.as_bytes().to_vec())
            .bind::<Nullable<Text>, _>(request.hello.clone())
            .bind::<BigInt, _>(request.created_at)
            .bind::<BigInt, _>(request.updated_at)
            .bind::<Integer, _>(if request.accepted { 1 } else { 0 })
            .bind::<BigInt, _>(request.accepted_at)
            .execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn get_friend_request(&mut self, user_id: &Id) -> Result<Option<StoredFriendRequest>> {
        let row = sql_query("SELECT * FROM friend_requests WHERE id = ?")
            .bind::<Binary, _>(user_id.as_bytes().to_vec())
            .get_result::<FriendRequestRow>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        row.map(friend_request_from_row).transpose()
    }

    fn get_friend_requests(&mut self) -> Result<Vec<StoredFriendRequest>> {
        let rows = sql_query("SELECT * FROM friend_requests ORDER BY created_at DESC")
            .load::<FriendRequestRow>(self.conn()?)
            .map_err(db_err)?;
        rows.into_iter().map(friend_request_from_row).collect()
    }

    fn remove_friend_request(&mut self, user_id: &Id) -> Result<bool> {
        let affected = sql_query("DELETE FROM friend_requests WHERE id = ?")
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
                let affected = sql_query("DELETE FROM friend_requests WHERE id = ?")
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
            sql_query("DELETE FROM friend_requests").execute(conn)?;
            Ok(())
        })
        .map_err(db_err)
    }

    fn get_conversation(&mut self, conversation_id: &Id) -> Result<Option<StoredConversation>> {
        let sql = format!(
            "{CONVERSATION_SELECT} JOIN messages m ON c.id = m.conversation_id \
             WHERE c.id = ? ORDER BY m.rid DESC LIMIT 1"
        );
        let row = sql_query(sql)
            .bind::<Binary, _>(conversation_id.as_bytes().to_vec())
            .get_result::<ConversationRow>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        row.map(conversation_from_row).transpose()
    }

    fn get_all_conversations(&mut self) -> Result<Vec<StoredConversation>> {
        let sql = format!(
            "{CONVERSATION_SELECT} \
             JOIN (SELECT conversation_id, MAX(rid) as max_rid FROM messages GROUP BY conversation_id) \
             last_msg_rid ON c.id = last_msg_rid.conversation_id \
             JOIN messages m ON m.rid = last_msg_rid.max_rid \
             ORDER BY m.received_at DESC, m.rid DESC"
        );
        let rows = sql_query(sql)
            .load::<ConversationRow>(self.conn()?)
            .map_err(db_err)?;
        rows.into_iter().map(conversation_from_row).collect()
    }

    fn remove_conversations(&mut self, conversation_ids: &[Id]) -> Result<bool> {
        if conversation_ids.is_empty() {
            return Ok(true);
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let mut removed = false;
            for id in conversation_ids {
                let affected = sql_query("DELETE FROM messages WHERE conversation_id = ?")
                    .bind::<Binary, _>(id.as_bytes().to_vec())
                    .execute(conn)?;
                removed |= affected > 0;
            }
            Ok(removed)
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
            sql_query("UPDATE channels SET owner = ? WHERE id = ?")
                .bind::<Binary, _>(new_owner_id.as_bytes().to_vec())
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .execute(conn)?;
            sql_query(
                "UPDATE channel_members SET role = ? WHERE channel_id = ? AND id = ?",
            )
            .bind::<Integer, _>(2)
            .bind::<Binary, _>(channel_id.as_bytes().to_vec())
            .bind::<Binary, _>(old_owner_id.as_bytes().to_vec())
            .execute(conn)?;
            sql_query(
                "UPDATE channel_members SET role = ? WHERE channel_id = ? AND id = ?",
            )
            .bind::<Integer, _>(0)
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
        members: &[StoredChannelMember],
    ) -> Result<()> {
        if members.is_empty() {
            return Ok(());
        }
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            for member in members {
                sql_query(
                    "INSERT INTO channel_members (id, channel_id, role, joined) \
                     VALUES (?, ?, ?, ?) \
                     ON CONFLICT (id, channel_id) DO UPDATE SET role = EXCLUDED.role",
                )
                .bind::<Binary, _>(member.id.as_bytes().to_vec())
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
        members: &[StoredChannelMember],
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            sql_query("DELETE FROM channel_members WHERE channel_id = ?")
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .execute(conn)?;
            for member in members {
                sql_query(
                    "INSERT INTO channel_members (id, channel_id, role, joined) \
                     VALUES (?, ?, ?, ?) \
                     ON CONFLICT (id, channel_id) DO UPDATE SET role = EXCLUDED.role",
                )
                .bind::<Binary, _>(member.id.as_bytes().to_vec())
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
    ) -> Result<Option<StoredChannelMember>> {
        let row = sql_query("SELECT * FROM channel_members WHERE channel_id = ? AND id = ?")
            .bind::<Binary, _>(channel_id.as_bytes().to_vec())
            .bind::<Binary, _>(member_id.as_bytes().to_vec())
            .get_result::<ChannelMemberRow>(self.conn()?)
            .optional()
            .map_err(db_err)?;
        row.map(channel_member_from_row).transpose()
    }

    fn get_channel_members(
        &mut self,
        channel_id: &Id,
        member_ids: &[Id],
    ) -> Result<Vec<StoredChannelMember>> {
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

    fn get_all_channel_members(&mut self, channel_id: &Id) -> Result<Vec<StoredChannelMember>> {
        let rows = sql_query(
            "SELECT * FROM channel_members WHERE channel_id = ? ORDER BY joined ASC",
        )
        .bind::<Binary, _>(channel_id.as_bytes().to_vec())
        .load::<ChannelMemberRow>(self.conn()?)
        .map_err(db_err)?;
        rows.into_iter().map(channel_member_from_row).collect()
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
                let affected = sql_query(
                    "UPDATE channel_members SET role = ? WHERE channel_id = ? AND id = ?",
                )
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
                let affected = sql_query(
                    "DELETE FROM channel_members WHERE channel_id = ? AND id = ?",
                )
                .bind::<Binary, _>(channel_id.as_bytes().to_vec())
                .bind::<Binary, _>(id.as_bytes().to_vec())
                .execute(conn)?;
                removed |= affected > 0;
            }
            Ok(removed)
        })
        .map_err(db_err)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use boson_api::Id;

    fn id(value: u8) -> Id {
        let mut bytes = [0u8; 32];
        bytes[31] = value;
        Id::try_from(&bytes[..]).unwrap()
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(format!("{}-wal", path.display()));
        let _ = fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn sqlite_store_round_trips_messages_and_contacts() {
        let path = std::env::temp_dir().join(format!(
            "boson-messaging-store-{}-{}.db",
            std::process::id(),
            now_ms()
        ));
        cleanup(&path);

        let mut store = SqliteMessagingStore::new();
        store.open(&path).unwrap();
        assert_eq!(store.get_contacts_revision().unwrap(), 0);

        let contact_id = id(1);
        let contact = StoredContact {
            id: contact_id,
            contact_type: 1,
            session_key: None,
            name: Some("test".into()),
            remark: None,
            tags: None,
            muted: false,
            blocked: false,
            revision: 1,
            created_at: 1,
            updated_at: 1,
            channel: None,
        };
        store.put_contact(1, &contact).unwrap();
        assert_eq!(store.get_contacts_revision().unwrap(), 1);
        assert_eq!(store.get_contact(&contact_id).unwrap().unwrap().name.as_deref(), Some("test"));

        let message_id = id(2);
        let message = StoredMessage {
            rid: 0,
            id: message_id,
            conversation_id: contact_id,
            version: 2,
            recipient: id(3),
            message_type: 1,
            from: Some(id(4)),
            created_at: 10,
            content_type: Some("text/plain".into()),
            content_disposition: None,
            payload: Some(b"hello".to_vec()),
            sent_at: 10,
            received_at: 0,
        };
        let rid = store.put_message(&message).unwrap();
        assert!(rid > 0);

        let messages = store.get_messages_in_range(&contact_id, 0, 100).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload.as_deref(), Some(b"hello".as_slice()));

        let conversation = store.get_conversation(&contact_id).unwrap().unwrap();
        assert_eq!(conversation.last_message.unwrap().rid, rid);
        let conversations = store.get_all_conversations().unwrap();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].last_message.as_ref().unwrap().rid, rid);

        store.remove_messages(&[rid]).unwrap();
        assert!(store.get_conversation(&contact_id).unwrap().is_none());

        let channel_id = id(5);
        let channel = StoredContact {
            id: channel_id,
            contact_type: 2,
            session_key: None,
            name: Some("channel".into()),
            remark: None,
            tags: None,
            muted: false,
            blocked: false,
            revision: 2,
            created_at: 2,
            updated_at: 2,
            channel: Some(StoredChannel {
                id: channel_id,
                owner: id(6),
                permission: 3,
                notice: Some("notice".into()),
                announce: false,
            }),
        };
        store.put_contact(2, &channel).unwrap();
        let member = StoredChannelMember {
            id: id(7),
            channel_id,
            role: 2,
            joined: 20,
        };
        store.put_channel_members(&channel_id, &[member]).unwrap();
        assert_eq!(store.get_all_channel_members(&channel_id).unwrap().len(), 1);
        assert!(store
            .update_channel_members_role(&channel_id, &[id(7)], 1)
            .unwrap());
        assert_eq!(
            store.get_channel_member(&channel_id, &id(7)).unwrap().unwrap().role,
            1
        );
        assert!(store.remove_channel_members(&channel_id, &[id(7)]).unwrap());
        assert!(store.get_all_channel_members(&channel_id).unwrap().is_empty());

        let request = StoredFriendRequest {
            id: id(8),
            initiator: id(9),
            hello: Some("hi".into()),
            created_at: 30,
            updated_at: 30,
            accepted: false,
            accepted_at: 0,
        };
        store.put_friend_request(&request).unwrap();
        assert_eq!(store.get_friend_requests().unwrap().len(), 1);
        assert!(store.remove_friend_request(&id(8)).unwrap());

        assert!(store.remove_contact_locally(&channel_id).unwrap());
        assert!(store.get_contact(&channel_id).unwrap().is_none());

        store.close();
        cleanup(&path);
    }
}
