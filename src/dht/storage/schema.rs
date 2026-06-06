diesel::table! {
    valores (id) {
        id -> Binary,
        publicKey -> Nullable<Binary>,
        privateKey -> Nullable<Binary>,
        recipient -> Nullable<Binary>,
        nonce -> Nullable<Binary>,
        signature -> Nullable<Binary>,
        sequenceNumber -> Integer,
        data -> Binary,
        persistent -> Bool,
        updated -> BigInt,
    }
}

diesel::table! {
    peers (id, fingerprint) {
        id -> Binary,
        fingerprint -> BigInt,
        privateKey -> Nullable<Binary>,
        nonce -> Binary,
        sequenceNumber -> Integer,
        nodeId -> Nullable<Binary>,
        nodeSignature -> Nullable<Binary>,
        signature -> Binary,
        endpoint -> Text,
        extra -> Nullable<Binary>,
        persistent -> Bool,
        updated -> BigInt,
    }
}
