// const VERSION: i32 = 4;

pub(crate) const SET_USER_VERSION: &str = "PRAGMA user_version = 4";
pub(crate) const GET_USER_VERSION: &str = "PRAGMA user_version";

pub(crate) const CREATE_VALUES_TABLE: &str = "
        CREATE TABLE IF NOT EXISTS valores(\
        id BLOB NOT NULL PRIMARY KEY, \
        persistent BOOLEAN NOT NULL DEFAULT FALSE, \
        publicKey BLOB, \
        privateKey BLOB, \
        recipient BLOB, \
        nonce BLOB, \
        signature BLOB, \
        sequenceNumber INTEGER, \
        data BLOB, \
        timestamp INTEGER NOT NULL, \
        announced INTEGER NOT NULL DEFAULT 0\
        ) WITHOUT ROWID
    ";

pub(crate) const CREATE_VALUES_INDEX: &str = "
        CREATE INDEX IF NOT EXISTS idx_valores_timpstamp ON valores(timestamp)
    ";

pub(crate) const CREATE_PEERS_TABLE: &str = "
        CREATE TABLE IF NOT EXISTS peers( \
        id BLOB NOT NULL, \
        nodeId BLOB NOT NULL, \
        origin BLOB NOT NULL, \
        persistent BOOLEAN NOT NULL DEFAULT FALSE, \
        privateKey BLOB, \
        port INTEGER NOT NULL, \
        alternativeURL VARCHAR(512), \
        signature BLOB NOT NULL, \
        timestamp INTEGER NOT NULL, \
        announced INTEGER NOT NULL DEFAULT 0, \
        PRIMARY KEY(id, nodeId, origin)\
        ) WITHOUT ROWID
    ";


pub(crate) const CREATE_PEERS_INDEX: &str = "
        CREATE INDEX IF NOT EXISTS idx_peers_timpstamp ON peers (timestamp)
    ";

pub(crate) const CREATE_PEERS_ID_INDEX: &str = "
        CREATE INDEX IF NOT EXISTS idx_peers_id ON peers(id)
    ";

pub(crate) const DROP_VALUES_TABLE: &str = "
        DROP TABLE IF EXISTS valores
    ";

pub(crate) const DROP_VALUES_INDEX: &str = "
        DROP INDEX IF EXISTS idx_valores_timpstamp
    ";

pub(crate) const DROP_PEERS_TABLE: &str = "
        DROP TABLE IF EXISTS peers
    ";

pub(crate) const DROP_PEERS_INDEX: &str = "
        DROP INDEX IF EXISTS idx_peers_timpstamp
    ";

pub(crate) const DROP_PEERS_ID_INDEX: &str = "
        DROP INDEX IF EXISTS idx_peers_id
    ";
