const clipRegex = /(.+)?twitch\.tv\/\w+\/clip\/[\w-]+/,
    streamRegex = /(.+)?twitch\.tv\/(.+)/,
    vodRegex = /(.+)?twitch\.tv\/videos\/(\d+)/,
    twitchDomainRegex = /(.+)?twitch\.tv/;

function resolvePathFromInput(input) {
    const trimmed = input.trim();

    if (trimmed.length < 1) return null;

    // normalize full urls (instance urls and twitch urls)
    if (/^https?:\/\//i.test(trimmed) || trimmed.includes('.')) {
        try {
            const formatted = /^https?:\/\//i.test(trimmed)
                ? trimmed
                : `https://${trimmed}`;
            const parsed = new URL(formatted);
            const normalizedPath = parsed.pathname.replace(/\/+$/, '');

            const vodMatch = normalizedPath.match(/^\/videos\/(\d+)$/i);
            if (vodMatch) return `/videos/${vodMatch[1]}`;

            const clipMatch = normalizedPath.match(
                /^\/([^/]+)\/clip\/([\w-]+)$/i
            );
            if (clipMatch) return `/${clipMatch[1]}/clip/${clipMatch[2]}`;

            const streamerMatch = normalizedPath.match(/^\/([^/]+)$/);
            if (streamerMatch) return `/${streamerMatch[1]}`;
        } catch (err) {
            console.warn('[Search] Unable to parse URL input:', err);
        }
    }

    if (
        trimmed.match(clipRegex) ||
        trimmed.match(streamRegex) ||
        trimmed.match(vodRegex)
    ) {
        return trimmed.replace(twitchDomainRegex, '');
    }

    if (!Number.isNaN(Number(trimmed))) {
        return `/videos/${trimmed}`;
    }

    return `/${trimmed.replace(/^\/+/, '')}`;
}

function _search() {
    const input = document.getElementById('media-txt').value;
    if (input.length == 0) return;

    const resSelect = document.getElementById('media-res'),
        resVal = resSelect.options[resSelect.selectedIndex].text,
        useProxy = document.getElementById('proxy').checked,
        queryParams = new URLSearchParams();

    if (resVal !== 'Resolution') {
        queryParams.set('quality', resVal);
    }

    if (useProxy) {
        queryParams.set('proxy', 'true');
    }

    const targetPath = resolvePathFromInput(input);

    if (!targetPath) return;

    const queryString = queryParams.toString();
    window.location.href = `${targetPath}${queryString.length > 0 ? `?${queryString}` : ''}`;
}
