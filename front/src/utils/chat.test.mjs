import {test} from "node:test";
import assert from "node:assert/strict";
import {parseChatFrame} from "./chat.mjs";
test("structured messages preserve Unicode, emotes and literal text",()=>{
 const messages=[{"display-name":"Alice",mod:"1",subscriber:"1",message:"😀 Kappa <script>",fragments:[{text:"😀 "},{text:"Kappa",emoteId:"25"},{text:" <script>"}]}];
 assert.deepEqual(parseChatFrame(JSON.stringify(messages)),messages);
});
test("ignores acknowledgements and invalid message payloads",()=>{
 for(const frame of ["OK","PING :tmi.twitch.tv",'{}','null','[{"message":"x"}]','[null]']) assert.deepEqual(parseChatFrame(frame),[]);
});
