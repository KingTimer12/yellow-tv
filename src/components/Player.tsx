import { createEffect, createResource, createSignal, onCleanup, Show } from "solid-js";
import { streamUrl } from "~/lib/vod";

type PlayerProps = {
  src: string;
  title: string;
  poster?: string;
};

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

  // The engine is picked from the original URL, but playback reads from the proxy.
  const [proxied] = createResource(() => props.src, streamUrl);

  createEffect(() => {
    const element = video();
    const src = proxied();
    if (!element || !src) return;

    setError(undefined);
    setLoading(true);
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

    onCleanup(() => {
      cancelled = true;
      element.removeEventListener("loadeddata", onReady);
      element.removeEventListener("playing", onReady);
      element.removeEventListener("error", fail);
      teardown();
      element.removeAttribute("src");
      element.load();
    });
  });

  return (
    <div class="relative aspect-video w-full overflow-hidden rounded-sm bg-black ring-1 ring-edge/70">
      <video
        ref={setVideo}
        class="h-full w-full"
        controls
        autoplay
        playsinline
        poster={props.poster || undefined}
        aria-label={props.title}
      />

      {/* Tuning: an amber line sweeps the frame while the stream negotiates. */}
      <Show when={loading() && !error()}>
        <div class="tuner-sweep pointer-events-none absolute inset-0 anim-fade" aria-hidden="true" />
        <p
          class="pointer-events-none absolute inset-x-0 bottom-14 text-center font-mono text-xs text-amber"
          aria-live="polite"
        >
          sintonizando {props.title}
          <span class="anim-caret">_</span>
        </p>
      </Show>

      <Show when={error()}>
        <div class="anim-fade absolute inset-0 grid place-content-center gap-2 bg-ink/92 p-6 text-center backdrop-blur-sm">
          <p class="font-display text-lg text-paper">{error()}</p>
          <p class="mx-auto max-w-sm text-sm text-paper/60">
            Streams IPTV caem com frequência — o canal ao lado provavelmente funciona.
          </p>
        </div>
      </Show>
    </div>
  );
}
