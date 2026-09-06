import { Component, For, Show } from "solid-js";
export interface ChatFragment { text: string; emoteId?: string | null; }
const ChatMessage: Component<{fragments?: ChatFragment[]; message: string}> = props => (
  <span><For each={props.fragments?.length ? props.fragments : [{text: props.message}]}>{fragment => (
    <Show when={fragment.emoteId} fallback={fragment.text}>
      <img class="chat-emote" alt={fragment.text} title={fragment.text} src={`/api/proxy?url=${encodeURIComponent(btoa(`https://static-cdn.jtvnw.net/emoticons/v2/${fragment.emoteId}/default/dark/2.0`))}`} />
    </Show>
  )}</For></span>
);
export default ChatMessage;
