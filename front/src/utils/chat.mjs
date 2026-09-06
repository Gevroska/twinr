// Twitch supplies exact emote ranges; never replace arbitrary matching words.
export function emoteFragments(message, emotes = "") {
  const characters = Array.from(message);
  const ranges = emotes.split("/").flatMap(group => {
    const [id, positions] = group.split(":");
    return (positions || "").split(",").map(position => {
      const [start, end] = position.split("-").map(Number);
      return { id, start, end };
    });
  }).filter(range => range.id && Number.isInteger(range.start) && Number.isInteger(range.end) && range.start >= 0 && range.end >= range.start && range.end < characters.length).sort((a, b) => a.start - b.start);
  const fragments = [];
  let cursor = 0;
  for (const {id, start, end} of ranges) {
    if (start < cursor) continue;
    if (start > cursor) fragments.push({text: characters.slice(cursor, start).join("")});
    fragments.push({text: characters.slice(start, end + 1).join(""), emoteId: id});
    cursor = end + 1;
  }
  if (cursor < characters.length) fragments.push({text: characters.slice(cursor).join("")});
  return fragments;
}
export function parseChatFrame(frame) {
  return frame.split(/\r?\n/).flatMap(line => {
    const match = line.match(/^@([^ ]+) :[^ ]+ PRIVMSG #[^ ]+ :(.*)$/);
    if (!match) return [];
    const tags = Object.fromEntries(match[1].split(";").map(pair => {
      const separator = pair.indexOf("=");
      const key = separator < 0 ? pair : pair.slice(0, separator);
      const value = separator < 0 ? "" : pair.slice(separator + 1);
      return [key, value.replace(/\\([s:nr\\])/g, (_, c) => ({s:" ", ":":";", n:"\n", r:"\r", "\\":"\\"})[c])];
    }));
    const message = match[2].replace(/^\x01ACTION (.*)\x01$/, "$1");
    return [{...tags, message, fragments: emoteFragments(message, tags.emotes)}];
  });
}
