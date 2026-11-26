// SQLite SQL statements used by the concrete messaging store.

pub(super) const PRAGMAS: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA foreign_keys = ON;
    PRAGMA busy_timeout = 5000;
    PRAGMA locking_mode = NORMAL;
    PRAGMA synchronous = FULL;
";

pub(super) const CREATE_SCHEMA: &str = "
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

pub(super) const CONTACT_SELECT: &str = "
    SELECT c.*, ch.owner, ch.permission, ch.notice, ch.announce
    FROM contacts c
    LEFT JOIN channels ch ON c.id = ch.id
";

pub(super) const CONVERSATION_SELECT: &str = "
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

pub(super) const UPSERT_CONTACTS_REVISION: &str = "
    INSERT INTO contacts_revision (id, revision, updated_at) \
    VALUES (1, ?, ?) \
    ON CONFLICT (id) DO UPDATE SET \
        revision = EXCLUDED.revision, \
        updated_at = EXCLUDED.updated_at
";

pub(super) const UPSERT_CONTACT: &str = "
    INSERT INTO contacts \
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
        updated_at = EXCLUDED.updated_at
";

pub(super) const UPSERT_CHANNEL: &str = "
    INSERT INTO channels (id, owner, permission, notice, announce) \
    VALUES (?, ?, ?, ?, ?) \
    ON CONFLICT (id) DO UPDATE SET \
        owner = EXCLUDED.owner, \
        permission = EXCLUDED.permission, \
        notice = EXCLUDED.notice, \
        announce = EXCLUDED.announce
";

pub(super) const SET_USER_VERSION: &str = "PRAGMA user_version = 1";

pub(super) const SELECT_USER_VERSION: &str = "PRAGMA user_version";

pub(super) const SELECT_CONTACTS_REVISION: &str = "
    SELECT revision FROM contacts_revision WHERE id = 1
";

pub(super) const DELETE_CONTACT_BY_ID: &str = "
    DELETE FROM contacts WHERE id = ?
";

pub(super) const DELETE_MESSAGES_BY_CONVERSATION: &str = "
    DELETE FROM messages WHERE conversation_id = ?
";

pub(super) const DELETE_ALL_CONTACTS: &str = "
    DELETE FROM contacts
";

pub(super) const EXISTS_CONTACT: &str = "
    SELECT EXISTS(SELECT 1 FROM contacts WHERE id = ?) AS exists
";

pub(super) const INSERT_MESSAGE: &str = "
    INSERT INTO messages \
        (id, conversation_id, version, recipient, type, from_id, created_at,\
            sent_at, received_at, content_type, content_disposition, payload) \
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
    RETURNING rid
";

pub(super) const UPDATE_MESSAGE_SENT_AT: &str = "
    UPDATE messages SET sent_at = ? WHERE id = ?
";

pub(super) const SELECT_MESSAGES_IN_RANGE: &str = "
    SELECT * FROM messages \
    WHERE conversation_id = ? AND created_at >= ? AND created_at < ? \
    ORDER BY rid ASC
";

pub(super) const SELECT_MESSAGES_BEFORE: &str = "
    SELECT * FROM messages \
    WHERE conversation_id = ? AND created_at <= ? \
    ORDER BY rid ASC \
    LIMIT ? OFFSET ?
";

pub(super) const DELETE_MESSAGE_BY_RID: &str = "DELETE FROM messages WHERE rid = ?";

pub(super) const DELETE_ALL_MESSAGES: &str = "DELETE FROM messages";

pub(super) const UPSERT_FRIEND_REQUEST: &str = "
    INSERT INTO friend_requests \
        (id, initiator, hello, created_at, updated_at, accepted, accepted_at) \
    VALUES (?, ?, ?, ?, ?, ?, ?) \
    ON CONFLICT (id) DO UPDATE SET \
        initiator = EXCLUDED.initiator, \
        hello = EXCLUDED.hello, \
        created_at = EXCLUDED.created_at, \
        updated_at = EXCLUDED.updated_at, \
        accepted = EXCLUDED.accepted, \
        accepted_at = EXCLUDED.accepted_at
