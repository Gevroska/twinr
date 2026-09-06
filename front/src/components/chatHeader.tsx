import { Component } from "solid-js";
import { FiMenu } from "solid-icons/fi";
const ChatHeader: Component<{expanded: boolean; toggle: () => void}> = props => (
  <header class="watch-chat-heading">
    <h2>Chat</h2>
    <button class="md:hidden" aria-label={props.expanded ? "Hide navigation" : "Show navigation"} aria-expanded={props.expanded} aria-controls="watch-mobile-navigation" onclick={props.toggle}><FiMenu /></button>
  </header>
);
export default ChatHeader;
