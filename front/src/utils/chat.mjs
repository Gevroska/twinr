// IRC parsing and emote ranges are owned by Rust. This validates the UI boundary.
export function parseChatFrame(frame) {
  try {
    const messages=JSON.parse(frame);
    if (!Array.isArray(messages)) return [];
    return messages.filter(item=>item && typeof item.message==="string" &&
      typeof item["display-name"]==="string" && Array.isArray(item.fragments) &&
      item.fragments.every(f=>f && typeof f.text==="string" &&
        (f.emoteId==null || (typeof f.emoteId==="string" && /^[a-zA-Z0-9_]+$/.test(f.emoteId)))));
  } catch { return []; }
}
