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
    collections::HashMap,
    future::Future,
    pin::Pin,
    rc::Rc
};
use futures::{FutureExt, stream::FuturesUnordered};

use boson_api::{
    Id,
    errors::{Result, StateError},
};
use crate::{
    message::Message,
    rpc_server::RpcServer
};
use super::promise::Promise;
use super::event::{Event, WithParams, NoParams};

pub(crate) type Pending = FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>;

fn respond<T: 'static>(
    pending: &mut Pending,
    promise: Promise<T>,
    command: impl Future<Output = Result<T>> + 'static,
) {
    pending.push( async move {
        promise.complete(command.await.map_err(|e| e.to_string()));
    }.boxed_local());
}

pub(super) fn handle(
    event: Event,
    rpc: &Rc<RpcServer>,
    pending: &mut Pending
) -> bool {
    let rpc = rpc.clone();
    let mut stop = false;
    match event {
        Event::Start(NoParams {promise }) => {
            respond(pending, promise, async move {
                rpc.start().await
            });
        }
        Event::Stop(NoParams { promise}) => {
            stop = true;
            respond(pending, promise, async move {
                rpc.stop().await
            });
        }
        Event::GetConversation(WithParams {data: id, promise}) => {
            respond(pending, promise, async move {
                rpc.get_conversation(&id).await
            });
        }
        Event::GetConversations(NoParams {promise}) => {
            respond(pending, promise, async move {
                rpc.get_conversations().await
            });
        }
        Event::RemoveConversation(WithParams {data: id, promise}) => {
            respond(pending, promise, async move {
                rpc.remove_conversation(&id).await
            });
        }
        Event::RemoveConversations(WithParams {data: ids, promise}) => {
            respond(pending, promise, async move {
                rpc.remove_conversations(&ids).await
            });
        }
        Event::GetMessages(WithParams {data: (conversation_id, until, limit, offset), promise}) => {
            respond(pending, promise, async move {
                rpc.get_messages(&conversation_id, until, limit, offset).await
            });
        }
        Event::GetMessagesInRange(WithParams {
            data: (conversation_id, begin, end),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.get_messages_in_range(&conversation_id, begin, end
                ).await });
        }
        Event::RemoveMessage(WithParams {
            data: message_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_message(message_id).await
            });
        }
        Event::RemoveMessagesByIds(WithParams {
            data: message_ids,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_messages_by_ids(&message_ids).await
            });
        }
        Event::RemoveMessagesInConversation(WithParams {
            data: conversation_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_messages_in_conversation(&conversation_id).await
            });
        }
        Event::GetSessions(NoParams {promise }) => {
            respond(pending, promise, async move {
                rpc.get_sessions().await
            });
        }
        Event::RevokeSession(WithParams {
            data: device_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.revoke_session(&device_id).await
            });
        }
        Event::FriendRequest(WithParams {
            data: (user_id, hello),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.friend_request(&user_id, hello).await
            });
        }
        Event::FriendAccept(WithParams {
            data: user_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.accept_friend_request(&user_id).await
            });
        }
        Event::GetFriendRequest(WithParams {
            data: user_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.get_friend_request(&user_id).await
            });
        }
        Event::GetFriendRequests(NoParams { promise }) => {
            respond( pending, promise, async move {
                rpc.get_friend_requests().await
            });
        }
        Event::RemoveFriendRequest(WithParams {
            data: user_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_friend_request(&user_id).await
            });
        }
        Event::RemoveFriendRequests(WithParams {
            data: user_ids,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_friend_requests(&user_ids).await
            });
        }
        Event::ClearFriendRequests(NoParams {
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.clear_friend_requests().await
            });
        }
        Event::ContentMessage(WithParams {
            data: (recipient, headers, body, text),
            promise,
        }) => {
            respond(pending, promise, async move {
                send_content_message(&rpc, recipient, headers, body, text).await
            });
        }
        Event::RegisterFriendSession(WithParams {
            data: (user_id, session_key, remark),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.add_friend(&user_id, session_key, remark).await
            });
        }
        Event::BlockUser(WithParams {
            data: user_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.block_user(&user_id).await
            });
        }

        Event::CreateChannel(WithParams {
            data: (permission, name, notice, announcement),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.create_channel(permission, name, notice, announcement).await
            });
        }
        Event::RemoveChannel(WithParams {
            data: channel_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_channel(&channel_id).await
            });
        }

        Event::JoinChannel(WithParams {
            data: ticket,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.join_channel(ticket).await
            });
        }

        Event::LeaveChannel(WithParams {
            data: channel_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.leave_channel(&channel_id).await
            });
        }

        Event::CreateInviteTicket(WithParams {
            data: (channel_id, invitee),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.create_invite_ticket(&channel_id, invitee).await
            });
        }

        Event::TransferChannelOwnership(WithParams {
            data: (channel_id, new_owner),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.transfer_channel_ownership(&channel_id, new_owner).await
            });
        }

        Event::RotateChannelSessionKey(WithParams {
            data: channel_id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.rotate_channel_session_key(&channel_id).await
            });
        }

        Event::UpdateChannelInfo(WithParams {
            data: channel,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.update_channel_info(&channel).await
            });
        }

        Event::SetChannelMemberRole(WithParams {
            data: (channel_id, member_ids, role),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.set_channel_members_role(&channel_id, &member_ids, role).await
            });
        }

        Event::BanChannelMembers(WithParams {
            data: (channel_id, member_ids),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.ban_channel_members(&channel_id, &member_ids).await
            });
        }
        Event::UnbanChannelMembers(WithParams {
            data: (channel_id, member_ids),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.unban_channel_members(&channel_id, &member_ids).await
            });
        }
        Event::RemoveChannelMembers(WithParams {
            data: (channel_id, members),
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_channel_members(&channel_id, &members).await
            });
        }
        Event::GetContact(WithParams {
            data: id,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.get_contact(&id).await
            });
        }
        Event::GetContacts(NoParams {
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.get_contacts().await
            });
        }
        Event::UpdateContact(WithParams {
            data: contact,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.update_contact(&contact).await
            });
        }
        Event::RemoveContacts(WithParams {
            data: ids,
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_contacts(&ids).await
            });
        }
        Event::ClearContacts(NoParams {
            promise,
        }) => {
            respond(pending, promise, async move {
                rpc.clear_contacts().await
            });
        }
    }
    stop
}

async fn send_content_message(
    engine: &Rc<RpcServer>,
    recipient: Id,
    headers: HashMap<String, serde_json::Value>,
    body: Vec<u8>,
    text: bool,
) -> Result<Box<dyn Message>> {
    let mut builder = engine.message(Some(recipient));
    for (key, value) in headers {
        let value = value
            .as_str()
            .ok_or_else(|| StateError::new(format!("Message header '{key}' is not text")))?;
        builder = builder.header(&key, value);
    }
    builder = if text {
        let content =
            String::from_utf8(body).map_err(|error| StateError::new(error.to_string()))?;
        builder.text_body(&content)
    } else {
        builder.binary_body(body)
    };
    builder.send().await
}
