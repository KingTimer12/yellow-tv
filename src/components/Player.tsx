import {
  createEffect,
  createResource,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
  untrack,
} from "solid-js";
import PlayerControls, { type SubtitleTrack } from "~/components/PlayerControls";
import { bufferedAhead } from "~/lib/player";
import { pickSubtitle, reportProgress, streamUrl, type OwnerKind, type StreamRef } from "~/lib/api";

type PlayerProps = {
  src: string;
  title: string;
  poster?: string;
  /** Quando presente, a posição é gravada no banco. */
  owner?: { id: string; kind: OwnerKind };
  /** Segundos de onde retomar. */
  startAt?: number;
  onEnded?: () => void;
  /** Versões do mesmo conteúdo (dublado, legendado, outra lista). */
  streams?: StreamRef[];
  activeStream?: number;
  onPickStream?: (index: number) => void;
  /** Presente só quando existe um próximo: habilita a contagem regressiva. */
  nextLabel?: string;
  onNext?: () => void;
};

const REPORT_EVERY_MS = 5000;
const HIDE_CONTROLS_MS = 3000;
const NEXT_COUNTDOWN = 10;

const DEAD_CHANNEL = "Este canal não respondeu. Tente outro ou volte em alguns instantes.";

/**
 * Xtream-style lists hand out MPEG-TS streams (`.ts`, or an extensionless URL that
 * redirects to one), which no browser plays natively — those need mpegts.js over MSE.
 * Only an explicit `.m3u8` playlist goes to hls.js.
 */
function engineFor(src: string) {
  const path = src.split("?")[0].toLowerCase();
  if (path.endsWith(".m3u8")) return "hls" as const;
  if (path.endsWith(".mp4") || path.endsWith(".webm") || path.endsWith(".mov")) {
    return "native" as const;
  }
  return "mpegts" as const;
}

