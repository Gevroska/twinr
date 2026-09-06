use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
#[derive(Deserialize)]
pub(crate) struct QualityQuery {
    pub quality: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum QualityPreference {
    Auto,
    Height(u32),
    AudioOnly,
    AudioOpus(u32),
}

pub(crate) fn parse_quality_preference(value: Option<&str>) -> QualityPreference {
    match value {
        Some(v) if v.eq_ignore_ascii_case("audio_only") => QualityPreference::AudioOnly,
        Some(v) if v.starts_with("audio_opus_") => v
            .trim_start_matches("audio_opus_")
            .parse::<u32>()
            .map(QualityPreference::AudioOpus)
            .unwrap_or(QualityPreference::Auto),
        Some(v) => v
            .parse::<u32>()
            .map(QualityPreference::Height)
            .unwrap_or(QualityPreference::Auto),
        None => QualityPreference::Auto,
    }
}
pub(crate) fn manifest_proxy_url(source: &str, reference: &str, playlist: bool) -> String {
    format!(
        "/api/{}?url={}",
        if playlist { "playlist" } else { "proxy" },
        urlencoding::encode(&STANDARD.encode(resolve_playlist_url(source, reference)))
    )
}

pub(crate) fn proxy_vod_manifest(source: &str, manifest: &str) -> String {
    rewrite_with(manifest, |r, p| manifest_proxy_url(source, r, p))
}

pub(crate) fn select_playlist(manifest: &str, quality: &QualityPreference) -> Option<String> {
    let lines: Vec<&str> = manifest.lines().map(str::trim).collect();
    let variants: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("#EXT-X-STREAM-INF:"))
        .filter_map(|(i, line)| {
            lines[i + 1..]
                .iter()
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|uri| (*line, *uri))
        })
        .collect();
    match quality {
        QualityPreference::Height(height) => {
            if let Some((_, uri)) = variants.iter().find(|(line, _)| {
                attribute(line, "RESOLUTION")
                    .and_then(|r| r.rsplit_once('x').and_then(|(_, h)| h.parse::<u32>().ok()))
                    == Some(*height)
            }) {
                return Some(uri.to_string());
            }
        }
        QualityPreference::AudioOnly | QualityPreference::AudioOpus(_) => {
            if let Some(uri) = lines
                .iter()
                .filter(|l| {
                    l.starts_with("#EXT-X-MEDIA:")
                        && attribute(l, "TYPE").as_deref() == Some("AUDIO")
                })
                .find_map(|l| attribute(l, "URI").filter(|s| !s.is_empty()))
            {
                return Some(uri);
            }
            if let Some((_, uri)) = variants.iter().find(|(line, _)| {
                let codecs = attribute(line, "CODECS").unwrap_or_default().to_lowercase();
                let no_video = !["avc", "hvc", "hev", "av01", "vp09", "vp8", "theora"]
                    .iter()
                    .any(|c| codecs.contains(c));
                let no_resolution = attribute(line, "RESOLUTION").is_none_or(|r| r == "0x0");
                no_video
                    && no_resolution
                    && (codecs.contains("mp4a")
                        || codecs.contains("opus")
                        || attribute(line, "VIDEO").as_deref() == Some("audio_only"))
            }) {
                return Some(uri.to_string());
            }
        }
        QualityPreference::Auto => {}
    }
    variants.first().map(|(_, uri)| uri.to_string())
}

pub(crate) fn resolve_playlist_url(source_url: &str, selected_ref: &str) -> String {
    if selected_ref.is_empty() {
        return String::new();
    }
    reqwest::Url::parse(source_url)
        .and_then(|base| base.join(selected_ref))
        .map(|url| url.to_string())
        .unwrap_or_default()
}

