import { Component } from 'solid-js';
const DownloadVods: Component<{ id: string; queryString: string; title: string; streamer: string }> = props => (
    <div>
        <p class="text-sm">{props.title} — {props.streamer}</p>
        <p class="text-sm mb-2">Save an MP4. Progress appears in your browser's downloads.</p>
        <a class="btn btn-info btn-sm" href={`/api/vod/${encodeURIComponent(props.id)}/download${props.queryString}`} download="">
            Start download
        </a>
    </div>
);
export default DownloadVods;
