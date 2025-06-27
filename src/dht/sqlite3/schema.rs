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
        announced -> BigInt
    }
}

diesel::table! {
    peers (id) {
        id -> Binary,
        nodeId -> Binary,
        origin -> Binary,
        persistent -> Bool,
        privateKey -> Nullable<Binary>,
        port -> Integer,
        alternativeURL -> Nullable<Text>,
        signature -> Binary,
        timestamp -> BigInt,
        announced -> BigInt,
    }
}
