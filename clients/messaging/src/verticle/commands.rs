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
    conversation::Conversation,
    MessageBuilderOps,
    message::Message,
    rpc_server::RpcServer,
};
use super::{
    promise::Promise,
    event::{
        Event, Body,
        OPERATE_SET_MEMBER_ROLE, OPERATE_BAN_MEMBERS,
        OPERATE_UNBAN_MEMBERS, OPERATE_REMOVE_MEMBERS,
    },
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
        Event::Start(Body {params: (), result: promise }) => {
            respond(pending, promise, async move {
                rpc.start().await
            });
        }
        Event::Stop(Body {params: (), result: promise}) => {
            stop = true;
            respond(pending, promise, async move {
                rpc.stop().await
            });
        }
        Event::GetConversations(Body {params: id, result: promise}) => {
            respond(pending, promise, async move {
                match id {
                    Some(id) => Ok(rpc
                        .get_conversation(&id)
                        .await?
                        .map(|conversation| Box::new(conversation) as Conversation)
                        .into_iter()
                        .collect()),
                    None => Ok(rpc
                        .get_conversations()
                        .await?
                        .into_iter()
                        .map(|conversation| Box::new(conversation) as Conversation)
                        .collect()),
                }
            });
        }
        Event::RemoveConversations(Body {params: ids, result: promise}) => {
            respond(pending, promise, async move {
                rpc.remove_conversations(&ids).await
            });
        }
        Event::GetMessages(Body {params, result: promise}) => {
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
        Event::RemoveMessages(Body {params, result: promise}) => {
            respond(pending, promise, async move {
                if let Some(messages) = params.messages {
                    rpc.remove_messages_by_ids(&messages).await
                } else if let Some(conversation_id) = params.conversation_id {
                    rpc.remove_messages_in_conversation(&conversation_id).await
                } else {
                    Ok(())
                }
            });
        }
        Event::GetSessions(Body {params: (), result: promise}) => {
            respond(pending, promise, async move {
                rpc.get_sessions().await
            });
        }
        Event::RevokeSession(Body {
            params: device_id,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.revoke_session(&device_id).await
            });
        }
        Event::FriendRequest(Body {
            params,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.friend_request(&params.user_id, params.hello).await
            });
        }
        Event::FriendAccept(Body {
            params: user_id,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.accept_friend_request(&user_id).await
            });
        }
        Event::GetFriendRequests(Body {params: id, result: promise}) => {
            respond(pending, promise, async move {
                match id {
                    Some(user_id) => Ok(rpc.get_friend_request(&user_id).await?.into_iter().collect()),
                    None => rpc.get_friend_requests().await,
                }
            });
        }
        Event::RemoveFriendRequests(Body {
            params: user_ids,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_friend_requests(&user_ids).await
            });
        }
        Event::ContentMessage(Body {
            params,
            result: promise,
        }) => {
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
        Event::RegisterFriendSession(Body {
            params,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.add_friend(&params.user_id, params.session_key, params.remark).await
            });
        }
        Event::BlockUser(Body {
            params: user_id,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.block_user(&user_id).await
            });
        }

        Event::CreateChannel(Body {
            params,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.create_channel(params.permission, params.name, params.notice, params.announcement).await
            });
        }
        Event::RemoveChannel(Body {
            params: channel_id,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.remove_channel(&channel_id).await
            });
        }

        Event::JoinChannel(Body {
            params: ticket,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.join_channel(ticket).await
            });
        }

        Event::LeaveChannel(Body {
            params: channel_id,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.leave_channel(&channel_id).await
            });
        }

        Event::CreateInviteTicket(Body {
            params: (channel_id, invitee),
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.create_invite_ticket(&channel_id, invitee).await
            });
        }

        Event::TransferChannelOwnership(Body {
            params: (channel_id, new_owner),
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.transfer_channel_ownership(&channel_id, new_owner).await
            });
        }

        Event::RotateChannelSessionKey(Body {
            params: channel_id,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.rotate_channel_session_key(&channel_id).await
            });
        }

        Event::UpdateChannelInfo(Body {
            params: channel,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                rpc.update_channel_info(&channel).await
            });
        }

        Event::OperateChannelMembers(Body {
            params,
            result: promise,
        }) => {
            respond(pending, promise, async move {
                match params.operation {
                    OPERATE_SET_MEMBER_ROLE => {
                        let role = params.role.ok_or_else(|| StateError::new("Missing member role"))?;
                        rpc.set_channel_members_role(&params.channel_id, &params.member_ids, role).await
                    }
                    OPERATE_BAN_MEMBERS => {
                        rpc.ban_channel_members(&params.channel_id, &params.member_ids).await
                    }
                    OPERATE_UNBAN_MEMBERS => {
                        rpc.unban_channel_members(&params.channel_id, &params.member_ids).await
                    }
                    OPERATE_REMOVE_MEMBERS => {
                        rpc.remove_channel_members(&params.channel_id, &params.member_ids).await
                    }
                    _ => Err(StateError::new("Unknown channel member operation").into()),
                }
            });
        }
        Event::GetContacts(Body {params: id_opt, result: promise}) => {
            respond(pending, promise, async move {
                /*
                match id {
                    Some(id) => Ok(rpc.get_contact(&id).await?.into_iter().collect()),
                    None => rpc.get_contacts().await,
                }
                */
                rpc.get_contacts(id_opt.as_ref()).await
            });
        }
        Event::UpdateContact(Body {params: contact, result: promise}) => {
            respond(pending, promise, async move {
                rpc.update_contact(&contact).await
            });
        }
        Event::RemoveContacts(Body {params: ids, result: promise}) => {
            respond(pending, promise, async move {
                rpc.remove_contacts(&ids).await
            });
        }
        Event::ClearContacts(Body {params: _, result: promise}) => {
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
