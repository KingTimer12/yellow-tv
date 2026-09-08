import { Route, Router } from "@solidjs/router";
import { Suspense } from "solid-js";
import Nav from "~/components/Nav";
import Channels from "~/pages/Channels";
import Filme from "~/pages/Filme";
import Filmes from "~/pages/Filmes";
import NotFound from "~/pages/NotFound";
import Serie from "~/pages/Serie";
import Series from "~/pages/Series";
import Watch from "~/pages/Watch";
import "./App.css";

export default function App() {
  return (
    <Router
      root={props => (
        <>
          <Nav />
          <Suspense>{props.children}</Suspense>
        </>
      )}
    >
      <Route path="/" component={Channels} />
      <Route path="/watch/:id" component={Watch} />
      <Route path="/filmes" component={Filmes} />
      <Route path="/series" component={Series} />
      <Route path="/filme/:id" component={Filme} />
      <Route path="/serie/:id" component={Serie} />
      <Route path="*" component={NotFound} />
    </Router>
  );
}
