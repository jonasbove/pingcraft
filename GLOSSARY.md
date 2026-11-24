# PingCraft

PingCraft puts all of a person's chats, from every network they connect, into one inbox, on a server they run
themselves. These are the words the code, the interface and the docs use.

## People and what they connect

**User**:
A person signed in to a PingCraft server. A server can have several users, each added by its admin.
_Avoid_: account (for the person), member, profile

**Admin**:
The user who runs a PingCraft server and adds the other users.
_Avoid_: owner, operator

**Network**:
A chat service a user can connect, such as WhatsApp, Signal, Telegram or Discord. Matrix itself is also a network.
_Avoid_: service, platform, protocol, integration

**Account**:
One login on one network. A user can connect several accounts, including two on the same network.
_Avoid_: connection, login, identity, profile

## Conversations

**Chat**:
One conversation as seen through one account. A group that two of a user's accounts are both in is two chats.
_Avoid_: room, conversation, thread, channel

**Inbox**:
All of a user's chats from all their accounts, together in one list.
_Avoid_: unified inbox, feed, chat list

**Message**:
One thing sent in a chat: text, media, or a reaction to another message.
_Avoid_: event, post

## Server side

**Bridge**:
The server program that connects one network, carrying messages between it and the user's chats.
_Avoid_: connector, puppet, plugin

**Bridge provider**:
The family a bridge comes from, such as mautrix.
_Avoid_: bridge vendor, backend

**Risk level**:
How likely a network is to restrict or ban an account for being connected through a bridge, shown to the user
before they connect it.
_Avoid_: warning level, safety rating
