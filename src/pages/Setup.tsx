import { A, useNavigate } from "@solidjs/router";
import { createSignal, For, getOwner, onCleanup, onMount, runWithOwner, Show } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { ChevronLeft, Play, Search } from "~/components/Icons";
import {
  addSource,
  removeSource,
  setSetting,
  syncSource,
  TMDB_KEY_SETTING,
  type ImportReport,
} from "~/lib/api";
import { useSources } from "~/lib/sources";

export default function Setup() {
  const navigate = useNavigate();
  const list = useSources();

  const [url, setUrl] = createSignal("");
  const [tmdb, setTmdb] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [parsed, setParsed] = createSignal(0);
  const [report, setReport] = createSignal<ImportReport>();
  const [error, setError] = createSignal<string>();
  const [lastAction, setLastAction] = createSignal<() => void>();

  onMount(() => {
    // O `listen` é assíncrono e sua continuação retoma fora do owner
    // reativo do Solid, então guardamos o owner antes de esperar e
    // reanexamos o `onCleanup` com `runWithOwner` depois.
    const owner = getOwner();
    // Se `Setup` desmontar antes da promise resolver, o owner já estará
    // descartado quando o handle chegar — `onCleanup` nesse owner nunca
    // rodaria de novo e o listener ficaria vivo para sempre. Por isso
    // marcamos o descarte com uma flag registrada de forma síncrona.
    let disposed = false;
    onCleanup(() => {
      disposed = true;
    });

    // O import roda em Rust e vai avisando quantas entradas já entraram.
    void listen<{ sourceId: number; parsed: number }>(
      "import:progress",
      event => setParsed(event.payload.parsed),
    ).then(stop => {
      if (disposed) {
        // Chegou depois do desmonte: encerra o listener na hora, em vez
        // de tentar reanexar um cleanup que nunca mais vai disparar.
        stop();
        return;
      }
      runWithOwner(owner, () => onCleanup(stop));
    });
  });

  const run = async (task: () => Promise<ImportReport>) => {
    setBusy(true);
    setError(undefined);
    setParsed(0);
    try {
      const result = await task();
      setReport(result);
      await list.refetch();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const addUrl = () => {
    const value = url().trim();
    if (!value) return setError("Cole o endereço da lista M3U.");
    setLastAction(() => addUrl);
    void run(() => addSource(value, "url"));
  };

  const addFile = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "Lista M3U", extensions: ["m3u", "m3u8", "txt"] }],
    });
    if (typeof picked !== "string") return;
    setLastAction(() => addFile);
    void run(() => addSource(picked, "file"));
  };

  const retry = () => lastAction()?.();

  const saveKeyAndGo = async () => {
    const key = tmdb().trim();
    if (key) await setSetting(TMDB_KEY_SETTING, key);
    navigate("/", { replace: true });
  };

  return (
    <main class="relative z-10 mx-auto max-w-2xl px-4 py-10">
      {/* Com lista já importada esta página deixa de ser onboarding e vira
          gerenciamento de fontes: sem esta saída não haveria como voltar,
          porque a barra de navegação fica escondida em /setup. */}
      <Show when={list.sources().length}>
        <A
          href="/"
          class="press mb-6 inline-flex items-center gap-1.5 text-sm text-paper/50 hover:text-amber"
        >
          <ChevronLeft size={16} />
          Voltar ao catálogo
        </A>
      </Show>
      <h1 class="anim-reveal font-display text-3xl font-extrabold tracking-[-0.03em] text-paper">
        Sua lista, seu catálogo
      </h1>
      <p class="anim-reveal mt-2 text-sm text-paper/55" style={{ "--i": 1 }}>
        O YellowTV lê a lista M3U que você já tem e monta um catálogo local de filmes e séries.
        Nada sai do seu computador.
      </p>

      <section class="anim-reveal mt-8" style={{ "--i": 2 }}>
        <label class="flex items-center gap-3 rounded-sm border border-edge bg-panel px-3 py-2.5 focus-within:border-amber">
          <Search size={16} class="shrink-0 text-paper/35" />
          <input
            type="url"
            value={url()}
            onInput={event => setUrl(event.currentTarget.value)}
            onKeyDown={event => event.key === "Enter" && addUrl()}
            placeholder="http://servidor/get.php?username=…&type=m3u_plus"
            class="w-full bg-transparent text-paper outline-none placeholder:text-paper/30"
          />
        </label>
        <div class="mt-3 flex flex-wrap items-center gap-3">
          <button
            type="button"
            onClick={addUrl}
            disabled={busy()}
            class="press flex items-center gap-2 rounded-sm bg-amber px-5 py-2.5 font-medium text-ink hover:bg-paper disabled:opacity-40"
          >
            <Play size={15} />
            {busy() ? "Importando…" : "Importar lista"}
          </button>
          <button
            type="button"
            onClick={addFile}
            disabled={busy()}
            class="wipe press rounded-sm border border-edge px-4 py-2.5 text-sm text-paper/70 hover:border-amber hover:text-amber disabled:opacity-40"
          >
            Escolher arquivo
          </button>
        </div>
      </section>

      <Show when={busy()}>
        <p class="anim-fade mt-4 font-mono text-xs tabular-nums text-amber" aria-live="polite">
          {parsed().toLocaleString("pt-BR")} linhas lidas<span class="anim-caret">_</span>
        </p>
      </Show>

      <Show when={error()}>
        <div class="anim-fade mt-4 rounded-sm border border-live/40 bg-live/10 p-4">
          <p class="text-sm text-live">{error()}</p>
          <button
            type="button"
            onClick={retry}
            class="press mt-2 text-sm text-amber hover:underline"
          >
            Tentar de novo
          </button>
        </div>
      </Show>

      <Show when={report()}>
        {result => (
          <div class="anim-fade mt-4 rounded-sm border border-edge bg-panel/60 p-4">
            <p class="text-sm text-paper/85">
              {(result().movies + result().series + result().channels).toLocaleString("pt-BR")}{" "}
              títulos: {result().movies.toLocaleString("pt-BR")} filmes ·{" "}
              {result().series.toLocaleString("pt-BR")} séries ·{" "}
              {result().channels.toLocaleString("pt-BR")} canais
            </p>
            {/* A lista tem muito mais linhas que títulos: cada episódio é uma
                linha e todos caem na mesma série. Sem dizer isso, o contador
                que correu até 260 mil parece não ter nada a ver com o total. */}
            <p class="mt-1 font-mono text-xs text-paper/40">
              de {result().parsed.toLocaleString("pt-BR")} linhas da lista — episódios da mesma
              série contam como um título
            </p>
            <Show when={result().discarded}>
              <p class="mt-1 font-mono text-xs text-paper/40">
                {result().discarded.toLocaleString("pt-BR")} linhas ignoradas
              </p>
            </Show>
          </div>
        )}
      </Show>

      <Show when={list.sources().length}>
        <section class="mt-8">
          <h2 class="font-display text-sm font-bold text-paper/70">Listas adicionadas</h2>
          <ul class="mt-3 space-y-2">
            <For each={list.sources()}>
              {source => (
                <li class="flex items-center gap-3 rounded-sm border border-edge bg-panel/40 px-3 py-2">
                  <div class="min-w-0">
                    <p class="truncate text-sm text-paper/90">{source.label}</p>
                    <p class="truncate font-mono text-[0.7rem] text-paper/35">
                      {source.itemCount.toLocaleString("pt-BR")} títulos (filmes e séries) ·{" "}
                      {source.url}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => void run(() => syncSource(source.id))}
                    disabled={busy()}
                    class="press ml-auto shrink-0 text-xs text-paper/55 hover:text-amber disabled:opacity-40"
                  >
                    Atualizar
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      await removeSource(source.id);
                      await list.refetch();
                    }}
                    class="press shrink-0 text-xs text-paper/40 hover:text-live"
                  >
                    Remover
                  </button>
                </li>
              )}
            </For>
          </ul>
        </section>
      </Show>

      <section class="mt-10 border-t border-edge pt-6">
        <h2 class="font-display text-sm font-bold text-paper/70">Chave do TMDB (opcional)</h2>
        <p class="mt-1 text-sm text-paper/50">
          Sinopse, nota e elenco vêm do TMDB. Sem chave, o catálogo usa as capas da própria lista.
        </p>
        <input
          type="password"
          value={tmdb()}
          onInput={event => setTmdb(event.currentTarget.value)}
          placeholder="cole a chave v3 ou o token v4"
          class="mt-3 w-full rounded-sm border border-edge bg-panel px-3 py-2.5 text-paper outline-none placeholder:text-paper/30 focus:border-amber"
        />
        <div class="mt-4 flex flex-wrap items-center gap-4">
          <button
            type="button"
            onClick={saveKeyAndGo}
            disabled={!list.sources().length}
            class="press rounded-sm bg-amber px-5 py-2.5 font-medium text-ink hover:bg-paper disabled:opacity-40"
          >
            {tmdb().trim() ? "Salvar e começar" : "Começar"}
          </button>
          <Show when={!list.sources().length}>
            <span class="text-xs text-paper/40">Adicione uma lista para continuar.</span>
          </Show>
        </div>
      </section>
    </main>
  );
}
