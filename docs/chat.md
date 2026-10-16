# Chat implementation

For the browser protocol, see [Live chat](api.md#live-chat). Browsers connect to Twinr, which opens an anonymous connection to Twitch's IRC WebSocket service at `wss://irc-ws.chat.twitch.tv:443`.

The server requests IRC membership, tags, and commands capabilities, joins the selected channel, and responds to Twitch keepalive messages. It parses IRC tags, display names, badges, Unicode text, action messages, and emote positions into structured JSON. Browser clients do not need to parse raw IRC or provide Twitch credentials.

Chat is read-only. See [the server implementation and tests](../src/chat.rs) and [browser chat component](../front/src/components/streamChat.tsx).

VOD chat uses comment requests synchronized to the browser player's clock; it does not use the live-chat WebSocket.
