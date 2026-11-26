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
    rc::Rc,
};
use futures::{
    FutureExt,
    stream::FuturesUnordered,
};
use boson_api::{
    Id,
    errors::{Result, StateError},
};
use crate::{
    message::Message,
    transport::RpcServer,
};
use super::{
    promise::Promise,
    event::{Event, Body}
};

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
        Event::Start(Body((), promise)) => {
            respond(pending, promise, async move {
                rpc.start().await
            });
        }
        Event::Stop(Body((), promise)) => {
            stop = true;
            respond(pending, promise, async move {
                rpc.stop().await
            });
        }

        // commands related to conversations
        Event::GetConversation(Body(id, promise)) => {
            respond(pending, promise, async move {
                rpc.get_conversation(&id).await
            });
        }
        Event::GetConversationList(Body((), promise)) => {
            respond(pending, promise, async move {
                rpc.get_conversation_list().await
            });
        }
        Event::RemoveConversation(Body(id, promise)) => {
            respond(pending, promise, async move {
                rpc.remove_conversation(&id).await
            });
        }
        Event::RemoveConversationList(Body(ids, promise)) => {
            respond(pending, promise, async move {
                rpc.remove_conversation_list(&ids).await
            });
        }

        Event::GetMessages(Body(params, promise)) => {
            respond(pending, promise, async move {
                match (params.begin, params.end) {
                    (Some(begin), Some(end)) => {
                        rpc.get_messages_in_range(&params.conversation_id, begin, end).await
                    }
                    _ => {
                        rpc.get_messages(
                            &params.conversation_id,
                            params.until,
                            params.limit.unwrap_or_default(),
                            params.offset.unwrap_or_default(),
                        ).await
                    }
                }
            });
        }
        Event::RemoveMessageList(Body(ids, promise)) => {
            respond(pending, promise, async move {
                rpc.remove_messages_by_ids(&ids).await
            });
        }
        Event::RemoveMessagesByConversation(Body(id, promise)) => {
            respond(pending, promise, async move {
                rpc.remove_messages_by_conversation(&id).await
            });
        }
        Event::GetSessions(Body((), promise)) => {
            respond(pending, promise, async move {
                rpc.get_sessions().await
            });
        }
        Event::RevokeSession(Body(device_id, promise)) => {
            respond(pending, promise, async move {
                rpc.revoke_session(&device_id).await
            });
        }
        Event::FriendRequest(Body((user_id, hello), promise)) => {
            respond(pending, promise, async move {
                rpc.send_friend_request(&user_id, hello).await
            });
        }
        Event::FriendAccept(Body(user_id, promise)) => {
            respond(pending, promise, async move {
                rpc.accept_friend_request(&user_id).await
            });
        }
        Event::GetFriendRequests(Body(id, promise)) => {
            respond(pending, promise, async move {
                match id {
                    Some(user_id) => Ok(rpc.friend_request(&user_id).into_iter().collect()),
                    None => Ok(rpc.friend_requests())
                }
            });
        }
        Event::RemoveFriendRequests(Body(user_ids, promise)) => {
            respond(pending, promise, async move {
                rpc.remove_friend_requests(&user_ids)
            });
        }
        Event::ContentMessage(Body(params, promise)) => {
            respond(pending, promise, async move {
                send_content_message(
                    &rpc,
                    params.conversation_id,
                    params.metadata,
                    params.content,
                    params.is_encrypted,
                ).await
            });
        }
        Event::RegisterFriendSession(Body((user_id, session_key, remark), promise)) => {
            respond(pending, promise, async move {
                rpc.add_friend(user_id, session_key, remark).await
            });
        }
        Event::BlockUser(Body(user_id, promise)) => {
            respond(pending, promise, async move {
                rpc.block_user(&user_id).await
            });
        }

        Event::CreateChannel(Body(channel_info, promise)) => {
            respond(pending, promise, async move {
                rpc.create_channel(channel_info).await
            });
        }
        Event::RemoveChannel(Body(channel_id, promise)) => {
            respond(pending, promise, async move {
                rpc.delete_channel(&channel_id).await
            });
        }

        Event::JoinChannel(Body(ticket, promise)) => {
            respond(pending, promise, async move {
                rpc.join_channel(ticket).await
            });
        }

        Event::LeaveChannel(Body(channel_id, promise)) => {
            respond(pending, promise, async move {
                rpc.leave_channel(&channel_id).await
            });
        }

        Event::CreateInviteTicket(Body((channel_id, invitee), promise)) => {
            respond(pending, promise, async move {
                rpc.create_invite_ticket(&channel_id, invitee).await
            });
        }

        Event::TransferChannelOwnership(Body((channel_id, new_owner), promise)) => {
            respond(pending, promise, async move {
                rpc.transfer_channel_ownership(&channel_id, new_owner).await
            });
        }

        Event::RotateChannelSessionKey(Body(input, promise)) => {
            respond(pending, promise, async move {
                rpc.rotate_channel_session_key(input.0, input.1).await
            });
        }

        Event::UpdateChannelInfo(Body(channel, promise)) => {
            respond(pending, promise, async move {
                rpc.update_channel_info(channel).await
            });
        }

        Event::SetChannelMembersRole(Body((id, mut role), promise)) => {
            respond(pending, promise, async move {
                rpc.set_channel_members_role(&id, role.ids_take(), role.role()).await
            });
        }
        Event::BanChannelMembers(Body((id, ids), promise)) => {
            respond(pending, promise, async move {
                rpc.ban_channel_members(&id, ids).await
            });
        }
        Event::UnbanChannelMembers(Body((id, ids), promise)) => {
            respond(pending, promise, async move {
                rpc.unban_channel_members(&id, ids).await
            });
        }
        Event::RemoveChannelMembers(Body((id, ids), promise)) => {
            respond(pending, promise, async move {
                rpc.remove_channel_members(&id, ids).await
            });
        }
        Event::GetContacts(Body(id_opt, promise)) => {
            respond(pending, promise, async move {
                match id_opt {
                    Some(id) => Ok(rpc.get_contact(&id).await.into_iter().collect()),
                    None => Ok(rpc.get_contacts().await),
                }
            });
        }
        Event::UpdateContact(Body(contact, promise)) => {
            respond(pending, promise, async move {
                rpc.update_contact(contact).await
            });
        }
        Event::RemoveContacts(Body(ids, promise)) => {
            respond(pending, promise, async move {
                rpc.remove_contacts(&ids).await
            });
        }
        Event::ClearContacts(Body(_, promise)) => {
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
) -> Result<Message> {
    let mut builder = engine.message(Some(recipient));
    for (key, value) in headers {
        let value = value
            .as_str()
            .ok_or_else(|| StateError::new(format!("Message header '{key}' is not text")))?;
        builder = builder.with_header(&key, value);
    }
    builder = if text {
        let content =
            String::from_utf8(body).map_err(|error| StateError::new(error.to_string()))?;
        builder.with_text_body(&content)
    } else {
        builder.with_binary_body(body)
    };
    builder.send().await
}