pub(crate) fn attribute(line: &str, key: &str) -> Option<String> {
    let text = line.split_once(':')?.1;
    let mut quoted = false;
    let mut start = 0;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ',')))
    {
        if c == '"' {
            quoted = !quoted;
        }
        if c == ',' && !quoted {
            if let Some((k, v)) = text[start..i].split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
            start = i + 1;
        }
    }
    None
}
pub(crate) fn rewrite_with(
    manifest: &str,
    mut rewrite: impl FnMut(&str, bool) -> String,
) -> String {
    let master = manifest.contains("#EXT-X-STREAM-INF:");
    manifest
        .trim_start_matches('\u{feff}')
        .lines()
        .map(|raw| {
            let line = raw.trim();
            if !line.starts_with('#') && !line.is_empty() {
                return rewrite(line, master);
            }
            let mut output = line.to_string();
            let mut offset = 0;
            while let Some(found) = output[offset..].find("URI=") {
                let value_start = offset + found + 4;
                let quoted = output[value_start..].starts_with('"');
                let start = value_start + usize::from(quoted);
                let length = if quoted {
                    output[start..].find('"').unwrap_or(output.len() - start)
                } else {
                    output[start..].find(',').unwrap_or(output.len() - start)
                };
                let playlist = line.starts_with("#EXT-X-MEDIA:")
                    || line.starts_with("#EXT-X-I-FRAME-STREAM-INF:")
                    || line.starts_with("#EXT-X-RENDITION-REPORT:");
                let replacement = rewrite(&output[start..start + length], playlist);
                output.replace_range(start..start + length, &replacement);
                offset = (start + replacement.len() + usize::from(quoted)).min(output.len());
            }
            output
        })
        .collect::<Vec<_>>()
        .join("\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_manual_and_adaptive_quality() {
        assert_eq!(parse_quality_preference(None), QualityPreference::Auto);
        assert_eq!(
            parse_quality_preference(Some("auto")),
            QualityPreference::Auto
        );
        assert_eq!(
            parse_quality_preference(Some("720")),
            QualityPreference::Height(720)
        );
        assert_eq!(
            parse_quality_preference(Some("audio_opus_64")),
            QualityPreference::AudioOpus(64)
        );
        assert_eq!(
            parse_quality_preference(Some("audio_only")),
            QualityPreference::AudioOnly
        );
    }
    #[test]
    fn handles_comments_audio_groups_and_byte_ranges() {
        let master="#EXTM3U\n#EXT-X-STREAM-INF:RESOLUTION=1280x720,CODECS=\"avc1,mp4a\"\n\n#comment\nvideo.m3u8\n#EXT-X-STREAM-INF:VIDEO=\"audio_only\",RESOLUTION=0x0\n#comment\naudio.m3u8";
        assert_eq!(
            select_playlist(master, &QualityPreference::AudioOpus(32)).as_deref(),
            Some("audio.m3u8")
        );
        assert_eq!(
            select_playlist(master, &QualityPreference::Height(720)).as_deref(),
            Some("video.m3u8")
        );
        let m="#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=AES-128,URI=key\n#EXT-X-MAP:URI=\"init\",BYTERANGE=\"10@0\"\n#EXT-X-BYTERANGE:12@10\nsegment\n#EXT-X-ENDLIST";
        let result = rewrite_with(m, |uri, _| format!("safe/{uri}"));
        assert!(result.contains("URI=safe/key"));
        assert!(result.contains("BYTERANGE=\"10@0\""));
        assert!(result.contains("#EXT-X-BYTERANGE:12@10"));
    }
    const MASTER:&str="#EXTM3U\n#EXT-X-STREAM-INF:CODECS=\"avc1,mp4a.40.2\",RESOLUTION=1920x1080\nvideo.m3u8\n#EXT-X-STREAM-INF:CODECS=\"mp4a.40.2\"\naudio.m3u8";
    #[test]
    fn opus_selects_audio_live_and_vod() {
        for q in [
            QualityPreference::AudioOpus(32),
            QualityPreference::AudioOnly,
        ] {
            assert_eq!(select_playlist(MASTER, &q).as_deref(), Some("audio.m3u8"));
        }
        assert_eq!(
            select_playlist(MASTER, &QualityPreference::Auto).as_deref(),
            Some("video.m3u8")
        );
        let video = "#EXTM3U\n#EXT-X-STREAM-INF:CODECS=\"avc1,mp4a.40.2\"\nvideo.m3u8";
        assert_eq!(
            select_playlist(video, &QualityPreference::AudioOpus(64)).as_deref(),
            Some("video.m3u8")
        );
        let alt = format!("#EXT-X-MEDIA:TYPE=AUDIO,URI=\"alt.m3u8\"\n{video}");
        assert_eq!(
            select_playlist(&alt, &QualityPreference::AudioOpus(64)).as_deref(),
            Some("alt.m3u8")
        );
    }
    #[test]
    fn resolves_muted_segments_keys_maps_and_nested_playlists() {
        let source = "https://cdn.ttvnw.net/vod/chunked/index-muted-AC62XD2A6L.m3u8";
        let media="#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"../key?x=1,y=2\"\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXT-X-TWITCH-UNKNOWN:ABC=123\n#EXTINF:10,\n0-muted.ts\n#EXT-X-ENDLIST";
        let out = rewrite_with(media, |r, p| {
            assert!(!p);
            resolve_playlist_url(source, r)
        });
        assert!(out.contains("https://cdn.ttvnw.net/vod/key?x=1,y=2"));
        assert!(out.contains("https://cdn.ttvnw.net/vod/chunked/init.mp4"));
        assert!(out.contains("0-muted.ts"));
        assert!(out.contains("#EXT-X-TWITCH-UNKNOWN:ABC=123"));
        let master = proxy_vod_manifest(source, MASTER);
        assert_eq!(master.matches("/api/playlist?url=").count(), 2);
        assert_eq!(
            resolve_playlist_url(source, "//other.ttvnw.net/x"),
            "https://other.ttvnw.net/x"
        );
    }
}
