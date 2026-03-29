const video = document.getElementById('stream'),
    chat = document.getElementById('chat');

const queryParams = `?${
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
    const hls = new Hls({
            backBufferLength: 9,
            liveSyncDuration: 9,
            manifestLoadingMaxRetry: Infinity,
            manifestLoadingRetryDelay: 500,
            xhrSetup: (xhr, url) => {
                if (url.endsWith('.ts')) {
                    const parsedURl =
                        url.replace('vod/', `vod/${id}/`) + queryParams;
                    xhr.open('GET', parsedURl);
                } else xhr.open('GET', url);
            },
        }),
        streamURL = `/vod/${id}${queryParams}`,
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
    // based on https://github.com/videojs/video.js/issues/3725#issuecomment-410523448
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

(async () => {
    console.log('Fetching metadata');

    const streamer = document.getElementById('streamer'),
        avatar = document.getElementById('avatar'),
        category = document.getElementById('category'),
        title = document.getElementById('title');

    try {
        let metadata = await fetch(
            `${window.location.origin}/api/vodinfo/${window.id}`
        );

        if (metadata.status !== 200) {
            showPageError(
                `Unable to load VOD details right now (HTTP ${metadata.status}).`
            );
            return;
        }

        metadata = await metadata.json();

        if (metadata.valid === false || metadata.invalid === true) {
            showPageError('Unable to load VOD details right now.');
            return;
        }

        streamer.innerText = metadata.username;
        avatar.src = `/stream/urlproxy?url=${metadata.avatar}`;
        category.innerText = metadata.game;
        title.innerText = metadata.title;

        document.title = `Twinr - VOD ${metadata.title}`;
    } catch (err) {
        console.error('[VOD] Failed to fetch metadata:', err);
        showPageError('Unable to load VOD details right now.');
    }
})();

// chat
function LogError(message = '') {
    const warn = document.createElement('div');
    warn.innerText = `⚠ Chat error: ${message}`;
    chat.appendChild(warn);
}

async function fetchComments(offset = 0) {
    try {
        let req = await fetch(`/api/vodinfo/comments/${window.id}/${offset}`);

        if (req.status !== 200) {
            LogError(`Failed to fetch messages. Status ${req.status}`);
            return false;
        }

        req = await req.json();

        if (req.valid == false) {
            LogError(`Failed to fetch messages. Invalid API call.`);
            return false;
        }

        return req.data;
    } catch (err) {
        console.error('[VOD] Failed to fetch comments:', err);
        LogError('Failed to fetch messages due to network error.');
        return false;
    }
}

async function startChat(offset = 0) {
    console.log(`Fetching comments with offset ${offset}`);

    let commentsStart = 0,
        commentsEnd = 0,
        latestItem,
        comments = await fetchComments(offset);

    if (comments == false) {
        // Retry after 1s
        setTimeout(() => startChat(offset), 1000);
        return;
    }

    if (!Array.isArray(comments) || comments.length < 1) {
        LogError('No comments available for this VOD yet.');
        return;
    }

    commentsStart = comments[0].offset;
    commentsEnd = comments[comments.length - 1].offset;

    console.log(
        `Chat info\nInit offset: ${commentsStart}\nEnd offset: ${commentsEnd}`
    );

    function playbackListener() {
        const time = Math.round(video.currentTime);

        if (latestItem == time || time < commentsStart) return;

        latestItem = time;

        // load more comments
        if (time == commentsEnd || time > commentsEnd) {
            video.removeEventListener('timeupdate', playbackListener);
            startChat(time > commentsEnd ? time : commentsEnd);
            return;
        }

        const selectedComments = comments.filter((x) => x.offset == time);

        if (selectedComments.length < 1) return;

        selectedComments.forEach((selectedComment) => {
            const chatmsg = document.createElement('div'),
                chatuser = document.createElement('span'),
                chatcontent = document.createElement('span');

            chatuser.innerText = selectedComment.username + ': ';
            chatuser.style.color = selectedComment.color || 'white';

            chatcontent.innerText = selectedComment.message;

            // append message
            chatmsg.appendChild(chatuser);
            chatmsg.appendChild(chatcontent);

            chatmsg.classList = 'mt-1 mb-1';
            chat.appendChild(chatmsg);
        });

        chat.scrollTop = chat.scrollHeight;
    }

    video.addEventListener('timeupdate', playbackListener);
}

startChat();
