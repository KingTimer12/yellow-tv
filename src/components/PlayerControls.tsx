import { createSignal, For, Show, type JSX } from "solid-js";
import {
  Back10,
  Forward10,
  Fullscreen,
  FullscreenExit,
  Gauge,
  Pause,
  Pip,
  Play,
  Subtitles,
  Volume,
  VolumeOff,
} from "~/components/Icons";
import { formatTime, ratioFromPointer, SPEEDS, streamLabel } from "~/lib/player";
import type { StreamRef } from "~/lib/api";

export type SubtitleTrack = { id: string; label: string };

/** Botão que abre uma lista. Avisa o pai para os controles não sumirem com o menu aberto. */
function Menu(props: {
  icon: JSX.Element;
  label: string;
  active?: boolean;
  onOpenChange: (open: boolean) => void;
  children: (close: () => void) => JSX.Element;
}) {
  const [open, setOpen] = createSignal(false);
  const toggle = (value: boolean) => {
    setOpen(value);
    props.onOpenChange(value);
  };

  return (
    <div class="relative">
      <button
        type="button"
        onClick={() => toggle(!open())}
        onBlur={event => {
          // Só fecha quando o foco sai do menu inteiro, senão clicar num item
          // fecha antes do clique registrar.
          if (!event.relatedTarget || !event.currentTarget.parentElement?.contains(event.relatedTarget as Node)) {
            toggle(false);
          }
        }}
        class="press grid size-9 place-content-center rounded-sm text-paper/70 hover:bg-paper/10 hover:text-amber"
        classList={{ "text-amber": props.active || open() }}
        aria-label={props.label}
        aria-haspopup="true"
        aria-expanded={open()}
      >
        {props.icon}
      </button>
      <Show when={open()}>
        <div class="anim-fade absolute bottom-11 right-0 z-10 min-w-44 rounded-sm border border-edge bg-ink/95 p-1 shadow-[var(--shadow-lift)] backdrop-blur-md">
          {props.children(() => toggle(false))}
        </div>
      </Show>
    </div>
  );
}

function MenuItem(props: { active?: boolean; onClick: () => void; children: JSX.Element }) {
  return (
    <button
      type="button"
      onClick={props.onClick}
      class="press block w-full truncate rounded-sm px-3 py-1.5 text-left text-sm text-paper/70 hover:bg-paper/10 hover:text-amber"
      classList={{ "text-amber": props.active }}
    >
      {props.children}
    </button>
  );
}

