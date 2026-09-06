import { Component, onCleanup, createSignal, For, onMount } from "solid-js";
import { parseChatFrame, ChatMessage as Message } from "../utils/chat.mjs";
import ChatMessage from "./chatMessage";
import ModImg from "../../assets/mod.png";
import SubImg from "../../assets/sub.png";
const StreamChat: Component<{ username: string; scroll: HTMLDivElement }> = props => {
  const [messages, setMessages] = createSignal<Message[]>([]);
  let socket: WebSocket | undefined;
  let retry: number | undefined;
  let disposed = false;
  function connect() {
    if (disposed) return;
    socket = new WebSocket(`${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}`);
    socket.onopen = () => socket?.send(`JOIN ${props.username}`);
    socket.onclose = () => { if (!disposed) retry = window.setTimeout(connect, 1000); };
    socket.onmessage = event => {
      const incoming = parseChatFrame(String(event.data));
      if (!incoming.length) return;
      const follow = props.scroll.scrollHeight - props.scroll.scrollTop - props.scroll.clientHeight < 80;
      setMessages(previous => [...previous, ...incoming].slice(-1000));
      if (follow) props.scroll.scrollTop = props.scroll.scrollHeight;
    };
  }
  onMount(connect);
  onCleanup(() => { disposed = true; clearTimeout(retry); socket?.close(); });
  return <For each={messages()}>{item => <div>
    {item.mod === "1" && <img class="inline-block w-[18px] h-auto m-1" src={ModImg} alt="Moderator" />}
    {item.subscriber === "1" && <img class="inline-block w-[18px] h-auto m-1" src={SubImg} alt="Subscriber" />}
    <span style={{color: item.color || "#fff"}}>{item["display-name"]}</span>: <ChatMessage fragments={item.fragments} message={item.message} />
  </div>}</For>;
};
export default StreamChat;
