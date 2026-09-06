import {test} from "node:test";
import assert from "node:assert/strict";
import {parseChatFrame, emoteFragments} from "./chat.mjs";
test("preserves complete IRC text and multiple messages with native emote metadata", () => {
 const frame = "@display-name=Alice;emotes=25:3-7,9-13;color=#fff :alice!a@a PRIVMSG #test :Hi Kappa Kappa; x=y :yes\r\n@display-name=Bob;emotes=emotesv2_123:0-4 :b!b@b PRIVMSG #test :Dance\r\nPING :tmi.twitch.tv\r\n";
 const messages = parseChatFrame(frame);
 assert.equal(messages.length, 2);
 assert.deepEqual(messages[0].fragments, [{text:"Hi "}, {text:"Kappa",emoteId:"25"}, {text:" "}, {text:"Kappa",emoteId:"25"}, {text:"; x=y :yes"}]);
 assert.equal(messages[1].fragments[0].emoteId, "emotesv2_123");
});
test("Unicode and malformed ranges do not corrupt text or match arbitrary words", () => {
 assert.deepEqual(emoteFragments("😀 Kappa", "25:2-6"), [{text:"😀 "}, {text:"Kappa", emoteId:"25"}]);
 assert.deepEqual(emoteFragments("Kappa xKappa", "25:0-4/1:99-100/2:3-6"), [{text:"Kappa", emoteId:"25"}, {text:" xKappa"}]);
 assert.deepEqual(emoteFragments("<script>x</script>"), [{text:"<script>x</script>"}]);
});
