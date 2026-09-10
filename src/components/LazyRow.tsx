import { createSignal, onCleanup, onMount, Show, type JSX } from "solid-js";

/**
 * Só monta o conteúdo quando a fileira chega perto da viewport.
 *
 * O board devolve dezenas de fileiras de uma vez; montar todas significa
 * centenas de cartões e de `<img>` disputando a primeira pintura. O
 * placeholder tem a mesma altura da fileira real, então nada pula quando o
 * conteúdo entra. Uma vez visível, nunca desmonta: rolar de volta não pode
 * custar um novo carregamento.
 */
export default function LazyRow(props: { children: JSX.Element }) {
  const [visible, setVisible] = createSignal(false);
  let anchor: HTMLDivElement | undefined;

  onMount(() => {
    // Sem IntersectionObserver, mostra tudo: melhor uma página pesada que uma
    // página vazia.
    if (typeof IntersectionObserver === "undefined") {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      entries => {
        if (!entries.some(entry => entry.isIntersecting)) return;
        setVisible(true);
        observer.disconnect();
      },
      // Uma tela inteira de antecedência: a fileira termina de carregar antes
      // de aparecer de fato.
      { rootMargin: "800px 0px" },
    );
    if (anchor) observer.observe(anchor);
    onCleanup(() => observer.disconnect());
  });

  return (
    <Show
      when={visible()}
      fallback={
        <div ref={anchor} class="h-[19rem] sm:h-[20.5rem]" aria-hidden="true">
          <div class="skeleton mb-3 h-3.5 w-40 rounded-sm" />
          <div class="flex gap-4 overflow-hidden">
            {Array.from({ length: 6 }, (_, index) => (
              <div class="w-[38vw] shrink-0 sm:w-44 lg:w-48" data-index={index}>
                <div class="skeleton aspect-[2/3] rounded-sm ring-1 ring-edge/60" />
                <div class="skeleton mt-2 h-3.5 w-4/5 rounded-sm" />
              </div>
            ))}
          </div>
        </div>
      }
    >
      {props.children}
    </Show>
  );
}
