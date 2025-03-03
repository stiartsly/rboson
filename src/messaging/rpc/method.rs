
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
#[allow(dead_code)]
pub enum RPCMethod {
    // Device-related methods
    DeviceList = 0x00,
    DeviceRevoke = 0x01,

    // Contact-related methods
    ContactPut = 0x10,
    ContactRemove = 0x11,
    ContactList = 0x12,
    ContactClear = 0x13,

    // Group-related methods
    GroupCreate = 0x20,
    GroupMembers = 0x21,
    GroupUpdate = 0x22,
    GroupDelete = 0x23,
    GroupRole = 0x24,
    GroupBan = 0x25,
    GroupUnban = 0x26,
    GroupRemove = 0x27,
    GroupJoin = 0x28,
    GroupLeave = 0x29,
}

#[allow(dead_code)]
impl RPCMethod {
    pub fn is_node_context(&self) -> bool {
        self <= &RPCMethod::GroupCreate
    }

    pub fn is_group_context(&self) -> bool {
        self >= &RPCMethod::GroupCreate
    }
}

impl TryFrom<u8> for RPCMethod {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(RPCMethod::DeviceList),
            0x01 => Ok(RPCMethod::DeviceRevoke),
            0x10 => Ok(RPCMethod::ContactPut),
            0x11 => Ok(RPCMethod::ContactRemove),
            0x12 => Ok(RPCMethod::ContactList),
            0x13 => Ok(RPCMethod::ContactClear),
            0x20 => Ok(RPCMethod::GroupCreate),
            0x21 => Ok(RPCMethod::GroupMembers),
            0x22 => Ok(RPCMethod::GroupUpdate),
            0x23 => Ok(RPCMethod::GroupDelete),
            0x24 => Ok(RPCMethod::GroupRole),
            0x25 => Ok(RPCMethod::GroupBan),
            0x26 => Ok(RPCMethod::GroupUnban),
            0x27 => Ok(RPCMethod::GroupRemove),
            0x28 => Ok(RPCMethod::GroupJoin),
            0x29 => Ok(RPCMethod::GroupLeave),
            _ => Err(format!("Invalid method: {}", value)),
        }
    }
}
