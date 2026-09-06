import { Component } from "solid-js";
import FavBtn from "./favCh";
import { FiEye } from "solid-icons/fi";
const LiveMetadata: Component<{title: string; views: number; game: string; avatar: string; username: string}> = props => (
  <div class="mt-1 break-words">
    <div class="flex justify-between gap-2"><h2 class="text-lg font-semibold">{props.title}</h2><span class="inline-flex items-center gap-2"><FiEye />{props.views}</span></div>
    <span class="text-indigo-400 italic">{props.game}</span>
    <div class="watch-channel-row flex items-center gap-2"><img class="w-8 rounded-full" src={props.avatar} /><span>{props.username}</span><FavBtn username={props.username} /></div>
  </div>
);
export default LiveMetadata;
