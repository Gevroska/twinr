import type { Component } from 'solid-js';

import Nav from './components/nav';

const Settings: Component = () => {
    return (
        <>
            <Nav isHome={false} />
            <title>Twinr - Settings</title>
            <div class="hero min-h-[80vh] px-4">
                <div class="hero-content w-full max-w-2xl text-center">
                    <div class="card w-full border border-base-200 bg-base-100/90 shadow-xl backdrop-blur">
                        <div class="card-body gap-4">
                            <h1 class="text-3xl font-bold">Settings</h1>
                            <p class="text-sm text-base-content/70">
                                This page no longer resolves as a channel route,
                                so opening <code>/settings</code> will not trigger
                                streamer API calls.
                            </p>
                            <p class="text-sm text-base-content/70">
                                More settings controls can be added here.
                            </p>
                        </div>
                    </div>
                </div>
            </div>
        </>
    );
};

export default Settings;
