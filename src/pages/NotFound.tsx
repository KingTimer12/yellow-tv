import { A } from "@solidjs/router";
import { ChevronLeft } from "~/components/Icons";

export default function NotFound() {
  return (
    <main class="relative z-10 mx-auto max-w-md px-4 py-24">
      <p class="anim-fade font-mono text-sm text-amber-deep">
        sem sinal<span class="anim-caret">_</span>
      </p>
      {/* The headline slips like a mistuned frame every few seconds. */}
      <h1
        class="anim-reveal anim-glitch mt-2 font-display text-4xl font-extrabold tracking-[-0.03em] text-paper"
        style={{ "--i": 1 }}
      >
        Essa página saiu do ar
      </h1>
      <p class="anim-reveal mt-3 text-paper/60" style={{ "--i": 2 }}>
        O endereço não corresponde a nenhum canal da grade.
      </p>
      <A
        href="/"
        class="press anim-reveal mt-6 inline-flex items-center gap-2 rounded-sm border border-amber px-4 py-2.5 text-sm text-amber hover:bg-amber hover:text-ink"
        style={{ "--i": 3 }}
      >
        <ChevronLeft size={16} />
        Voltar para os canais
      </A>
    </main>
  );
}
