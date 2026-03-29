const chat = document.getElementById('chat'),
    queryParams = `?${
        quality !== 'undefined' && quality.length > 1
            ? `quality=${window.quality}&`
            : 'quality=1280x720&'
    }proxy=${window.proxy ? window.proxy : false}`;

function showPageError(message) {
    const loading = document.querySelector('.loading');
    if (loading) {
        const parent = loading.parentElement;
        if (parent) {
            parent.innerHTML = `<div class="text-error text-center px-4">${message}</div>`;
            return;
        }
    }

    const fallback = document.createElement('div');
    fallback.className = 'text-error text-center px-4 py-4';
    fallback.innerText = message;
    document.body.prepend(fallback);
}

if (Hls.isSupported()) {
    const video = document.getElementById('stream'),
        streamURL = `${window.location.origin}/stream/${username}${queryParams}`,
        hls = new Hls({
            maxBufferLength: 16,
            maxBufferSize: 64 * 1024 * 1024,
            maxMaxBufferLength: 32,
            backBufferLength: 2,
            liveSyncDuration: 2,
            manifestLoadingMaxRetry: Infinity,
            manifestLoadingRetryDelay: 500,
            xhrSetup: (xhr, url) => {
                if (url !== streamURL) {
                    xhr.open(
                        'GET',
                        `${window.location.origin}/stream/urlproxy?url=${url}`
                    );
                } else xhr.open('GET', url);
            },
        }),
        retryStream = () => {
            hls.attachMedia(video);
            hls.loadSource(streamURL);
            hls.startLoad();
        };
    hls.attachMedia(video);
    hls.on(Hls.Events.MEDIA_ATTACHED, function () {
        hls.loadSource(streamURL);
    });
    hls.on(Hls.Events.MANIFEST_PARSED, function () {
        video.play();
    });
    hls.on(Hls.Events.ERROR, function (_event, data) {
        if (data.fatal) {
            switch (data.type) {
                case Hls.ErrorTypes.NETWORK_ERROR:
                    console.log('Network error. Retrying..');
                    retryStream();
                    break;
                case Hls.ErrorTypes.MEDIA_ERROR:
                    console.log('Media error. Retrying..');
                    hls.recoverMediaError();
                    break;
            }
        }
    });
} else {
    showPageError('Your browser do not support HLS.');
}

console.log('Connecting to chat');
const ws = new WebSocket('wss://irc-ws.chat.twitch.tv/');
ws.onopen = () => {
    ws.send('CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership');
    ws.send('PASS SCHMOOPIIE');
    ws.send('NICK justinfan35233');
    ws.send('USER justinfan35233 8 * :justinfan35233');

    ws.send(`JOIN #${username}`);
};
ws.onclose = () => {
    console.log('Websocket closed');

    const warn = document.createElement('div');
    warn.innerText = '⚠ Chat error: Websocket closed';

    chat.appendChild(warn);
};
ws.onmessage = (msg) => {
    const data = msg.data;
    if (!data || typeof data !== 'string') return;
    if (!data.includes('PRIVMSG')) return;

    const nameMatch = data.match(/display-name=(.*?);/g),
        messageMatch = data.match(new RegExp(`${username}\ :(.*?)$`, 'gm')),
        colorMatch = data.match(/#([a-fA-F0-9]{3,6})/);

    if (!nameMatch || !messageMatch) return;

    const name = nameMatch[0].replace(/display-name=/, '').replace(/;/, ''),
        message = messageMatch[0].replace(/.*:/, ''),
        color = colorMatch ? colorMatch[0] : '#fff',
        chatmsg = document.createElement('div'),
        chatuser = document.createElement('span'),
        chatcontent = document.createElement('span');

    chatuser.innerText = name + ': ';
    chatuser.style.color = color || 'white';

    chatcontent.innerText = message;

    chatmsg.appendChild(chatuser);
    chatmsg.appendChild(chatcontent);

    chatmsg.classList = 'mt-1 mb-1';
    chat.appendChild(chatmsg);

    chat.scrollTop = chat.scrollHeight;
};

async function fetchMetadata() {
    const streamer = document.getElementById('streamer'),
        avatar = document.getElementById('avatar'),
        category = document.getElementById('category'),
        title = document.getElementById('title'),
        views = document.getElementById('views');

    try {
        let metadata = await fetch(
            `${window.location.origin}/api/streaminfo/${window.username}`
        );

        if (metadata.status !== 200) {
            showPageError(
                `Unable to load stream details right now (HTTP ${metadata.status}).`
            );
            return false;
        }

        metadata = await metadata.json();

        if (metadata.invalid === true || metadata.valid === false) {
            showPageError('Unable to load stream details right now.');
            return false;
        }

        streamer.innerText = window.username;
        avatar.src = `/stream/urlproxy?url=${metadata.avatar}`;
        category.innerText = metadata.game;
        title.innerText = metadata.title;
        views.innerText = `👤 ${metadata.views}`;

        return true;
    } catch (err) {
        console.error('[Stream] Failed to fetch metadata:', err);
        showPageError('Unable to load stream details right now.');
        return false;
    }
}

(async () => {
    console.log('Fetching metadata');
    await fetchMetadata();
})();

setInterval(async () => {
    console.log('Updating metadata');
    await fetchMetadata();
}, 60000);
