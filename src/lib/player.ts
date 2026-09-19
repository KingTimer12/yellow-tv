/** Utilitários do player. Sem estado e sem DOM, para poder ser lido de cabeça. */

/** 3725 → "1:02:05"; 65 → "1:05". A hora só aparece quando existe. */
export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const total = Math.floor(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const pad = (value: number) => String(value).padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(secs)}` : `${minutes}:${pad(secs)}`;
}

/**
 * Quanto do vídeo já foi bufferizado à frente da posição atual, de 0 a 1.
 *
 * `buffered` é uma lista de intervalos, não um número: depois de arrastar a
 * barra várias vezes existem vários pedaços soltos. Só interessa o intervalo
 * que contém a posição atual — é dele que a reprodução depende agora.
 */
export function bufferedAhead(video: HTMLVideoElement): number {
  const duration = video.duration;
  if (!Number.isFinite(duration) || duration <= 0) return 0;
  for (let index = 0; index < video.buffered.length; index += 1) {
    const start = video.buffered.start(index);
    const end = video.buffered.end(index);
    if (start <= video.currentTime && video.currentTime <= end) {
      return Math.min(end / duration, 1);
    }
  }
  return 0;
}

/** Posição na barra (0 a 1) a partir do clique/arraste. */
export function ratioFromPointer(element: HTMLElement, clientX: number): number {
  const box = element.getBoundingClientRect();
  if (box.width <= 0) return 0;
  return Math.min(Math.max((clientX - box.left) / box.width, 0), 1);
}

export const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2] as const;

/** Rótulo curto para a lista de fontes: "Legendado · 1080P · Minha lista". */
export function streamLabel(stream: {
  variant: string | null;
  quality: string | null;
  sourceLabel: string;
}): string {
  return [stream.variant, stream.quality, stream.sourceLabel].filter(Boolean).join(" · ");
}
