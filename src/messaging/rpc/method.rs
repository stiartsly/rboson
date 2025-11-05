use serde_repr::{Serialize_repr, Deserialize_repr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub(crate) enum RPCMethod {
    UserProfile     = 0x01,

    DeviceList      = 0x11,
    DeviceRevoke    = 0x12,

    ContactPush     = 0x21,
    ContactClear    = 0x23,

    ChannelCreate   = 0x31,
    ChannelDelete   = 0x32,
    ChannelJoin     = 0x33,
    ChannelLeave    = 0x34,
    ChannelInfo     = 0x35,
    ChannelMembers  = 0x36,
    ChannelOwner    = 0x37,
    ChannelPermission = 0x38,
    ChannelName     = 0x39,
    ChannelNotice   = 0x3A,
    ChannelRole     = 0x3B,
    ChannelBan      = 0x3C,
    ChannelUnban    = 0x3D,
    ChannelRemove   = 0x3E,
}

impl TryFrom<u8> for RPCMethod {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(RPCMethod::UserProfile),
            0x11 => Ok(RPCMethod::DeviceList),
            0x12 => Ok(RPCMethod::DeviceRevoke),
            0x21 => Ok(RPCMethod::ContactPush),
            0x23 => Ok(RPCMethod::ContactClear),
            0x31 => Ok(RPCMethod::ChannelCreate),
            0x32 => Ok(RPCMethod::ChannelDelete),
            0x33 => Ok(RPCMethod::ChannelJoin),
            0x34 => Ok(RPCMethod::ChannelLeave),
            0x35 => Ok(RPCMethod::ChannelInfo),
            0x36 => Ok(RPCMethod::ChannelMembers),
            0x37 => Ok(RPCMethod::ChannelOwner),
            0x38 => Ok(RPCMethod::ChannelPermission),
            0x39 => Ok(RPCMethod::ChannelName),
            0x3A => Ok(RPCMethod::ChannelNotice),
            0x3B => Ok(RPCMethod::ChannelRole),
            0x3C => Ok(RPCMethod::ChannelBan),
            0x3D => Ok(RPCMethod::ChannelUnban),
            0x3E => Ok(RPCMethod::ChannelRemove),

            _    => Err(format!("Invalid method: {:#X}", value)),
        }
    }
}

impl From<RPCMethod> for i32 {
    fn from(p: RPCMethod) -> Self {
        p as i32
    }
}