export default function Player(props: PlayerProps) {
  const [video, setVideo] = createSignal<HTMLVideoElement>();
  const [error, setError] = createSignal<string>();
  const [loading, setLoading] = createSignal(true);

  const [playing, setPlaying] = createSignal(false);
  const [currentTime, setCurrentTime] = createSignal(0);
  const [duration, setDuration] = createSignal(0);
  const [buffered, setBuffered] = createSignal(0);
  const [volume, setVolume] = createSignal(1);
  const [muted, setMuted] = createSignal(false);
  const [speed, setSpeed] = createSignal(1);
  const [fullscreen, setFullscreen] = createSignal(false);
  const [pipAvailable, setPipAvailable] = createSignal(false);
  const [subtitles, setSubtitles] = createSignal<(SubtitleTrack & { url: string })[]>([]);
  const [activeSubtitle, setActiveSubtitle] = createSignal<string | null>(null);
  const [controlsVisible, setControlsVisible] = createSignal(true);
  const [menuOpen, setMenuOpen] = createSignal(false);
  const [countdown, setCountdown] = createSignal<number>();

  let container: HTMLDivElement | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let countdownTimer: ReturnType<typeof setInterval> | undefined;
  // Retomada ao trocar de versão: o efeito remonta a mídia do zero, e sem
  // guardar onde estava a troca de dublado para legendado voltaria ao início.
  let lastPosition = 0;
  let lastOwnerId: string | undefined;

  // The engine is picked from the original URL, but playback reads from the proxy.
  const [proxied] = createResource(() => props.src, streamUrl);

  const streams = () => props.streams ?? [];

  const showControls = () => {
    setControlsVisible(true);
    if (hideTimer) clearTimeout(hideTimer);
    // Nunca esconde com o vídeo parado ou com um menu aberto: nos dois casos a
    // pessoa está olhando para os controles, não para o filme.
    if (!playing() || menuOpen()) return;
    hideTimer = setTimeout(() => setControlsVisible(false), HIDE_CONTROLS_MS);
  };

  const stopCountdown = () => {
    if (countdownTimer) clearInterval(countdownTimer);
    countdownTimer = undefined;
    setCountdown(undefined);
  };

  const startCountdown = () => {
    if (!props.onNext) return;
    stopCountdown();
    setCountdown(NEXT_COUNTDOWN);
    countdownTimer = setInterval(() => {
      const left = (countdown() ?? 0) - 1;
      if (left <= 0) {
        stopCountdown();
        props.onNext?.();
        return;
      }
      setCountdown(left);
    }, 1000);
  };

  const togglePlay = () => {
    const element = video();
    if (!element) return;
    if (element.paused) void element.play().catch(() => undefined);
    else element.pause();
  };

  const skip = (delta: number) => {
    const element = video();
    if (!element) return;
    element.currentTime = Math.max(0, Math.min(element.currentTime + delta, element.duration || 0));
    showControls();
  };

  const seekTo = (seconds: number) => {
    const element = video();
    if (element) element.currentTime = seconds;
  };

  const changeVolume = (value: number) => {
    const element = video();
    if (!element) return;
    element.volume = value;
    element.muted = value === 0;
  };

  const toggleMute = () => {
    const element = video();
    if (element) element.muted = !element.muted;
  };

  const changeSpeed = (value: number) => {
    const element = video();
    if (element) element.playbackRate = value;
    setSpeed(value);
  };

  const toggleFullscreen = () => {
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined);
    else void container?.requestFullscreen().catch(() => undefined);
  };

  const togglePip = () => {
    const element = video();
    if (!element) return;
    if (document.pictureInPictureElement) void document.exitPictureInPicture().catch(() => undefined);
    else void element.requestPictureInPicture().catch(() => undefined);
  };

  const selectSubtitle = (id: string | null) => {
    setActiveSubtitle(id);
    const element = video();
    if (!element) return;
    const list = subtitles();
    for (let index = 0; index < element.textTracks.length; index += 1) {
      const track = element.textTracks[index];
      track.mode = list[index] && list[index].id === id ? "showing" : "disabled";
    }
  };

  const addSubtitle = async () => {
    try {
      const picked = await pickSubtitle();
      if (!picked) return;
      const url = URL.createObjectURL(new Blob([picked.content], { type: "text/vtt" }));
      const id = `local-${Date.now()}`;
      setSubtitles(current => [...current, { id, label: picked.label, url }]);
      // A faixa só existe no DOM depois deste quadro; ligar antes não pega.
      queueMicrotask(() => selectSubtitle(id));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  onMount(() => {
    setPipAvailable(Boolean(document.pictureInPictureEnabled));

    const onFullscreen = () => setFullscreen(Boolean(document.fullscreenElement));
    document.addEventListener("fullscreenchange", onFullscreen);

    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      // Não sequestra o teclado de quem está digitando numa busca.
      if (
        target &&
        (target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
      ) {
        return;
      }
      const element = video();
      if (!element) return;

      const handled = () => {
        event.preventDefault();
        showControls();
      };

      switch (event.key) {
        case " ":
        case "k":
        case "K":
          handled();
          togglePlay();
          return;
        case "ArrowLeft":
        case "j":
        case "J":
          handled();
          skip(-10);
          return;
        case "ArrowRight":
        case "l":
        case "L":
          handled();
          skip(10);
          return;
        case "ArrowUp":
          handled();
          changeVolume(Math.min(element.volume + 0.1, 1));
          return;
        case "ArrowDown":
          handled();
          changeVolume(Math.max(element.volume - 0.1, 0));
          return;
        case "m":
        case "M":
          handled();
          toggleMute();
          return;
        case "f":
        case "F":
          handled();
          toggleFullscreen();
          return;
        case "p":
        case "P":
          handled();
          togglePip();
          return;
        default:
          break;
      }
      // 0 a 9 saltam para a fração correspondente da duração.
      if (/^[0-9]$/.test(event.key) && Number.isFinite(element.duration)) {
        handled();
        element.currentTime = (Number(event.key) / 10) * element.duration;
      }
    };
    window.addEventListener("keydown", onKey);

    onCleanup(() => {
      document.removeEventListener("fullscreenchange", onFullscreen);
      window.removeEventListener("keydown", onKey);
      if (hideTimer) clearTimeout(hideTimer);
      stopCountdown();
      // Blob de legenda vive até ser revogado; sem isso o arquivo fica na
      // memória do processo pelo resto da sessão.
      for (const track of subtitles()) URL.revokeObjectURL(track.url);
    });
  });

  createEffect(() => {
    const element = video();
    const src = proxied();
    if (!element || !src) return;

    setError(undefined);
    setLoading(true);
    stopCountdown();
    // Congela o dono no início do efeito: quando a série avança de episódio,
    // `props.owner` já aponta para o próximo antes de o cleanup rodar, e gravar
    // o playhead do episódio que acabou contra o id do seguinte o marcaria como
    // assistido sem nunca ter tocado.
    const owner = untrack(() => props.owner);
    // Mesmo dono com mídia nova quer dizer troca de versão, não troca de
    // episódio: aí a posição é preservada.
    const switchedVersion = owner?.id !== undefined && owner.id === lastOwnerId;
    const resumeFrom = switchedVersion ? lastPosition : undefined;
    lastOwnerId = owner?.id;
    if (!switchedVersion) lastPosition = 0;

    let cancelled = false;
    let teardown = () => {};

    // Browsers refuse unmuted autoplay without a gesture; muting gets the picture
    // on screen and the native controls let the viewer turn the sound back on.
    const play = () =>
      element.play().catch(() => {
        element.muted = true;
        return element.play().catch(() => undefined);
      });
    const fail = () => {
      if (!cancelled) {
        setError(DEAD_CHANNEL);
        setLoading(false);
      }
    };

    const playNative = () => {
      element.src = src;
      play();
    };

    const playMpegts = async () => {
      const mpegts = (await import("mpegts.js")).default;
      if (cancelled) return;
      if (!mpegts.getFeatureList().mseLivePlayback) return fail();

      const player = mpegts.createPlayer(
        { type: "mpegts", isLive: true, url: src },
        { enableStashBuffer: false, liveBufferLatencyChasing: true, lazyLoad: false },
      );
      player.on(mpegts.Events.ERROR, fail);
      player.attachMediaElement(element);
      player.load();
      play();

      teardown = () => {
        player.destroy();
      };
    };

    const playHls = async () => {
      const Hls = (await import("hls.js")).default;
      if (cancelled) return;
      if (element.canPlayType("application/vnd.apple.mpegurl") !== "") return playNative();
      if (!Hls.isSupported()) return fail();

      const hls = new Hls({ enableWorker: true, lowLatencyMode: true, backBufferLength: 30 });
      hls.on(Hls.Events.ERROR, (_event, data) => {
        if (!data.fatal) return;
        if (data.type === Hls.ErrorTypes.NETWORK_ERROR) return hls.startLoad();
        if (data.type === Hls.ErrorTypes.MEDIA_ERROR) return hls.recoverMediaError();
        fail();
      });
      hls.loadSource(src);
      hls.attachMedia(element);
      hls.on(Hls.Events.MANIFEST_PARSED, play);
      teardown = () => hls.destroy();
    };

    const engine = engineFor(props.src);
    const started =
      engine === "hls" ? playHls() : engine === "mpegts" ? playMpegts() : Promise.resolve(playNative());
    Promise.resolve(started).catch(fail);

    const onReady = () => setLoading(false);
    element.addEventListener("loadeddata", onReady);
    element.addEventListener("playing", onReady);
    element.addEventListener("error", fail);

    // Retomada: só depois de o vídeo saber a duração é que dá para posicionar.
    const seek = () => {
      setDuration(Number.isFinite(element.duration) ? element.duration : 0);
      const target = resumeFrom ?? props.startAt ?? 0;
      if (target > 0 && Number.isFinite(element.duration) && element.currentTime < 1) {
        element.currentTime = target;
      }
    };
    element.addEventListener("loadedmetadata", seek);

    let lastReport = 0;
    const report = () => {
      if (!owner || !element.currentTime) return;
      const duration = Number.isFinite(element.duration) ? element.duration : null;
      void reportProgress(owner.id, owner.kind, element.currentTime, duration).catch(
        () => undefined,
      );
    };
    const onTimeUpdate = () => {
      lastPosition = element.currentTime;
      setCurrentTime(element.currentTime);
      setBuffered(bufferedAhead(element));
      const now = Date.now();
      if (now - lastReport < REPORT_EVERY_MS) return;
      lastReport = now;
      report();
    };
    const onEnded = () => {
      report();
      setPlaying(false);
      props.onEnded?.();
      startCountdown();
    };
    const onPlay = () => {
      setPlaying(true);
      showControls();
    };
    const onPause = () => {
      setPlaying(false);
      setControlsVisible(true);
      report();
    };
    const onVolumeChange = () => {
      setVolume(element.volume);
      setMuted(element.muted);
    };
    const onRateChange = () => setSpeed(element.playbackRate);
    const onDurationChange = () =>
      setDuration(Number.isFinite(element.duration) ? element.duration : 0);

    element.addEventListener("timeupdate", onTimeUpdate);
    element.addEventListener("play", onPlay);
    element.addEventListener("pause", onPause);
    element.addEventListener("ended", onEnded);
    element.addEventListener("volumechange", onVolumeChange);
    element.addEventListener("ratechange", onRateChange);
    element.addEventListener("durationchange", onDurationChange);
    element.addEventListener("progress", () => setBuffered(bufferedAhead(element)));

    onCleanup(() => {
      cancelled = true;
      report();
      element.removeEventListener("loadedmetadata", seek);
      element.removeEventListener("timeupdate", onTimeUpdate);
      element.removeEventListener("play", onPlay);
      element.removeEventListener("pause", onPause);
      element.removeEventListener("ended", onEnded);
      element.removeEventListener("volumechange", onVolumeChange);
      element.removeEventListener("ratechange", onRateChange);
      element.removeEventListener("durationchange", onDurationChange);
      element.removeEventListener("loadeddata", onReady);
      element.removeEventListener("playing", onReady);
      element.removeEventListener("error", fail);
      teardown();
      element.removeAttribute("src");
      element.load();
    });
  });

  return (
    <div
      ref={container}
      class="group/player relative aspect-video w-full overflow-hidden rounded-sm bg-black ring-1 ring-edge/70"
      classList={{ "cursor-none": !controlsVisible() }}
      onPointerMove={showControls}
      onPointerLeave={() => playing() && !menuOpen() && setControlsVisible(false)}
    >
      {/* eslint-disable-next-line jsx-a11y/media-has-caption -- as faixas entram por <For> */}
      <video
        ref={setVideo}
        class="h-full w-full"
        autoplay
        playsinline
        poster={props.poster || undefined}
        aria-label={props.title}
        onClick={togglePlay}
        onDblClick={toggleFullscreen}
      >
        <For each={subtitles()}>
          {track => <track kind="subtitles" label={track.label} src={track.url} srclang="pt" />}
        </For>
      </video>

      {/* Tuning: an amber line sweeps the frame while the stream negotiates. */}
      <Show when={loading() && !error()}>
        <div class="tuner-sweep pointer-events-none absolute inset-0 anim-fade" aria-hidden="true" />
        <p
          class="pointer-events-none absolute inset-x-0 bottom-20 text-center font-mono text-xs text-amber"
          aria-live="polite"
        >
          sintonizando {props.title}
          <span class="anim-caret">_</span>
        </p>
      </Show>

      <Show when={countdown() !== undefined}>
        <div class="anim-fade absolute inset-0 z-20 grid place-content-center gap-3 bg-ink/85 p-6 text-center backdrop-blur-sm">
          <p class="font-mono text-xs uppercase tracking-wide text-paper/45">a seguir</p>
          <p class="font-display text-xl text-paper">{props.nextLabel}</p>
          <div class="mt-2 flex items-center justify-center gap-3">
            <button
              type="button"
              onClick={() => {
                stopCountdown();
                props.onNext?.();
              }}
              class="press rounded-sm bg-amber px-4 py-2 font-medium text-ink hover:bg-paper"
            >
              Assistir agora ({countdown()})
            </button>
            <button
              type="button"
              onClick={stopCountdown}
              class="press rounded-sm border border-edge px-4 py-2 text-sm text-paper/70 hover:border-amber hover:text-amber"
            >
              Cancelar
            </button>
          </div>
        </div>
      </Show>

      <Show when={error()}>
        <div class="anim-fade absolute inset-0 z-20 grid place-content-center gap-2 bg-ink/92 p-6 text-center backdrop-blur-sm">
          <p class="font-display text-lg text-paper">{error()}</p>
          <p class="mx-auto max-w-sm text-sm text-paper/60">
            Streams IPTV caem com frequência — o canal ao lado provavelmente funciona.
          </p>
        </div>
      </Show>

      <Show when={!error()}>
        <PlayerControls
          playing={playing()}
          currentTime={currentTime()}
          duration={duration()}
          buffered={buffered()}
          volume={volume()}
          muted={muted()}
          speed={speed()}
          fullscreen={fullscreen()}
          pipAvailable={pipAvailable()}
          visible={controlsVisible() || !playing() || menuOpen()}
          streams={streams()}
          activeStream={props.activeStream ?? 0}
          subtitles={subtitles()}
          activeSubtitle={activeSubtitle()}
          onTogglePlay={togglePlay}
          onSeek={seekTo}
          onSkip={skip}
          onVolume={changeVolume}
          onToggleMute={toggleMute}
          onSpeed={changeSpeed}
          onToggleFullscreen={toggleFullscreen}
          onPip={togglePip}
          onPickStream={index => props.onPickStream?.(index)}
          onPickSubtitle={selectSubtitle}
          onLoadSubtitle={() => void addSubtitle()}
          onMenuOpen={open => {
            setMenuOpen(open);
            showControls();
          }}
        />
      </Show>
    </div>
  );
}
