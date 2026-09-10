import { Navigate, Route, Router, useLocation } from "@solidjs/router";
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
 * até a primeira importação terminar. Toda rota fora de `/setup` passa por
 * aqui — só `/setup` fica de fora do portão.
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

/** A barra de navegação some durante o onboarding: nada em `/setup` deveria levar a rotas com catálogo vazio. */
function Shell(props: { children?: any }) {
  const location = useLocation();
  return (
    <>
      <Show when={location.pathname !== "/setup"}>
        <Nav />
      </Show>
      <Suspense>{props.children}</Suspense>
    </>
  );
}

export default function App() {
  return (
    <Router root={Shell}>
      <Route path="/setup" component={Setup} />
      <Route
        path="/"
        component={() => (
          <Gate>
            <Inicio />
          </Gate>
        )}
      />
      <Route
        path="/filmes"
        component={() => (
          <Gate>
            <Filmes />
          </Gate>
        )}
      />
      <Route
        path="/series"
        component={() => (
          <Gate>
            <Series />
          </Gate>
        )}
      />
      <Route
        path="/biblioteca"
        component={() => (
          <Gate>
            <Biblioteca />
          </Gate>
        )}
      />
      <Route
        path="/canais"
        component={() => (
          <Gate>
            <Channels />
          </Gate>
        )}
      />
      <Route
        path="/filme/:id"
        component={() => (
          <Gate>
            <Filme />
          </Gate>
        )}
      />
      <Route
        path="/serie/:id"
        component={() => (
          <Gate>
            <Serie />
          </Gate>
        )}
      />
      <Route
        path="/watch/:id"
        component={() => (
          <Gate>
            <Watch />
          </Gate>
        )}
      />
      <Route path="*" component={NotFound} />
    </Router>
  );
}
