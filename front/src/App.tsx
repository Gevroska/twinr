import type { Component } from "solid-js";
import { Route, Routes } from "@solidjs/router";

import Home from "./Home";
import Stream from "./Stream";
import Clips from "./Clips";
import Vod from "./Vod";
import Favorites from "./Favorites";
import Settings from "./Settings";
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
    <Routes>
      <Route path="/" component={Home} />
      <Route path="/favorites" component={Favorites} />
      <Route path="/settings" component={Settings} />
      <Route path="/:username/clip/:slug" component={Clips} />
      <Route path="/videos/:id" component={Vod} />
      <Route path="/:username" component={Stream} />
      <Route path="*" component={NotFound} />
    </Routes>
  );
};

export default App;
