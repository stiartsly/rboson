use std::fmt;
use std::result::Result as SResult;
use serde::{
    Serialize, Deserialize, Serializer, Deserializer,
    de::{self, Visitor, MapAccess, IgnoredAny},
    ser::SerializeMap,
};

use crate::{Id, PeerInfo};

pub(crate) struct AnnouncePeerRequest {
    token:  i32,
    expected_seq: i32, // -1 means unset
    peer:   PeerInfo,
}

impl AnnouncePeerRequest {
    pub(crate) fn new(
        peer: PeerInfo,
        token: i32,
        expected_seq: i32
    ) -> Self {
        Self { token, expected_seq, peer }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn expected_seq(&self) -> i32 {
        self.expected_seq
    }

    pub(crate) fn peer(&self) -> &PeerInfo {
        &self.peer
    }
}

impl Serialize for AnnouncePeerRequest {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let peer = &self.peer;
        let mut map = se.serialize_map(None)?;

        map.serialize_entry("tok", &self.token)?;
        if self.expected_seq >= 0 {
            map.serialize_entry("cas", &self.expected_seq)?;
        }

        map.serialize_entry("k", peer.id())?;
        map.serialize_entry("n", peer.nonce())?;
        if peer.sequence_number() > 0 {
            map.serialize_entry("seq", &peer.sequence_number())?;
        }
        if peer.is_authenticated() {
            map.serialize_key("o")?;
            map.serialize_value(peer.nodeid().unwrap())?;
            map.serialize_key("os")?;
            map.serialize_value(peer.node_signature().unwrap())?;
        }
        map.serialize_key("sig")?;
        map.serialize_value(peer.signature())?;
        map.serialize_entry("f", &peer.fingerprint())?;
        map.serialize_entry("e", peer.endpoint())?;
        if let Some(extra) = peer.extra_data() {
            map.serialize_key("ex")?;
            map.serialize_value(extra)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for AnnouncePeerRequest {
    fn deserialize<D>(de: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Token,          // "tok"
            Cas,            // "cas"
            PeerId,         // "k"
            Nonce,          // "n"
            Seq,            // "seq"
            NodeId,         // "o"
            NodeSig,        // "os"
            Signature,      // "sig"
            Fingerprint,    // "f"
            Endpoint,       // "e"
            Extra,          // "ex"
            Ignore          // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "tok"   => Ok(Field::Token),
                    "cas"   => Ok(Field::Cas),
                    "k"     => Ok(Field::PeerId),
                    "n"     => Ok(Field::Nonce),
                    "seq"   => Ok(Field::Seq),
                    "o"     => Ok(Field::NodeId),
                    "os"    => Ok(Field::NodeSig),
                    "sig"   => Ok(Field::Signature),
                    "f"     => Ok(Field::Fingerprint),
                    "e"     => Ok(Field::Endpoint),
                    "ex"    => Ok(Field::Extra),
                    _       => Ok(Field::Ignore),
                }
            }
        }

        struct FieldVisiter;
        impl<'de> Visitor<'de> for FieldVisiter {
            type Value = AnnouncePeerRequest;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("AnnouncePeerRequest")
            }

            #[allow(unused)]
            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut token: i32 = 0;
                let mut expected_seq: i32 = -1;
                let mut peer_id: Option<Id> = None;
                let mut nonce: Option<Vec<u8>> = None;
                let mut seq: i32 = 0;
                let mut node_id: Option<Id> = None;
                let mut node_sig: Option<Vec<u8>> = None;
                let mut sig: Option<Vec<u8>> = None;
                let mut fingerprint: u64 = 0;
                let mut endpoint: Option<String> = None;
                let mut extra: Option<Vec<u8>> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Token        => token = map.next_value()?,
                        Field::Cas          => expected_seq = map.next_value()?,
                        Field::PeerId       => peer_id = Some(map.next_value()?),
                        Field::Nonce        => nonce = Some(map.next_value()?),
                        Field::Seq          => seq = map.next_value()?,
                        Field::NodeId       => node_id = map.next_value()?,
                        Field::NodeSig      => node_sig = map.next_value()?,
                        Field::Signature    => sig = Some(map.next_value()?),
                        Field::Fingerprint  => fingerprint = map.next_value()?,
                        Field::Endpoint     => endpoint = Some(map.next_value()?),
                        Field::Extra        => extra = map.next_value()?,
                        Field::Ignore       => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                let peer = PeerInfo::packed(
                    peer_id.ok_or_else(|| de::Error::missing_field("k"))?,
                    nonce.ok_or_else(|| de::Error::missing_field("n"))?,
                    seq,
                    node_id,
                    node_sig,
                    sig.ok_or_else(|| de::Error::missing_field("sig"))?,
                    fingerprint,
                    endpoint.ok_or_else(|| de::Error::missing_field("e"))?,
                    extra,
                );

                Ok(AnnouncePeerRequest::new(peer, token, expected_seq))
            }
        }

        de.deserialize_map(FieldVisiter)
    }
}

impl fmt::Display for AnnouncePeerRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tok:{}, expected_seq:{},peer:[{}]",
            self.token,
            self.expected_seq,
            self.peer
        )
    }
}