export default function PlayerControls(props: {
  playing: boolean;
  currentTime: number;
  duration: number;
  /** 0 a 1. */
  buffered: number;
  volume: number;
  muted: boolean;
  speed: number;
  fullscreen: boolean;
  pipAvailable: boolean;
  visible: boolean;
  streams: StreamRef[];
  activeStream: number;
  subtitles: SubtitleTrack[];
  activeSubtitle: string | null;
  onTogglePlay: () => void;
  onSeek: (seconds: number) => void;
  onSkip: (delta: number) => void;
  onVolume: (value: number) => void;
  onToggleMute: () => void;
  onSpeed: (value: number) => void;
  onToggleFullscreen: () => void;
  onPip: () => void;
  onPickStream: (index: number) => void;
  onPickSubtitle: (id: string | null) => void;
  onLoadSubtitle: () => void;
  onMenuOpen: (open: boolean) => void;
}) {
  // Enquanto a pessoa arrasta, a barra mostra a posição do dedo, não a do
  // vídeo: seguir o vídeo faria o marcador escapar de baixo do cursor.
  const [scrub, setScrub] = createSignal<number>();
  let bar: HTMLDivElement | undefined;

  const ratio = () => {
    const dragging = scrub();
    if (dragging !== undefined) return dragging;
    if (!Number.isFinite(props.duration) || props.duration <= 0) return 0;
    return Math.min(props.currentTime / props.duration, 1);
  };

  const startScrub = (event: PointerEvent) => {
    if (!bar) return;
    event.currentTarget instanceof HTMLElement && event.currentTarget.setPointerCapture(event.pointerId);
    setScrub(ratioFromPointer(bar, event.clientX));
  };
  const moveScrub = (event: PointerEvent) => {
    if (scrub() === undefined || !bar) return;
    setScrub(ratioFromPointer(bar, event.clientX));
  };
  const endScrub = () => {
    const value = scrub();
    setScrub(undefined);
    if (value === undefined || !Number.isFinite(props.duration)) return;
    props.onSeek(value * props.duration);
  };

  return (
    <div
      class="absolute inset-x-0 bottom-0 z-10 bg-gradient-to-t from-ink via-ink/85 to-transparent px-3 pb-2 pt-10 transition-opacity duration-200"
      classList={{ "opacity-0 pointer-events-none": !props.visible }}
      // O clique nos controles não pode chegar ao vídeo e pausá-lo.
      onClick={event => event.stopPropagation()}
    >
      <div
        ref={bar}
        role="slider"
        tabindex="0"
        aria-label="Posição"
        aria-valuemin={0}
        aria-valuemax={Math.round(props.duration) || 0}
        aria-valuenow={Math.round(props.currentTime)}
        aria-valuetext={formatTime(props.currentTime)}
        onPointerDown={startScrub}
        onPointerMove={moveScrub}
        onPointerUp={endScrub}
        onPointerCancel={endScrub}
        class="group relative h-6 cursor-pointer touch-none select-none"
      >
        <div class="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 rounded-full bg-paper/20">
          <div
            class="absolute inset-y-0 left-0 rounded-full bg-paper/25"
            style={{ width: `${props.buffered * 100}%` }}
          />
          <div
            class="absolute inset-y-0 left-0 rounded-full bg-amber"
            style={{ width: `${ratio() * 100}%` }}
          />
          <span
            class="absolute top-1/2 size-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-amber opacity-0 transition-opacity group-hover:opacity-100"
            classList={{ "opacity-100": scrub() !== undefined }}
            style={{ left: `${ratio() * 100}%` }}
          />
        </div>
      </div>

      <div class="flex items-center gap-1">
        <button
          type="button"
          onClick={props.onTogglePlay}
          class="press grid size-9 place-content-center rounded-sm text-paper hover:bg-paper/10 hover:text-amber"
          aria-label={props.playing ? "Pausar" : "Reproduzir"}
        >
          {props.playing ? <Pause size={18} /> : <Play size={18} />}
        </button>

        <button
          type="button"
          onClick={() => props.onSkip(-10)}
          class="press grid size-9 place-content-center rounded-sm text-paper/70 hover:bg-paper/10 hover:text-amber"
          aria-label="Voltar 10 segundos"
        >
          <Back10 size={18} />
        </button>
        <button
          type="button"
          onClick={() => props.onSkip(10)}
          class="press grid size-9 place-content-center rounded-sm text-paper/70 hover:bg-paper/10 hover:text-amber"
          aria-label="Avançar 10 segundos"
        >
          <Forward10 size={18} />
        </button>

        <div class="group/vol flex items-center">
          <button
            type="button"
            onClick={props.onToggleMute}
            class="press grid size-9 place-content-center rounded-sm text-paper/70 hover:bg-paper/10 hover:text-amber"
            aria-label={props.muted ? "Ativar som" : "Silenciar"}
          >
            {props.muted || props.volume === 0 ? <VolumeOff size={18} /> : <Volume size={18} />}
          </button>
          <input
            type="range"
            min="0"
            max="1"
            step="0.02"
            value={props.muted ? 0 : props.volume}
            onInput={event => props.onVolume(Number(event.currentTarget.value))}
            aria-label="Volume"
            class="h-1 w-0 cursor-pointer accent-amber opacity-0 transition-[width,opacity] duration-200 group-hover/vol:w-20 group-hover/vol:opacity-100 focus:w-20 focus:opacity-100"
          />
        </div>

        <span class="ml-1 font-mono text-xs tabular-nums text-paper/60">
          {formatTime(props.currentTime)}
          <span class="text-paper/30"> / {formatTime(props.duration)}</span>
        </span>

        <div class="ml-auto flex items-center gap-1">
          <Show when={props.streams.length > 1}>
            <Menu
              icon={<span class="font-mono text-[0.7rem] font-bold">ÁUDIO</span>}
              label="Versão"
              onOpenChange={props.onMenuOpen}
            >
              {close => (
                <For each={props.streams}>
                  {(stream, index) => (
                    <MenuItem
                      active={index() === props.activeStream}
                      onClick={() => {
                        props.onPickStream(index());
                        close();
                      }}
                    >
                      {streamLabel(stream)}
                    </MenuItem>
                  )}
                </For>
              )}
            </Menu>
          </Show>

          <Menu
            icon={<Subtitles size={18} />}
            label="Legendas"
            active={props.activeSubtitle !== null}
            onOpenChange={props.onMenuOpen}
          >
            {close => (
              <>
                <MenuItem
                  active={props.activeSubtitle === null}
                  onClick={() => {
                    props.onPickSubtitle(null);
                    close();
                  }}
                >
                  Desligada
                </MenuItem>
                <For each={props.subtitles}>
                  {track => (
                    <MenuItem
                      active={props.activeSubtitle === track.id}
                      onClick={() => {
                        props.onPickSubtitle(track.id);
                        close();
                      }}
                    >
                      {track.label}
                    </MenuItem>
                  )}
                </For>
                <div class="my-1 border-t border-edge" />
                <MenuItem
                  onClick={() => {
                    props.onLoadSubtitle();
                    close();
                  }}
                >
                  Carregar .srt…
                </MenuItem>
              </>
            )}
          </Menu>

          <Menu
            icon={<Gauge size={18} />}
            label="Velocidade"
            active={props.speed !== 1}
            onOpenChange={props.onMenuOpen}
          >
            {close => (
              <For each={SPEEDS}>
                {speed => (
                  <MenuItem
                    active={props.speed === speed}
                    onClick={() => {
                      props.onSpeed(speed);
                      close();
                    }}
                  >
                    {speed === 1 ? "Normal" : `${speed}×`}
                  </MenuItem>
                )}
              </For>
            )}
          </Menu>

          <Show when={props.pipAvailable}>
            <button
              type="button"
              onClick={props.onPip}
              class="press grid size-9 place-content-center rounded-sm text-paper/70 hover:bg-paper/10 hover:text-amber"
              aria-label="Picture-in-picture"
            >
              <Pip size={18} />
            </button>
          </Show>

          <button
            type="button"
            onClick={props.onToggleFullscreen}
            class="press grid size-9 place-content-center rounded-sm text-paper/70 hover:bg-paper/10 hover:text-amber"
            aria-label={props.fullscreen ? "Sair da tela cheia" : "Tela cheia"}
          >
            {props.fullscreen ? <FullscreenExit size={18} /> : <Fullscreen size={18} />}
          </button>
        </div>
      </div>
    </div>
  );
}
