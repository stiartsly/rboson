diesel::table! {
    valores (id) {
        id -> Binary,
        persistent -> Bool,
        publicKey -> Nullable<Binary>,
        privateKey -> Nullable<Binary>,
        recipient -> Nullable<Binary>,
        nonce -> Nullable<Binary>,
        signature -> Nullable<Binary>,
        sequenceNumber -> Integer,
        data -> Binary,
        timestamp -> BigInt,
        announced -> BigInt,
    }
}

diesel::table! {
    peers (id, fingerprint) {
        id -> Binary,
        fingerprint -> BigInt,
        persistent -> Bool,
        privateKey -> Nullable<Binary>,
        nonce -> Binary,
        sequenceNumber -> Integer,
        nodeId -> Nullable<Binary>,
        nodeSignature -> Nullable<Binary>,
        signature -> Binary,
        endpoint -> Text,
        extra -> Nullable<Binary>,
        timestamp -> BigInt,
        announced -> BigInt,
    }
}
