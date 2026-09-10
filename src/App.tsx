import { Navigate, Route, Router } from "@solidjs/router";
import { Show, Suspense } from "solid-js";
import Nav from "~/components/Nav";
import Biblioteca from "~/pages/Biblioteca";
import Channels from "~/pages/Channels";
import Filme from "~/pages/Filme";
import Filmes from "~/pages/Filmes";
import Inicio from "~/pages/Inicio";
import NotFound from "~/pages/NotFound";
import Serie from "~/pages/Serie";
import Series from "~/pages/Series";
import Setup from "~/pages/Setup";
import Watch from "~/pages/Watch";
import { useSources } from "~/lib/sources";
import "./App.css";

/**
 * Sem nenhuma lista adicionada não existe catálogo, então tudo cai em `/setup`
 * até a primeira importação terminar.
 */
function Gate(props: { children?: any }) {
  const list = useSources();
  return (
    <Show when={!list.pending()} fallback={null}>
      <Show when={list.sources().length} fallback={<Navigate href="/setup" />}>
        {props.children}
      </Show>
    </Show>
  );
}

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
      <Route path="/setup" component={Setup} />
      <Route
        path="/"
        component={() => (
          <Gate>
            <Inicio />
          </Gate>
        )}
      />
      <Route path="/filmes" component={Filmes} />
      <Route path="/series" component={Series} />
      <Route path="/biblioteca" component={Biblioteca} />
      <Route path="/canais" component={Channels} />
      <Route path="/filme/:id" component={Filme} />
      <Route path="/serie/:id" component={Serie} />
      <Route path="/watch/:id" component={Watch} />
      <Route path="*" component={NotFound} />
    </Router>
  );
}
