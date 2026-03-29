import axios from 'axios';
import { Router, Request, Response } from 'express';

export default {
    append(app: Router, userAgent: string, clientId: string) {
        app.get('/proxy', async (req: Request, res: Response) => {
            try {
                const { url } = req.query;

                if (!url) {
                    res.status(400).send('No url provided.');
                    return;
                }

                const encodedUrl = decodeURIComponent(String(url)).replace(
                        / /g,
                        '+'
                    ),
                    decodedUrl = Buffer.from(encodedUrl, 'base64').toString(
                        'utf8'
                    ),
                    urlReq = await axios.get(decodedUrl, {
                        responseType: 'stream',
                        headers: {
                            'User-Agent': userAgent,
                            Referer: 'https://www.twitch.tv',
                            Origin: 'https://www.twitch.tv',
                            'Client-ID': clientId,
                        },
                        validateStatus: () => true,
                    });

                if (urlReq.status !== 200) {
                    res.setHeader(
                        'Content-Type',
                        String(
                            urlReq.headers['content-type'] ||
                                urlReq.headers['Content-Type']
                        )
                    );
                    res.status(urlReq.status).send('Error fetching resource');
                    return;
                }
                if (
                    urlReq.headers['Cache-Control'] !== undefined ||
                    urlReq.headers['cache-control'] !== undefined
                ) {
                    res.setHeader(
                        'Cache-Control',
                        urlReq.headers['Cache-Control'] ||
                            urlReq.headers['cache-control']
                    );
                }
                res.setHeader(
                    'Content-Type',
                    String(
                        urlReq.headers['content-type'] ||
                            urlReq.headers['Content-Type']
                    )
                );
                res.status(urlReq.status);
                urlReq.data.pipe(res);
            } catch (err) {
                res.status(500).json({
                    error: true,
                    message: err.message,
                });
            }
        });
    },
};
