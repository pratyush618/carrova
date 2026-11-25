import { DownloadButton } from '../download/DownloadButton';
import type { Release } from '../download/useRelease';
import { useRef } from 'react';
import { Shot } from '../Shot';
import { usePointerDepth } from '../usePointerDepth';

export function Hero({ release }: { readonly release: Release | undefined }) {
  const stack = useRef<HTMLElement>(null);
  usePointerDepth(stack);
  return (
    <section className="hero" id="top">
      <h1>
        Your setup.
        <br />
        On your new computer.
      </h1>
      <p className="hero-sub">
        Carrova moves your app settings, keys and data from your old computer to your new one, over
        your own network. You choose what comes along.
      </p>
      <div className="hero-cta">
        <DownloadButton release={release} />
        <a className="more" href="#how">
          See how it works
        </a>
      </div>
      <p className="fine">Free and open source. No account. No cloud.</p>
      {/* Three planes: other screens behind, the plan in the middle, real close-ups in front. */}
      <figure className="hero-shot stack" ref={stack}>
        <div className="layer layer-back back-left" aria-hidden="true">
          <div className="window">
            <Shot name="nearby" sizes="(min-width: 1360px) 600px, 46vw" alt="" />
          </div>
        </div>
        <div className="layer layer-back back-right" aria-hidden="true">
          <div className="window">
            <Shot name="apps" sizes="(min-width: 1360px) 600px, 46vw" alt="" />
          </div>
        </div>
        <div className="layer layer-mid">
          <div className="window">
            <Shot
              name="review"
              eager
              sizes="(max-width: 760px) calc(100vw - 32px), min(998px, calc((100vw - 32px) * 0.78))"
              alt="The new computer's plan before anything moves: 412 new files, 3 replaced and backed up first, 28 already the same, enough space, and an Approve and move button."
            />
          </div>
        </div>
        <div className="layer layer-front front-code">
          <div className="chip crop crop-code">
            <Shot
              name="code"
              sizes="(min-width: 1360px) 640px, 60vw"
              alt="The pairing code, 7-zebra-lemon, shown on the old computer."
            />
          </div>
        </div>
        <div className="layer layer-front front-link">
          <div className="chip crop crop-link">
            <Shot
              name="linked"
              sizes="(min-width: 1360px) 760px, 60vw"
              alt="The two computers linked with a closed lock between them: only the new computer can read what is sent."
            />
          </div>
        </div>
        <div className="layer layer-front front-done">
          <div className="chip crop crop-done">
            <Shot
              name="done"
              sizes="(min-width: 1360px) 700px, 60vw"
              alt="Everything arrived: 6 apps from ezio-desktop are in place on this computer."
            />
          </div>
        </div>
      </figure>
    </section>
  );
}
