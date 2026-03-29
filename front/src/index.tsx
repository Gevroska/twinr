/* @refresh reload */
import { render } from 'solid-js/web';
import { Router } from '@solidjs/router';
import h from 'solid-js/h';

import './index.css';
import App from './App';

// Fallback for environments/bundlers that accidentally emit React classic JSX calls.
const globalWithReact = globalThis as typeof globalThis & {
    React?: {
        createElement: typeof h;
        Fragment: typeof h.Fragment;
    };
};

if (!globalWithReact.React) {
    globalWithReact.React = {
        createElement: h,
        Fragment: h.Fragment,
    };
}

const root = document.getElementById('root');

render(
    () => (
        <Router>
            <App />
        </Router>
    ),
    root!
);