";

pub(super) const SELECT_FRIEND_REQUEST_BY_ID: &str = "
    SELECT * FROM friend_requests WHERE id = ?
";

pub(super) const SELECT_FRIEND_REQUESTS: &str = "
    SELECT * FROM friend_requests ORDER BY created_at DESC
";

pub(super) const DELETE_FRIEND_REQUEST_BY_ID: &str = "
    DELETE FROM friend_requests WHERE id = ?
";

pub(super) const DELETE_ALL_FRIEND_REQUESTS: &str = "
    DELETE FROM friend_requests
";

pub(super) const UPDATE_CHANNEL_OWNER: &str = "
    UPDATE channels SET owner = ? WHERE id = ?
";

pub(super) const UPDATE_CHANNEL_MEMBER_ROLE: &str = "
    UPDATE channel_members SET role = ? WHERE channel_id = ? AND id = ?
";

pub(super) const UPSERT_CHANNEL_MEMBER: &str = "
    INSERT INTO channel_members (id, channel_id, role, joined) \
    VALUES (?, ?, ?, ?) \
    ON CONFLICT (id, channel_id) DO UPDATE SET role = EXCLUDED.role
";

pub(super) const DELETE_CHANNEL_MEMBERS_BY_CHANNEL: &str = "
    DELETE FROM channel_members WHERE channel_id = ?
";

pub(super) const SELECT_CHANNEL_MEMBER: &str = "
    SELECT * FROM channel_members WHERE channel_id = ? AND id = ?
";

pub(super) const SELECT_CHANNEL_MEMBERS: &str = "
    SELECT * FROM channel_members WHERE channel_id = ? ORDER BY joined ASC
";

pub(super) const DELETE_CHANNEL_MEMBER: &str = "
    DELETE FROM channel_members WHERE channel_id = ? AND id = ?
";

pub(super) const SELECT_CONTACT_BY_ID: &str = "
    SELECT c.*, ch.owner, ch.permission, ch.notice, ch.announce
    FROM contacts c
    LEFT JOIN channels ch ON c.id = ch.id
    WHERE c.id = ?\
";

pub(super) const SELECT_CONVERSATION_BY_ID: &str = "
    SELECT c.*, ch.owner, ch.permission, ch.notice, ch.announce,
        m.rid as last_message_rid, m.id as last_message_id,
        m.conversation_id as last_message_conversation_id, m.version as last_message_version,
        m.recipient as last_message_recipient, m.type as last_message_type, m.from_id as last_message_from_id,
        m.created_at as last_message_created_at, m.content_type as last_message_content_type,
        m.content_disposition as last_message_content_disposition, m.payload as last_message_payload,
        m.sent_at as last_message_sent_at, m.received_at as last_message_received_at
    FROM contacts c
    LEFT JOIN channels ch ON c.id = ch.id
    JOIN messages m ON c.id = m.conversation_id \
             WHERE c.id = ? ORDER BY m.rid DESC LIMIT 1\
";

pub(super) const SELECT_CONVERSATIONS: &str = "
    SELECT c.*, ch.owner, ch.permission, ch.notice, ch.announce,
        m.rid as last_message_rid, m.id as last_message_id,
        m.conversation_id as last_message_conversation_id, m.version as last_message_version,
        m.recipient as last_message_recipient, m.type as last_message_type, m.from_id as last_message_from_id,
        m.created_at as last_message_created_at, m.content_type as last_message_content_type,
        m.content_disposition as last_message_content_disposition, m.payload as last_message_payload,
        m.sent_at as last_message_sent_at, m.received_at as last_message_received_at
    FROM contacts c
    LEFT JOIN channels ch ON c.id = ch.id
    JOIN (SELECT conversation_id, MAX(rid) as max_rid FROM messages GROUP BY conversation_id) \
        last_msg_rid ON c.id = last_msg_rid.conversation_id \
    JOIN messages m ON m.rid = last_msg_rid.max_rid \
    ORDER BY m.received_at DESC, m.rid DESC\
";
