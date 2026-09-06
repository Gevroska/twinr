import type { Component } from "solid-js";
import { lazy, Suspense, ErrorBoundary } from "solid-js";
import { Route, Routes } from "@solidjs/router";

import Home from "./Home";
const Stream = lazy(() => import("./Stream"));
const Clips = lazy(() => import("./Clips"));
const Vod = lazy(() => import("./Vod"));
const Favorites = lazy(() => import("./Favorites"));
const Settings = lazy(() => import("./Settings"));
import Nav from "./components/nav";

const NotFound: Component = () => {
  return (
    <>
      <Nav isHome={false} />
      <div class="container max-auto my-auto px-5 py-10">
        <div class="border border-base-200 rounded-lg p-6 mt-3 ml-5">
          <h1 class="font-semibold text-2xl">Page not found</h1>
          <p>Check the URL and try again.</p>
        </div>
      </div>
    </>
  );
};

const App: Component = () => {
  return (
    <ErrorBoundary fallback={<p role="alert" class="p-4">Unable to load this page. Please refresh and try again.</p>}>
    <Suspense fallback={<p role="status" class="p-4">Loading...</p>}>
    <Routes>
      <Route path="/" component={Home} />
      <Route path="/favorites" component={Favorites} />
      <Route path="/settings" component={Settings} />
      <Route path="/:username/clip/:slug" component={Clips} />
      <Route path="/videos/:id" component={Vod} />
      <Route path="/:username" component={Stream} />
      <Route path="*" component={NotFound} />
    </Routes>
    </Suspense>
    </ErrorBoundary>
  );
};

export default App;
